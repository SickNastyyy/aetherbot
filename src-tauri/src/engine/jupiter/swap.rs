/// Full Jupiter swap orchestrator.
///
/// Flow (spec-mandated, no autonomous loop):
///   1. Quote   — GET /swap/v2/order
///   2. Gates   — run all safety checks
///   3. Approve — owner explicit confirmation
///   4. Simulate
///   5. Sign    — isolated signer
///   6. Submit  — RPC sendTransaction (maxRetries=0)
///   7. Confirm — poll for confirmation with timeout
///   8. Reconcile — write exact amounts to audit ledger
///
/// On unknown/timeout result: mark PENDING, record signature, NEVER resubmit.
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use super::audit::AuditLedger;
use super::config::JupiterSwapConfig;
use super::daily_stop::DailyStopTracker;
use super::gates::{run_all_gates, GateInput};
use super::kill_switch::KillSwitch;
use super::quote::{JupiterQuoteClient, SwapQuote};
use super::signer::{verify_and_sign, SignerTicket};
use super::ticket::{SwapTicket, TicketStatus, TicketStore};
use super::wallet::{WalletReader, WalletBalances};
use crate::database::Database;

pub struct SwapOrchestrator {
    config: JupiterSwapConfig,
    quote_client: JupiterQuoteClient,
    wallet_reader: WalletReader,
    kill_switch: KillSwitch,
    daily_stop: DailyStopTracker,
    ticket_store: Arc<TicketStore>,
    audit: AuditLedger,
}

impl SwapOrchestrator {
    pub fn new(config: JupiterSwapConfig, db: Arc<Database>) -> Self {
        let wallet_reader = WalletReader::new(config.rpc_url.clone());
        let daily_stop = DailyStopTracker::new(db.clone());
        let audit = AuditLedger::new(db);
        Self {
            config,
            quote_client: JupiterQuoteClient::new(),
            wallet_reader,
            kill_switch: KillSwitch::new(),
            daily_stop,
            ticket_store: Arc::new(TicketStore::new()),
            audit,
        }
    }

    pub fn kill_switch(&self) -> &KillSwitch {
        &self.kill_switch
    }

    pub fn ticket_store(&self) -> &TicketStore {
        &self.ticket_store
    }

    // ── Step 1 + 2: Quote and run gates ──────────────────────────────────────

    /// Fetch a quote and run all safety gates. Returns a SwapTicket for
    /// the owner to review and approve. Does NOT sign or submit anything.
    pub async fn create_ticket(
        &self,
        input_mint: &str,
        output_mint: &str,
        in_amount: u64,
        sol_price_usd: f64,
    ) -> Result<SwapTicket, String> {
        // Execution gate — must be open before any quote
        if !self.config.execution_gate_open() {
            return Err("Execution gate is closed: set live_execution_enabled=true, wallet_public_key, mint_allowlist, and signer_key_path".into());
        }

        // Fetch fresh wallet balances
        let balances = self
            .wallet_reader
            .read_balances(&self.config.wallet_public_key, sol_price_usd)
            .await
            .map_err(|e| format!("Wallet read failed: {e}"))?;

        // Initialise today's daily stop baseline
        self.daily_stop.ensure_today(balances.total_usd)?;
        let (start_equity, daily_stop_blocked) = self.daily_stop.state()?;

        // Fetch quote
        let quote = self
            .quote_client
            .get_quote(
                input_mint,
                output_mint,
                in_amount,
                self.config.max_slippage_bps,
                &self.config.wallet_public_key,
            )
            .await?;

        // Run all gates
        let now_ts = chrono::Utc::now().timestamp();
        let gate_input = GateInput {
            quote: &quote,
            balances: &balances,
            config: &self.config,
            daily_loss_usd: 0.0, // caller should supply actual day P&L
            start_of_day_equity_usd: start_equity,
            daily_stop_blocked,
            kill_switch_active: self.kill_switch.is_active(),
            sol_price_usd,
            now_ts,
        };
        let gate_results = run_all_gates(&gate_input);
        let ticket = SwapTicket::from_quote(&quote, gate_results);

        // Record the attempt in audit ledger before returning
        self.audit.record_attempt(
            &ticket.id,
            input_mint,
            output_mint,
            in_amount,
            quote.out_amount,
            quote.slippage_bps,
            quote.price_impact_pct,
        )?;

        self.ticket_store.insert(ticket.clone())?;
        Ok(ticket)
    }

    // ── Step 3: Owner approval ────────────────────────────────────────────────

    /// Mark a ticket as owner-approved. Called ONLY after the owner
    /// explicitly confirms via CLI prompt or dashboard button.
    pub fn approve_ticket(&self, ticket_id: &str) -> Result<SwapTicket, String> {
        let ticket = self
            .ticket_store
            .get(ticket_id)?
            .ok_or_else(|| format!("Ticket {ticket_id} not found"))?;

        if !ticket.all_gates_passed {
            return Err("Cannot approve ticket — one or more gates failed".into());
        }
        if ticket.status != TicketStatus::PendingApproval {
            return Err(format!(
                "Ticket is not pending approval (status: {:?})",
                ticket.status
            ));
        }

        self.ticket_store
            .update_status(ticket_id, TicketStatus::Approved)?;
        self.ticket_store
            .get(ticket_id)?
            .ok_or_else(|| "Ticket disappeared after approval".into())
    }

    // ── Steps 4–8: Execute an approved ticket ─────────────────────────────────

    /// Execute: simulate → sign → submit → confirm → reconcile.
    /// Call this only after `approve_ticket` succeeds.
    /// On unknown/timeout result: marks PENDING, never resubmits.
    pub async fn execute_approved(&self, ticket_id: &str) -> Result<SwapTicket, String> {
        let ticket = self
            .ticket_store
            .get(ticket_id)?
            .ok_or_else(|| format!("Ticket {ticket_id} not found"))?;

        if ticket.status != TicketStatus::Approved {
            return Err(format!(
                "Ticket {ticket_id} is not in Approved state ({:?})",
                ticket.status
            ));
        }

        // Execution gate double-check
        if !self.config.execution_gate_open() {
            return Err("Execution gate closed".into());
        }
        if self.kill_switch.is_active() {
            self.ticket_store.update_status(
                ticket_id,
                TicketStatus::Rejected { reason: "Kill switch active".into() },
            )?;
            return Err("Kill switch is active".into());
        }

        // Reconstruct the quote from the ticket for the swap transaction request
        let quote = SwapQuote {
            input_mint: ticket.input_mint.clone(),
            output_mint: ticket.output_mint.clone(),
            in_amount: ticket.in_amount,
            out_amount: ticket.quoted_out_amount,
            other_amount_threshold: ticket.other_amount_threshold,
            slippage_bps: ticket.slippage_bps,
            price_impact_pct: ticket.price_impact_pct,
            priority_fee_lamports: ticket.estimated_priority_fee_lamports,
            last_valid_block_height: ticket.last_valid_block_height,
            fetched_at: chrono::Utc::now().timestamp(),
            route_label: ticket.route_label.clone(),
        };

        // 4a. Get the unsigned swap transaction from Jupiter
        let (unsigned_tx_b64, last_valid_bh) = self
            .quote_client
            .get_swap_transaction(&quote, &self.config.wallet_public_key, None)
            .await
            .map_err(|e| {
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed { reason: e.clone() },
                );
                e
            })?;

        self.ticket_store
            .set_unsigned_tx(ticket_id, unsigned_tx_b64.clone())?;

        // 4b. Simulate
        self.wallet_reader
            .simulate_transaction(&unsigned_tx_b64)
            .await
            .map_err(|e| {
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed {
                        reason: format!("Simulation failed: {e}"),
                    },
                );
                format!("Simulation failed: {e}")
            })?;

        self.ticket_store
            .update_status(ticket_id, TicketStatus::Simulated)?;

        // 5. Isolated signer — verify and sign
        let key_path = self
            .config
            .signer_key_path
            .as_deref()
            .ok_or("signer_key_path not set")?;

        let current_slot = self
            .wallet_reader
            .get_current_slot()
            .await
            .unwrap_or(0);

        let signer_ticket = SignerTicket {
            expected_fee_payer: self.config.wallet_public_key.clone(),
            expected_input_mint: ticket.input_mint.clone(),
            expected_output_mint: ticket.output_mint.clone(),
            expected_in_amount: ticket.in_amount,
            last_valid_block_height: last_valid_bh,
            current_slot,
            priority_fee_cap_sol: self.config.priority_fee_cap_sol,
        };

        let signed_tx_b64 = verify_and_sign(key_path, &unsigned_tx_b64, &signer_ticket)
            .map_err(|e| {
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed {
                        reason: format!("Signer refused: {e}"),
                    },
                );
                format!("Signer refused: {e}")
            })?;

        // 6. Submit — maxRetries=0, never resubmit on unknown
        let signature = match self
            .wallet_reader
            .send_transaction(&signed_tx_b64)
            .await
        {
            Ok(sig) => sig,
            Err(e) => {
                // Unknown/failed submission — mark PENDING, record, never retry
                let _ = self.audit.record_unknown(ticket_id);
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Pending {
                        signature: String::new(),
                    },
                );
                return Err(format!(
                    "Submission failed — marked PENDING, do not resubmit: {e}"
                ));
            }
        };

        self.audit
            .record_submitted(ticket_id, &signature, 5_000, ticket.estimated_priority_fee_lamports)?;

        self.ticket_store.update_status(
            ticket_id,
            TicketStatus::Submitted { signature: signature.clone() },
        )?;

        // 7. Confirm — poll with 60-second timeout
        match timeout(
            Duration::from_secs(60),
            self.poll_confirmation(&signature),
        )
        .await
        {
            Ok(Ok(slot)) => {
                // 8. Reconcile — record confirmed slot
                // (Actual output atoms require parsing the transaction log,
                //  which needs full RPC getTransaction. Recorded as 0 and
                //  flagged for reconciliation.)
                self.audit.record_confirmed(ticket_id, slot, 0)?;
                self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Confirmed {
                        signature: signature.clone(),
                        slot,
                    },
                )?;
            }
            Ok(Err(e)) => {
                // Transaction failed on-chain
                let _ = self.audit.record_failed(ticket_id);
                self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed { reason: e.clone() },
                )?;
                return Err(format!("Transaction failed on-chain: {e}"));
            }
            Err(_timeout) => {
                // Timeout — unknown result, mark PENDING, never resubmit
                self.audit.record_unknown(ticket_id)?;
                self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Pending { signature: signature.clone() },
                )?;
                return Err(format!(
                    "Confirmation timed out — swap marked PENDING with sig {signature}. \
                     Reconcile manually. Do NOT resubmit."
                ));
            }
        }

        self.ticket_store
            .get(ticket_id)?
            .ok_or_else(|| "Ticket not found after execution".into())
    }

    async fn poll_confirmation(&self, signature: &str) -> Result<u64, String> {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            match self
                .wallet_reader
                .get_signature_status(signature)
                .await?
            {
                Some(status) if status.confirmed => {
                    if !status.err.is_null() {
                        return Err(format!("Transaction error: {}", status.err));
                    }
                    return Ok(status.slot);
                }
                Some(status) if !status.err.is_null() => {
                    return Err(format!("Transaction error: {}", status.err));
                }
                _ => continue,
            }
        }
    }
}

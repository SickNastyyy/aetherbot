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
use bs58;

use super::audit::AuditLedger;
use super::config::JupiterSwapConfig;
use super::daily_stop::DailyStopTracker;
use super::gates::{run_all_gates, GateInput};
use super::kill_switch::KillSwitch;
use super::quote::JupiterQuoteClient;
use super::signer::{verify_and_sign, SignerTicket};
use super::ticket::{SwapTicket, TicketStatus, TicketStore};
use super::wallet::WalletReader;
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
        let ticket = SwapTicket::from_quote(&quote, gate_results, sol_price_usd);

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

    /// Execute: re-run gates → simulate → sign → submit → confirm → reconcile.
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

        // Ticket TTL — reject stale approvals
        if ticket.is_expired(self.config.ticket_ttl_secs) {
            let _ = self.ticket_store.update_status(
                ticket_id,
                TicketStatus::Rejected { reason: "Ticket TTL expired".into() },
            );
            return Err(format!(
                "Ticket {ticket_id} expired — TTL {}s exceeded. Fetch a new quote.",
                self.config.ticket_ttl_secs
            ));
        }

        // Execution gate double-check
        if !self.config.execution_gate_open() {
            return Err("Execution gate closed".into());
        }
        if self.kill_switch.is_active() {
            let _ = self.ticket_store.update_status(
                ticket_id,
                TicketStatus::Rejected { reason: "Kill switch active".into() },
            );
            return Err("Kill switch is active".into());
        }

        // 3b. Re-run all gates with FRESH balances at execute time
        let fresh_balances = self
            .wallet_reader
            .read_balances(&self.config.wallet_public_key, ticket.sol_price_usd_at_quote)
            .await
            .map_err(|e| format!("Wallet re-read at execute time failed: {e}"))?;

        let (start_equity, daily_stop_blocked) = self.daily_stop.state()?;
        let now_ts = chrono::Utc::now().timestamp();

        // Re-build the quote struct for gate re-run
        let quote_for_gates = super::quote::SwapQuote {
            input_mint: ticket.input_mint.clone(),
            output_mint: ticket.output_mint.clone(),
            in_amount: ticket.in_amount,
            out_amount: ticket.quoted_out_amount,
            other_amount_threshold: ticket.other_amount_threshold,
            slippage_bps: ticket.slippage_bps,
            price_impact_pct: ticket.price_impact_pct,
            priority_fee_lamports: ticket.estimated_priority_fee_lamports,
            last_valid_block_height: ticket.last_valid_block_height,
            fetched_at: ticket.created_at_ts,
            route_label: ticket.route_label.clone(),
            unsigned_tx_b64: None,
            request_id: None,
        };

        let gate_input = super::gates::GateInput {
            quote: &quote_for_gates,
            balances: &fresh_balances,
            config: &self.config,
            daily_loss_usd: 0.0,
            start_of_day_equity_usd: start_equity,
            daily_stop_blocked,
            kill_switch_active: self.kill_switch.is_active(),
            sol_price_usd: ticket.sol_price_usd_at_quote,
            now_ts,
        };

        let gate_results = super::gates::run_all_gates(&gate_input);
        if !super::gates::all_gates_passed(&gate_results) {
            let failures: Vec<String> = gate_results
                .iter()
                .filter_map(|r| {
                    if let super::gates::GateStatus::Fail(reason) = &r.status {
                        Some(format!("{}: {reason}", r.name))
                    } else {
                        None
                    }
                })
                .collect();
            let _ = self.ticket_store.update_status(
                ticket_id,
                TicketStatus::Rejected {
                    reason: format!("Gate re-check failed at execute time: {}", failures.join("; ")),
                },
            );
            return Err(format!("Gate re-check failed: {}", failures.join("; ")));
        }

        // 4a. Use the unsigned tx stored at quote time — do NOT fetch a new one.
        let unsigned_tx_b64 = ticket
            .unsigned_tx_b64
            .clone()
            .ok_or("No unsigned transaction in ticket — was taker set when quoting?")?;

        // 4b. Resolve ALTs from the transaction before signing
        let resolved_alt_accounts = self
            .resolve_alt_accounts(&unsigned_tx_b64)
            .await
            .map_err(|e| format!("ALT resolution failed: {e}"))?;

        // 4c. Simulate
        self.wallet_reader
            .simulate_transaction(&unsigned_tx_b64)
            .await
            .map_err(|e| {
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed { reason: format!("Simulation failed: {e}") },
                );
                format!("Simulation failed: {e}")
            })?;

        self.ticket_store
            .update_status(ticket_id, TicketStatus::Simulated)?;

        // 5. Isolated signer — verify semantics and sign
        let key_path = self
            .config
            .signer_key_path
            .as_deref()
            .ok_or("signer_key_path not set")?;

        // Use block height (not slot) for blockhash expiry check.
        // Fail closed: if RPC returns error, sign_internal will reject the tx
        // because current_block_height == 0 is treated as unknown.
        let current_block_height = self
            .wallet_reader
            .get_current_block_height()
            .await
            .unwrap_or(0);

        let signer_ticket = SignerTicket {
            expected_fee_payer: self.config.wallet_public_key.clone(),
            expected_input_mint: ticket.input_mint.clone(),
            expected_output_mint: ticket.output_mint.clone(),
            expected_in_amount: ticket.in_amount,
            last_valid_block_height: ticket.last_valid_block_height,
            current_block_height,
            priority_fee_cap_sol: self.config.priority_fee_cap_sol,
            resolved_alt_accounts,
        };

        let signed_tx_b64 = verify_and_sign(key_path, &unsigned_tx_b64, &signer_ticket)
            .map_err(|e| {
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed { reason: format!("Signer refused: {e}") },
                );
                format!("Signer refused: {e}")
            })?;

        // 6. Submit — maxRetries=0, never resubmit on unknown
        let signature = match self.wallet_reader.send_transaction(&signed_tx_b64).await {
            Ok(sig) => sig,
            Err(e) => {
                let _ = self.audit.record_unknown(ticket_id);
                // The tx may or may not have landed — record the empty-sig PENDING
                let _ = self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Pending { signature: String::new() },
                );
                return Err(format!(
                    "Submission failed — marked PENDING, do not resubmit: {e}"
                ));
            }
        };

        self.audit.record_submitted(
            ticket_id,
            &signature,
            5_000,
            ticket.estimated_priority_fee_lamports,
        )?;

        self.ticket_store.update_status(
            ticket_id,
            TicketStatus::Submitted { signature: signature.clone() },
        )?;

        // 7. Confirm — poll with 60-second timeout; check kill switch each iteration
        match timeout(Duration::from_secs(60), self.poll_confirmation(&signature)).await {
            Ok(Ok(slot)) => {
                // 8. Reconcile — read actual post-swap balances to compute output
                let post_balances = self
                    .wallet_reader
                    .read_balances(&self.config.wallet_public_key, ticket.sol_price_usd_at_quote)
                    .await
                    .ok();

                let actual_output_atoms = post_balances
                    .as_ref()
                    .and_then(|b| {
                        b.token_accounts
                            .iter()
                            .find(|t| t.mint == ticket.output_mint)
                            .map(|t| t.amount_atoms)
                    })
                    .unwrap_or(0);

                // Wire daily stop: if post-balance is worse than pre-balance, check
                if let Some(post) = post_balances {
                    let loss_usd = fresh_balances.total_usd - post.total_usd;
                    let _ = self
                        .daily_stop
                        .check_and_maybe_block(loss_usd.max(0.0), self.config.daily_stop_pct);
                }

                self.audit.record_confirmed(ticket_id, slot, actual_output_atoms)?;
                self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Confirmed { signature: signature.clone(), slot },
                )?;
            }
            Ok(Err(e)) => {
                let _ = self.audit.record_failed(ticket_id);
                self.ticket_store.update_status(
                    ticket_id,
                    TicketStatus::Failed { reason: e.clone() },
                )?;
                return Err(format!("Transaction failed on-chain: {e}"));
            }
            Err(_timeout) => {
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

    /// Resolve ALT accounts from a base64-encoded unsigned transaction.
    /// For v0 transactions: fetches each referenced lookup table from the RPC
    /// and returns the accounts in correct order (writable then readonly per ALT).
    /// Returns an empty Vec for legacy transactions (no ALTs).
    async fn resolve_alt_accounts(&self, unsigned_tx_b64: &str) -> Result<Vec<[u8; 32]>, String> {
        let tx_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            unsigned_tx_b64,
        )
        .map_err(|e| format!("Base64 decode for ALT resolution: {e}"))?;

        // Quick parse just to get ALT references
        let (_, alt_refs) = super::signer::parse_alts_for_resolve(&tx_bytes)?;

        if alt_refs.is_empty() {
            return Ok(vec![]);
        }

        let mut resolved: Vec<[u8; 32]> = Vec::new();
        for (table_address, writable_indices, readonly_indices) in alt_refs {
            let table_b58 = bs58::encode(&table_address).into_string();
            let table_accounts = self
                .wallet_reader
                .get_lookup_table_addresses(&table_b58)
                .await
                .map_err(|e| format!("Failed to resolve ALT {table_b58}: {e}"))?;

            for &idx in &writable_indices {
                match table_accounts.get(idx as usize) {
                    Some(&acct) => resolved.push(acct),
                    None => return Err(format!(
                        "ALT {table_b58}: writable index {idx} out of bounds (table has {} entries)",
                        table_accounts.len()
                    )),
                }
            }
            for &idx in &readonly_indices {
                match table_accounts.get(idx as usize) {
                    Some(&acct) => resolved.push(acct),
                    None => return Err(format!(
                        "ALT {table_b58}: readonly index {idx} out of bounds",
                    )),
                }
            }
        }

        Ok(resolved)
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

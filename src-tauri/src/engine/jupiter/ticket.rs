use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use super::gates::GateResult;
use super::quote::SwapQuote;

// ── TicketStatus ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TicketStatus {
    PendingApproval,
    Approved,
    Rejected { reason: String },
    Simulated,
    Submitted { signature: String },
    Confirmed { signature: String, slot: u64 },
    Failed { reason: String },
    /// Unknown result — do not resubmit. Reconcile by signature.
    Pending { signature: String },
}

// ── SwapTicket ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapTicket {
    /// Unique ticket ID: "jup-<timestamp_nanos>"
    pub id: String,
    pub created_at: String,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: u64,
    pub quoted_out_amount: u64,
    /// Minimum output after slippage
    pub other_amount_threshold: u64,
    pub slippage_bps: u32,
    pub price_impact_pct: f64,
    pub estimated_priority_fee_lamports: u64,
    pub last_valid_block_height: u64,
    pub route_label: String,
    pub gate_results: Vec<GateResult>,
    pub all_gates_passed: bool,
    pub status: TicketStatus,
    /// Base64-encoded unsigned transaction (set after owner approval step)
    pub unsigned_tx_b64: Option<String>,
}

impl SwapTicket {
    pub fn from_quote(quote: &SwapQuote, gate_results: Vec<GateResult>) -> Self {
        let all_gates_passed = super::gates::all_gates_passed(&gate_results);
        let id = format!(
            "jup-{}",
            chrono::Utc::now()
                .timestamp_nanos_opt()
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() * 1_000_000)
        );
        Self {
            id,
            created_at: chrono::Utc::now().to_rfc3339(),
            input_mint: quote.input_mint.clone(),
            output_mint: quote.output_mint.clone(),
            in_amount: quote.in_amount,
            quoted_out_amount: quote.out_amount,
            other_amount_threshold: quote.other_amount_threshold,
            slippage_bps: quote.slippage_bps,
            price_impact_pct: quote.price_impact_pct,
            estimated_priority_fee_lamports: quote.priority_fee_lamports,
            last_valid_block_height: quote.last_valid_block_height,
            route_label: quote.route_label.clone(),
            gate_results,
            all_gates_passed,
            status: TicketStatus::PendingApproval,
            unsigned_tx_b64: None,
        }
    }
}

// ── TicketStore ───────────────────────────────────────────────────────────────

/// Single-slot pending ticket store. Only one ticket can be pending at a time
/// (no autonomous loop — each swap requires explicit owner approval).
pub struct TicketStore {
    inner: Mutex<HashMap<String, SwapTicket>>,
}

impl TicketStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, ticket: SwapTicket) -> Result<(), String> {
        let mut map = self
            .inner
            .lock()
            .map_err(|e| format!("TicketStore lock: {e}"))?;
        map.insert(ticket.id.clone(), ticket);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<SwapTicket>, String> {
        let map = self
            .inner
            .lock()
            .map_err(|e| format!("TicketStore lock: {e}"))?;
        Ok(map.get(id).cloned())
    }

    pub fn update_status(&self, id: &str, status: TicketStatus) -> Result<(), String> {
        let mut map = self
            .inner
            .lock()
            .map_err(|e| format!("TicketStore lock: {e}"))?;
        if let Some(ticket) = map.get_mut(id) {
            ticket.status = status;
            Ok(())
        } else {
            Err(format!("Ticket {id} not found"))
        }
    }

    pub fn set_unsigned_tx(&self, id: &str, tx_b64: String) -> Result<(), String> {
        let mut map = self
            .inner
            .lock()
            .map_err(|e| format!("TicketStore lock: {e}"))?;
        if let Some(ticket) = map.get_mut(id) {
            ticket.unsigned_tx_b64 = Some(tx_b64);
            Ok(())
        } else {
            Err(format!("Ticket {id} not found"))
        }
    }

    pub fn all(&self) -> Result<Vec<SwapTicket>, String> {
        let map = self
            .inner
            .lock()
            .map_err(|e| format!("TicketStore lock: {e}"))?;
        let mut tickets: Vec<SwapTicket> = map.values().cloned().collect();
        tickets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(tickets)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut map = self
            .inner
            .lock()
            .map_err(|e| format!("TicketStore lock: {e}"))?;
        map.remove(id);
        Ok(())
    }
}

impl Default for TicketStore {
    fn default() -> Self {
        Self::new()
    }
}

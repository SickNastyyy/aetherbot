use crate::database::{self, Database};

// ── AuditLedger ───────────────────────────────────────────────────────────────

/// Writes exact swap records to the jupiter_audit SQLite table.
/// Every swap attempt — regardless of outcome — gets an entry.
pub struct AuditLedger {
    db: std::sync::Arc<Database>,
}

impl AuditLedger {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self { db }
    }

    /// Record the initial swap attempt.
    pub fn record_attempt(
        &self,
        swap_id: &str,
        input_mint: &str,
        output_mint: &str,
        input_atoms: u64,
        quoted_output_atoms: u64,
        slippage_bps: u32,
        price_impact_pct: f64,
    ) -> Result<(), String> {
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        database::insert_jupiter_audit(
            &conn,
            swap_id,
            &chrono::Utc::now().to_rfc3339(),
            input_mint,
            output_mint,
            input_atoms,
            quoted_output_atoms,
            slippage_bps,
            price_impact_pct,
        )
        .map_err(|e| format!("DB insert_jupiter_audit: {e}"))
    }

    /// Record that the transaction was submitted.
    pub fn record_submitted(
        &self,
        swap_id: &str,
        signature: &str,
        fee_lamports: u64,
        priority_fee_lamports: u64,
    ) -> Result<(), String> {
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        database::update_jupiter_audit_submitted(
            &conn,
            swap_id,
            signature,
            fee_lamports,
            priority_fee_lamports,
        )
        .map_err(|e| format!("DB update_jupiter_audit_submitted: {e}"))
    }

    /// Record a confirmed swap — exact on-chain amounts.
    pub fn record_confirmed(
        &self,
        swap_id: &str,
        slot: u64,
        actual_output_atoms: u64,
    ) -> Result<(), String> {
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        database::update_jupiter_audit_confirmed(
            &conn,
            swap_id,
            slot,
            actual_output_atoms,
            &chrono::Utc::now().to_rfc3339(),
        )
        .map_err(|e| format!("DB update_jupiter_audit_confirmed: {e}"))
    }

    /// Record a terminal failure (gates failed, signer refused, etc.).
    pub fn record_failed(&self, swap_id: &str) -> Result<(), String> {
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        database::update_jupiter_audit_status(&conn, swap_id, "failed")
            .map_err(|e| format!("DB update status failed: {e}"))
    }

    /// Mark as "unknown" — submission timed out or errored.
    /// Do NOT resubmit. Reconcile by signature later.
    pub fn record_unknown(&self, swap_id: &str) -> Result<(), String> {
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        database::update_jupiter_audit_status(&conn, swap_id, "unknown")
            .map_err(|e| format!("DB update status unknown: {e}"))
    }

    /// Return audit log entries (most recent first).
    pub fn get_log(
        &self,
        limit: u32,
    ) -> Result<Vec<crate::database::JupiterAuditRow>, String> {
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        database::get_jupiter_audit_log(&conn, limit)
            .map_err(|e| format!("DB get_jupiter_audit_log: {e}"))
    }
}

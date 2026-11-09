use chrono::TimeZone;
use chrono_tz::America::New_York;

use crate::database::{self, Database};

// ── DailyStopTracker ──────────────────────────────────────────────────────────

/// Tracks the daily stop-loss state, persisted across restarts via SQLite.
/// The "day" boundary is midnight America/New_York.
pub struct DailyStopTracker {
    db: std::sync::Arc<Database>,
}

impl DailyStopTracker {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self { db }
    }

    /// Today's date string in America/New_York: "YYYY-MM-DD".
    pub fn today_ny() -> String {
        let now_utc = chrono::Utc::now();
        let now_ny = New_York.from_utc_datetime(&now_utc.naive_utc());
        now_ny.format("%Y-%m-%d").to_string()
    }

    /// Initialise the row for today if it doesn't exist yet.
    /// `current_equity_usd` is the wallet value at the start of day.
    pub fn ensure_today(&self, current_equity_usd: f64) -> Result<(), String> {
        let date = Self::today_ny();
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;

        // Only insert if not already present (don't overwrite start-of-day equity)
        let existing = database::get_jupiter_daily_stop(&conn, &date)
            .map_err(|e| format!("DB error: {e}"))?;

        if existing.is_none() {
            database::upsert_jupiter_daily_stop(&conn, &date, current_equity_usd, false)
                .map_err(|e| format!("DB error: {e}"))?;
        }

        Ok(())
    }

    /// Check whether the daily stop is active for today.
    pub fn is_blocked(&self) -> Result<bool, String> {
        let date = Self::today_ny();
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        match database::get_jupiter_daily_stop(&conn, &date)
            .map_err(|e| format!("DB error: {e}"))?
        {
            Some(row) => Ok(row.blocked),
            None => Ok(false),
        }
    }

    /// Return (start_of_day_equity_usd, blocked).
    pub fn state(&self) -> Result<(f64, bool), String> {
        let date = Self::today_ny();
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        match database::get_jupiter_daily_stop(&conn, &date)
            .map_err(|e| format!("DB error: {e}"))?
        {
            Some(row) => Ok((row.start_of_day_equity_usd, row.blocked)),
            None => Ok((0.0, false)),
        }
    }

    /// Evaluate whether the daily stop should trigger.
    /// `loss_usd` is today's realized + unrealized loss (positive = loss).
    /// `stop_pct` is the threshold (e.g., 10.0 for 10%).
    /// Persists the blocked state if it triggers.
    pub fn check_and_maybe_block(
        &self,
        loss_usd: f64,
        stop_pct: f64,
    ) -> Result<bool, String> {
        let date = Self::today_ny();
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;

        let row = database::get_jupiter_daily_stop(&conn, &date)
            .map_err(|e| format!("DB error: {e}"))?;

        let (start_equity, already_blocked) = match row {
            Some(r) => (r.start_of_day_equity_usd, r.blocked),
            None => return Ok(false),
        };

        if already_blocked {
            return Ok(true);
        }

        let loss_pct = if start_equity > 0.0 {
            (loss_usd / start_equity) * 100.0
        } else {
            0.0
        };

        if loss_pct >= stop_pct {
            database::upsert_jupiter_daily_stop(&conn, &date, start_equity, true)
                .map_err(|e| format!("DB error: {e}"))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Force-block for today (e.g., manual kill).
    pub fn block_today(&self) -> Result<(), String> {
        let date = Self::today_ny();
        let conn = self.db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        let row = database::get_jupiter_daily_stop(&conn, &date)
            .map_err(|e| format!("DB error: {e}"))?;
        let start_equity = row.map(|r| r.start_of_day_equity_usd).unwrap_or(0.0);
        database::upsert_jupiter_daily_stop(&conn, &date, start_equity, true)
            .map_err(|e| format!("DB error: {e}"))
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use crate::database::Database;
    use rusqlite::Connection;
    use std::sync::Arc;

    pub fn make_test_db() -> Arc<Database> {
        let conn = Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS jupiter_daily_stop (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL UNIQUE,
                start_of_day_equity_usd REAL NOT NULL,
                blocked INTEGER NOT NULL DEFAULT 0
            );",
        )
        .expect("schema");
        Arc::new(Database { conn: std::sync::Mutex::new(conn) })
    }
}

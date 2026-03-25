use crate::paths;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct Database {
    // Used by commands in Phase 2+ for trade history queries
    #[allow(dead_code)]
    pub conn: Mutex<Connection>,
}

pub fn init_database() -> Result<Database, String> {
    let path = paths::get_data_dir()?.join("trades.db");
    let conn = Connection::open(&path)
        .map_err(|e| format!("Failed to open database at {}: {e}", path.display()))?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS trades (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            exchange TEXT NOT NULL,
            pair TEXT NOT NULL,
            side TEXT NOT NULL,
            entry_price REAL NOT NULL,
            exit_price REAL,
            quantity REAL NOT NULL,
            pnl REAL,
            pnl_pct REAL,
            status TEXT NOT NULL DEFAULT 'OPEN',
            strategy TEXT,
            opened_at TEXT NOT NULL,
            closed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS trade_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            context_json TEXT
        );

        CREATE TABLE IF NOT EXISTS settings_kv (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_trades_exchange ON trades(exchange);
        CREATE INDEX IF NOT EXISTS idx_trades_pair ON trades(pair);
        CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
        CREATE INDEX IF NOT EXISTS idx_trade_log_timestamp ON trade_log(timestamp);
        ",
    )
    .map_err(|e| format!("Failed to initialize database schema: {e}"))?;

    Ok(Database {
        conn: Mutex::new(conn),
    })
}

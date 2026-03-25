use crate::paths;
use rusqlite::{params, Connection};
use std::sync::Mutex;

pub struct Database {
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
            trade_id TEXT UNIQUE,
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
            stop_loss REAL,
            take_profit REAL,
            time_limit TEXT,
            exit_reason TEXT,
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

        CREATE TABLE IF NOT EXISTS candles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            exchange TEXT NOT NULL,
            pair TEXT NOT NULL,
            timeframe TEXT NOT NULL,
            open REAL NOT NULL,
            high REAL NOT NULL,
            low REAL NOT NULL,
            close REAL NOT NULL,
            volume REAL NOT NULL,
            timestamp INTEGER NOT NULL,
            UNIQUE(exchange, pair, timeframe, timestamp)
        );

        CREATE TABLE IF NOT EXISTS portfolio_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            total_equity REAL NOT NULL,
            available_balance REAL NOT NULL,
            unrealized_pnl REAL NOT NULL,
            open_positions INTEGER NOT NULL DEFAULT 0,
            total_trades INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS settings_kv (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_trades_exchange ON trades(exchange);
        CREATE INDEX IF NOT EXISTS idx_trades_pair ON trades(pair);
        CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
        CREATE INDEX IF NOT EXISTS idx_trades_trade_id ON trades(trade_id);
        CREATE INDEX IF NOT EXISTS idx_trade_log_timestamp ON trade_log(timestamp);
        CREATE INDEX IF NOT EXISTS idx_candles_lookup ON candles(exchange, pair, timeframe, timestamp);
        CREATE INDEX IF NOT EXISTS idx_portfolio_ts ON portfolio_snapshots(timestamp);
        ",
    )
    .map_err(|e| format!("Failed to initialize database schema: {e}"))?;

    Ok(Database {
        conn: Mutex::new(conn),
    })
}

// ── Trade CRUD ──────────────────────────────────────────────────────────────

pub fn insert_trade(
    conn: &Connection,
    trade_id: &str,
    exchange: &str,
    pair: &str,
    side: &str,
    entry_price: f64,
    quantity: f64,
    strategy: &str,
    stop_loss: f64,
    take_profit: f64,
    time_limit: &str,
    opened_at: &str,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO trades (trade_id, exchange, pair, side, entry_price, quantity, strategy, stop_loss, take_profit, time_limit, status, opened_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'OPEN', ?11)",
        params![trade_id, exchange, pair, side, entry_price, quantity, strategy, stop_loss, take_profit, time_limit, opened_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn close_trade(
    conn: &Connection,
    trade_id: &str,
    exit_price: f64,
    pnl: f64,
    pnl_pct: f64,
    exit_reason: &str,
    closed_at: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE trades SET exit_price = ?1, pnl = ?2, pnl_pct = ?3, status = 'CLOSED', exit_reason = ?4, closed_at = ?5 WHERE trade_id = ?6",
        params![exit_price, pnl, pnl_pct, exit_reason, closed_at, trade_id],
    )?;
    Ok(())
}

pub fn get_open_trades(conn: &Connection) -> Result<Vec<TradeRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT trade_id, exchange, pair, side, entry_price, quantity, strategy, stop_loss, take_profit, time_limit, opened_at FROM trades WHERE status = 'OPEN' ORDER BY opened_at DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TradeRow {
            trade_id: row.get(0)?,
            exchange: row.get(1)?,
            pair: row.get(2)?,
            side: row.get(3)?,
            entry_price: row.get(4)?,
            quantity: row.get(5)?,
            strategy: row.get(6)?,
            stop_loss: row.get(7)?,
            take_profit: row.get(8)?,
            time_limit: row.get(9)?,
            opened_at: row.get(10)?,
        })
    })?;
    rows.collect()
}

pub fn get_trade_history(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<TradeHistoryRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT trade_id, exchange, pair, side, entry_price, exit_price, quantity, pnl, pnl_pct, status, strategy, exit_reason, opened_at, closed_at FROM trades ORDER BY opened_at DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(TradeHistoryRow {
            trade_id: row.get(0)?,
            exchange: row.get(1)?,
            pair: row.get(2)?,
            side: row.get(3)?,
            entry_price: row.get(4)?,
            exit_price: row.get(5)?,
            quantity: row.get(6)?,
            pnl: row.get(7)?,
            pnl_pct: row.get(8)?,
            status: row.get(9)?,
            strategy: row.get(10)?,
            exit_reason: row.get(11)?,
            opened_at: row.get(12)?,
            closed_at: row.get(13)?,
        })
    })?;
    rows.collect()
}

pub fn get_daily_pnl(conn: &Connection, date: &str) -> Result<f64, rusqlite::Error> {
    let pnl: f64 = conn.query_row(
        "SELECT COALESCE(SUM(pnl), 0.0) FROM trades WHERE status = 'CLOSED' AND closed_at LIKE ?1",
        params![format!("{date}%")],
        |row| row.get(0),
    )?;
    Ok(pnl)
}

// ── Candle Persistence ──────────────────────────────────────────────────────

pub fn insert_candle(
    conn: &Connection,
    exchange: &str,
    pair: &str,
    timeframe: &str,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    timestamp: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO candles (exchange, pair, timeframe, open, high, low, close, volume, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![exchange, pair, timeframe, open, high, low, close, volume, timestamp],
    )?;
    Ok(())
}

pub fn load_recent_candles(
    conn: &Connection,
    exchange: &str,
    pair: &str,
    timeframe: &str,
    limit: u32,
) -> Result<Vec<CandleRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT open, high, low, close, volume, timestamp FROM candles WHERE exchange = ?1 AND pair = ?2 AND timeframe = ?3 ORDER BY timestamp DESC LIMIT ?4"
    )?;
    let rows = stmt.query_map(params![exchange, pair, timeframe, limit], |row| {
        Ok(CandleRow {
            open: row.get(0)?,
            high: row.get(1)?,
            low: row.get(2)?,
            close: row.get(3)?,
            volume: row.get(4)?,
            timestamp: row.get(5)?,
        })
    })?;
    let mut result: Vec<CandleRow> = rows.collect::<Result<Vec<_>, _>>()?;
    result.reverse(); // oldest first
    Ok(result)
}

// ── Portfolio Snapshots ─────────────────────────────────────────────────────

pub fn insert_portfolio_snapshot(
    conn: &Connection,
    timestamp: &str,
    total_equity: f64,
    available_balance: f64,
    unrealized_pnl: f64,
    open_positions: u32,
    total_trades: u32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO portfolio_snapshots (timestamp, total_equity, available_balance, unrealized_pnl, open_positions, total_trades) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![timestamp, total_equity, available_balance, unrealized_pnl, open_positions, total_trades],
    )?;
    Ok(())
}

// ── Log Persistence ─────────────────────────────────────────────────────────

pub fn insert_log(
    conn: &Connection,
    timestamp: &str,
    level: &str,
    message: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO trade_log (timestamp, level, message) VALUES (?1, ?2, ?3)",
        params![timestamp, level, message],
    )?;
    Ok(())
}

// ── Row types ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TradeRow {
    pub trade_id: String,
    pub exchange: String,
    pub pair: String,
    pub side: String,
    pub entry_price: f64,
    pub quantity: f64,
    pub strategy: String,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub time_limit: String,
    pub opened_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TradeHistoryRow {
    pub trade_id: String,
    pub exchange: String,
    pub pair: String,
    pub side: String,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub quantity: f64,
    pub pnl: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub status: String,
    pub strategy: Option<String>,
    pub exit_reason: Option<String>,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug)]
pub struct CandleRow {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: i64,
}

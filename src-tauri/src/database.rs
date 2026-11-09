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

        CREATE TABLE IF NOT EXISTS jupiter_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            swap_id TEXT UNIQUE NOT NULL,
            created_at TEXT NOT NULL,
            input_mint TEXT NOT NULL,
            output_mint TEXT NOT NULL,
            input_atoms INTEGER NOT NULL,
            quoted_output_atoms INTEGER NOT NULL,
            actual_output_atoms INTEGER,
            slippage_bps INTEGER NOT NULL,
            price_impact_pct REAL NOT NULL,
            fee_lamports INTEGER,
            priority_fee_lamports INTEGER,
            signature TEXT,
            slot INTEGER,
            confirmed_at TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            reconciled INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS jupiter_daily_stop (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL UNIQUE,
            start_of_day_equity_usd REAL NOT NULL,
            blocked INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_jupiter_audit_swap_id ON jupiter_audit(swap_id);
        CREATE INDEX IF NOT EXISTS idx_jupiter_audit_signature ON jupiter_audit(signature);
        CREATE INDEX IF NOT EXISTS idx_jupiter_daily_stop_date ON jupiter_daily_stop(date);
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

// ── Jupiter Audit Ledger ────────────────────────────────────────────────────

pub fn insert_jupiter_audit(
    conn: &Connection,
    swap_id: &str,
    created_at: &str,
    input_mint: &str,
    output_mint: &str,
    input_atoms: u64,
    quoted_output_atoms: u64,
    slippage_bps: u32,
    price_impact_pct: f64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO jupiter_audit (swap_id, created_at, input_mint, output_mint, input_atoms, \
         quoted_output_atoms, slippage_bps, price_impact_pct, status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending')",
        params![
            swap_id, created_at, input_mint, output_mint,
            input_atoms as i64, quoted_output_atoms as i64,
            slippage_bps, price_impact_pct
        ],
    )?;
    Ok(())
}

pub fn update_jupiter_audit_submitted(
    conn: &Connection,
    swap_id: &str,
    signature: &str,
    fee_lamports: u64,
    priority_fee_lamports: u64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE jupiter_audit SET signature = ?1, fee_lamports = ?2, \
         priority_fee_lamports = ?3, status = 'submitted' WHERE swap_id = ?4",
        params![signature, fee_lamports as i64, priority_fee_lamports as i64, swap_id],
    )?;
    Ok(())
}

pub fn update_jupiter_audit_confirmed(
    conn: &Connection,
    swap_id: &str,
    slot: u64,
    actual_output_atoms: u64,
    confirmed_at: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE jupiter_audit SET slot = ?1, actual_output_atoms = ?2, \
         confirmed_at = ?3, status = 'confirmed', reconciled = 1 WHERE swap_id = ?4",
        params![slot as i64, actual_output_atoms as i64, confirmed_at, swap_id],
    )?;
    Ok(())
}

pub fn update_jupiter_audit_status(
    conn: &Connection,
    swap_id: &str,
    status: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE jupiter_audit SET status = ?1 WHERE swap_id = ?2",
        params![status, swap_id],
    )?;
    Ok(())
}

pub fn get_jupiter_audit_log(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<JupiterAuditRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT swap_id, created_at, input_mint, output_mint, input_atoms, quoted_output_atoms, \
         actual_output_atoms, slippage_bps, price_impact_pct, fee_lamports, priority_fee_lamports, \
         signature, slot, confirmed_at, status, reconciled \
         FROM jupiter_audit ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(JupiterAuditRow {
            swap_id: row.get(0)?,
            created_at: row.get(1)?,
            input_mint: row.get(2)?,
            output_mint: row.get(3)?,
            input_atoms: row.get::<_, i64>(4)? as u64,
            quoted_output_atoms: row.get::<_, i64>(5)? as u64,
            actual_output_atoms: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
            slippage_bps: row.get::<_, i64>(7)? as u32,
            price_impact_pct: row.get(8)?,
            fee_lamports: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
            priority_fee_lamports: row.get::<_, Option<i64>>(10)?.map(|v| v as u64),
            signature: row.get(11)?,
            slot: row.get::<_, Option<i64>>(12)?.map(|v| v as u64),
            confirmed_at: row.get(13)?,
            status: row.get(14)?,
            reconciled: row.get::<_, i64>(15)? != 0,
        })
    })?;
    rows.collect()
}

// ── Jupiter Daily Stop ───────────────────────────────────────────────────────

pub fn upsert_jupiter_daily_stop(
    conn: &Connection,
    date: &str,
    start_of_day_equity_usd: f64,
    blocked: bool,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO jupiter_daily_stop (date, start_of_day_equity_usd, blocked) \
         VALUES (?1, ?2, ?3) \
         ON CONFLICT(date) DO UPDATE SET blocked = excluded.blocked",
        params![date, start_of_day_equity_usd, blocked as i64],
    )?;
    Ok(())
}

pub fn get_jupiter_daily_stop(
    conn: &Connection,
    date: &str,
) -> Result<Option<DailyStopRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT date, start_of_day_equity_usd, blocked FROM jupiter_daily_stop WHERE date = ?1",
    )?;
    let mut rows = stmt.query_map([date], |row| {
        Ok(DailyStopRow {
            date: row.get(0)?,
            start_of_day_equity_usd: row.get(1)?,
            blocked: row.get::<_, i64>(2)? != 0,
        })
    })?;
    rows.next().transpose()
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct JupiterAuditRow {
    pub swap_id: String,
    pub created_at: String,
    pub input_mint: String,
    pub output_mint: String,
    pub input_atoms: u64,
    pub quoted_output_atoms: u64,
    pub actual_output_atoms: Option<u64>,
    pub slippage_bps: u32,
    pub price_impact_pct: f64,
    pub fee_lamports: Option<u64>,
    pub priority_fee_lamports: Option<u64>,
    pub signature: Option<String>,
    pub slot: Option<u64>,
    pub confirmed_at: Option<String>,
    pub status: String,
    pub reconciled: bool,
}

#[derive(Debug, Clone)]
pub struct DailyStopRow {
    pub date: String,
    pub start_of_day_equity_usd: f64,
    pub blocked: bool,
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

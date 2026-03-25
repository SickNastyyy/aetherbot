use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── App Config ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub paper_mode: bool,
    pub max_drawdown_pct: f64,
    pub position_size_pct: f64,
    pub stop_loss_pct: f64,
    // Engine config (with defaults for backward compat)
    #[serde(default = "default_tp_atr")]
    pub take_profit_atr_mult: f64,
    #[serde(default = "default_sl_atr")]
    pub stop_loss_atr_mult: f64,
    #[serde(default = "default_max_hold")]
    pub max_hold_hours: u64,
    #[serde(default = "default_max_positions")]
    pub max_concurrent_positions: u32,
    #[serde(default = "default_cooldown")]
    pub cooldown_minutes: u32,
    #[serde(default = "default_max_stops")]
    pub max_stops_before_pause: u32,
    #[serde(default = "default_stops_window")]
    pub stops_window_minutes: u32,
    #[serde(default = "default_max_slippage")]
    pub max_slippage_pct: f64,
    #[serde(default = "default_min_liquidity")]
    pub min_liquidity_usd: f64,
    #[serde(default = "default_paper_balance")]
    pub paper_balance: f64,
    #[serde(default = "default_tick_interval")]
    pub tick_interval_ms: u64,
}

fn default_tp_atr() -> f64 { 3.0 }
fn default_sl_atr() -> f64 { 2.0 }
fn default_max_hold() -> u64 { 24 }
fn default_max_positions() -> u32 { 5 }
fn default_cooldown() -> u32 { 30 }
fn default_max_stops() -> u32 { 3 }
fn default_stops_window() -> u32 { 60 }
fn default_max_slippage() -> f64 { 1.0 }
fn default_min_liquidity() -> f64 { 10000.0 }
fn default_paper_balance() -> f64 { 10000.0 }
fn default_tick_interval() -> u64 { 1000 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            paper_mode: true,
            max_drawdown_pct: 10.0,
            position_size_pct: 5.0,
            stop_loss_pct: 2.0,
            take_profit_atr_mult: default_tp_atr(),
            stop_loss_atr_mult: default_sl_atr(),
            max_hold_hours: default_max_hold(),
            max_concurrent_positions: default_max_positions(),
            cooldown_minutes: default_cooldown(),
            max_stops_before_pause: default_max_stops(),
            stops_window_minutes: default_stops_window(),
            max_slippage_pct: default_max_slippage(),
            min_liquidity_usd: default_min_liquidity(),
            paper_balance: default_paper_balance(),
            tick_interval_ms: default_tick_interval(),
        }
    }
}

// ── Credentials ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCredentials {
    pub exchange: String,
    pub api_key: String,
    pub secret: String,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskedCredentials {
    pub exchange: String,
    pub api_key_masked: String,
    pub has_secret: bool,
    pub has_passphrase: bool,
}

// ── System Status (legacy, for non-engine UI) ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub uptime_secs: u64,
    pub mode: String,
    pub connections: Vec<ConnectionStatus>,
    pub bot_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub name: String,
    pub connected: bool,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PnlSummary {
    pub total_pnl: f64,
    pub total_pnl_pct: f64,
    pub today_pnl: f64,
    pub win_rate: f64,
    pub total_trades: u32,
    pub active_positions: u32,
}

// ── Engine Types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BotEngineState {
    Idle,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Filled,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitReason {
    StopLoss,
    TakeProfit,
    TimeLimit,
    SignalReversal,
    Manual,
    Liquidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: i64,
    pub timeframe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub pair: String,
    pub exchange: String,
    pub direction: Direction,
    pub strength: f64,
    pub indicators: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub exchange: String,
    pub pair: String,
    pub side: Side,
    pub entry_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub unrealized_pnl: f64,
    pub unrealized_pnl_pct: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub time_limit: String,
    pub strategy: String,
    pub opened_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTick {
    pub price: f64,
    pub volume: f64,
    pub timestamp: i64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
}

// ── Events emitted to frontend ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickEvent {
    pub timestamp: String,
    pub prices: HashMap<String, f64>,
    pub positions: Vec<Position>,
    pub signals: Vec<Signal>,
    pub pnl: PnlSummary,
    pub connections: Vec<ConnectionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    pub trade_type: String,
    pub position: Position,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub state: BotEngineState,
}

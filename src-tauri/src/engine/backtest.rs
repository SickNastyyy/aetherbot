use crate::database::{self, Database, CandleRow};
use crate::models::*;
use crate::engine::signals;
use crate::engine::strategy::{Strategy, StrategyManager};
use serde::{Serialize, Deserialize};

/// Backtest configuration
#[derive(Debug, Clone, Deserialize)]
pub struct BacktestConfig {
    pub exchange: String,
    pub pair: String,
    pub initial_balance: f64,
    pub position_size_pct: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
}

/// Result of a single backtest run
#[derive(Debug, Clone, Serialize)]
pub struct BacktestResult {
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub total_pnl_pct: f64,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    pub profit_factor: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
    pub trades: Vec<BacktestTrade>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BacktestTrade {
    pub side: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub exit_reason: String,
    pub strategy: String,
    pub entry_time: i64,
    pub exit_time: i64,
}

/// Run a backtest on stored candle data
pub fn run_backtest(
    db: &Database,
    config: BacktestConfig,
) -> Result<BacktestResult, String> {
    let conn = db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;

    // Load all candles for the pair
    let candles = database::load_recent_candles(&conn, &config.exchange, &config.pair, "1m", 10000)
        .map_err(|e| format!("Failed to load candles: {e}"))?;

    if candles.len() < 50 {
        return Err(format!("Not enough candle data: have {}, need at least 50", candles.len()));
    }

    let strategy_manager = StrategyManager::new();
    let mut balance = config.initial_balance;
    let mut peak_balance = balance;
    let mut max_drawdown = 0.0f64;
    let mut trades: Vec<BacktestTrade> = Vec::new();

    // Current open position
    let mut open_position: Option<OpenPos> = None;

    // Walk through candles, building up history for indicators
    for i in 26..candles.len() {
        let window = &candles[..=i];
        let model_candles: Vec<Candle> = window.iter().map(|r| row_to_candle(r, &config.pair)).collect();
        let current = &candles[i];
        let current_price = current.close;

        // Compute signal
        let signal = match signals::compute_signal(&config.pair, &config.exchange, &model_candles) {
            Some(s) => s,
            None => continue,
        };

        // Check open position for exit
        if let Some(ref pos) = open_position {
            let pnl = match pos.side {
                Side::Long => (current_price - pos.entry_price) * pos.quantity,
                Side::Short => (pos.entry_price - current_price) * pos.quantity,
            };
            let pnl_pct = (pnl / (pos.entry_price * pos.quantity)) * 100.0;

            // Check stop-loss
            let stopped = match pos.side {
                Side::Long => pnl_pct <= -config.stop_loss_pct,
                Side::Short => pnl_pct <= -config.stop_loss_pct,
            };

            // Check take-profit
            let profit_taken = pnl_pct >= config.take_profit_pct;

            // Check strategy exit
            let strategy_exit = strategy_manager.evaluate_exit(
                &signal, &model_candles, pos.entry_price, pos.side, &pos.strategy,
            );

            if stopped || profit_taken || strategy_exit {
                let exit_reason = if stopped { "StopLoss" } else if profit_taken { "TakeProfit" } else { "Signal" };
                balance += pnl;

                trades.push(BacktestTrade {
                    side: format!("{:?}", pos.side),
                    entry_price: pos.entry_price,
                    exit_price: current_price,
                    pnl,
                    pnl_pct,
                    exit_reason: exit_reason.to_string(),
                    strategy: pos.strategy.clone(),
                    entry_time: pos.entry_time,
                    exit_time: current.timestamp,
                });

                open_position = None;

                // Track drawdown
                if balance > peak_balance { peak_balance = balance; }
                let dd = peak_balance - balance;
                if dd > max_drawdown { max_drawdown = dd; }
            }

            continue; // Don't enter new position while one is open
        }

        // Try to enter
        if let Some((intent, strat_name)) = strategy_manager.evaluate_entry(&signal, &model_candles) {
            let trade_amount = balance * (config.position_size_pct / 100.0);
            let quantity = trade_amount / current_price;

            if quantity * current_price >= 1.0 {
                open_position = Some(OpenPos {
                    side: intent.side,
                    entry_price: current_price,
                    quantity,
                    strategy: strat_name.to_string(),
                    entry_time: current.timestamp,
                });
            }
        }
    }

    // Close any remaining open position at last price
    if let Some(pos) = open_position {
        let last_price = candles.last().unwrap().close;
        let pnl = match pos.side {
            Side::Long => (last_price - pos.entry_price) * pos.quantity,
            Side::Short => (pos.entry_price - last_price) * pos.quantity,
        };
        balance += pnl;
        trades.push(BacktestTrade {
            side: format!("{:?}", pos.side),
            entry_price: pos.entry_price,
            exit_price: last_price,
            pnl,
            pnl_pct: (pnl / (pos.entry_price * pos.quantity)) * 100.0,
            exit_reason: "EndOfData".to_string(),
            strategy: pos.strategy,
            entry_time: pos.entry_time,
            exit_time: candles.last().unwrap().timestamp,
        });
    }

    // Compute statistics
    let winning: Vec<&BacktestTrade> = trades.iter().filter(|t| t.pnl > 0.0).collect();
    let losing: Vec<&BacktestTrade> = trades.iter().filter(|t| t.pnl <= 0.0).collect();
    let gross_profit: f64 = winning.iter().map(|t| t.pnl).sum();
    let gross_loss: f64 = losing.iter().map(|t| t.pnl.abs()).sum();

    let total_pnl = balance - config.initial_balance;

    Ok(BacktestResult {
        total_trades: trades.len() as u32,
        winning_trades: winning.len() as u32,
        losing_trades: losing.len() as u32,
        win_rate: if !trades.is_empty() { (winning.len() as f64 / trades.len() as f64) * 100.0 } else { 0.0 },
        total_pnl,
        total_pnl_pct: (total_pnl / config.initial_balance) * 100.0,
        max_drawdown,
        max_drawdown_pct: if peak_balance > 0.0 { (max_drawdown / peak_balance) * 100.0 } else { 0.0 },
        profit_factor: if gross_loss > 0.0 { gross_profit / gross_loss } else { f64::INFINITY },
        avg_win: if !winning.is_empty() { gross_profit / winning.len() as f64 } else { 0.0 },
        avg_loss: if !losing.is_empty() { gross_loss / losing.len() as f64 } else { 0.0 },
        best_trade: trades.iter().map(|t| t.pnl).fold(0.0f64, f64::max),
        worst_trade: trades.iter().map(|t| t.pnl).fold(0.0f64, f64::min),
        trades,
    })
}

struct OpenPos {
    side: Side,
    entry_price: f64,
    quantity: f64,
    strategy: String,
    entry_time: i64,
}

fn row_to_candle(row: &CandleRow, pair: &str) -> Candle {
    Candle {
        open: row.open,
        high: row.high,
        low: row.low,
        close: row.close,
        volume: row.volume,
        timestamp: row.timestamp,
        timeframe: "1m".to_string(),
    }
}

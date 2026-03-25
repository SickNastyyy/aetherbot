pub mod ai;
pub mod data_provider;
pub mod error;
pub mod orders;
pub mod risk;
pub mod signals;
pub mod strategy;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use tracing::{info, warn};

use tauri::Emitter;
use crate::database::{self, Database};
use crate::models::*;
use ai::evaluator::{ClaudeEvaluator, ClaudeEvaluatorConfig};
use data_provider::candles::{CandleBuilder, Timeframe};
use data_provider::jupiter::JupiterProvider;
use data_provider::uniswap::UniswapProvider;
use data_provider::kalshi::KalshiProvider;
use data_provider::DataProvider;
use orders::OrderManager;
use risk::RiskManager;
use signals::compute_signal;
use strategy::StrategyManager;

/// Exchange + pairs configuration
struct ExchangeConfig {
    name: &'static str,
    pairs: &'static [&'static str],
}

const EXCHANGES: &[ExchangeConfig] = &[
    ExchangeConfig {
        name: "Jupiter",
        pairs: &["SOL/USDC", "JUP/USDC"],
    },
    ExchangeConfig {
        name: "Uniswap",
        pairs: &["ETH/USDC", "WBTC/ETH", "LINK/USDC", "UNI/ETH", "ARB/USDC"],
    },
    ExchangeConfig {
        name: "Kalshi",
        pairs: &["Fed Rate Cut by June", "BTC above $100k by Q3", "S&P 500 above 6000 by Dec"],
    },
];

pub struct BotEngine {
    state: Arc<RwLock<BotEngineState>>,
    app_handle: tauri::AppHandle,
    config: Arc<RwLock<AppConfig>>,
    shutdown_tx: Option<watch::Sender<bool>>,
}

impl BotEngine {
    pub fn new(app_handle: tauri::AppHandle, config: AppConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(BotEngineState::Idle)),
            app_handle,
            config: Arc::new(RwLock::new(config)),
            shutdown_tx: None,
        }
    }

    pub async fn state(&self) -> BotEngineState {
        *self.state.read().await
    }

    pub async fn start(&mut self, db: std::sync::Arc<Database>) {
        let current = *self.state.read().await;
        if current == BotEngineState::Running {
            return;
        }

        *self.state.write().await = BotEngineState::Starting;
        self.emit_state(BotEngineState::Starting);
        self.emit_log("info", "Engine starting...");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        let state = self.state.clone();
        let config = self.config.clone();
        let app_handle = self.app_handle.clone();

        tokio::spawn(async move {
            *state.write().await = BotEngineState::Running;
            let _ = app_handle.emit("bot:state-change", StateChangeEvent { state: BotEngineState::Running });
            let _ = app_handle.emit("bot:log", LogEvent {
                level: "success".into(),
                message: "Engine running — scanning all exchanges".into(),
            });

            run_tick_loop(state, config, app_handle, shutdown_rx, db).await;
        });
    }

    pub async fn stop(&mut self, mode: &str) {
        *self.state.write().await = BotEngineState::Stopping;
        self.emit_state(BotEngineState::Stopping);

        match mode {
            "liquidate" => self.emit_log("warn", "Stopping — liquidating all positions..."),
            _ => self.emit_log("info", "Stopping — holding existing positions"),
        }

        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(true);
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        *self.state.write().await = BotEngineState::Idle;
        self.emit_state(BotEngineState::Idle);
        self.emit_log("success", "Engine stopped");
    }

    fn emit_state(&self, state: BotEngineState) {
        let _ = self.app_handle.emit("bot:state-change", StateChangeEvent { state });
    }

    fn emit_log(&self, level: &str, message: &str) {
        let _ = self.app_handle.emit("bot:log", LogEvent {
            level: level.into(),
            message: message.into(),
        });
    }
}

/// Provider wrapper for multi-exchange dispatch
struct Providers {
    jupiter: JupiterProvider,
    uniswap: UniswapProvider,
    kalshi: KalshiProvider,
}

impl Providers {
    fn get(&self, exchange: &str) -> &dyn DataProvider {
        match exchange {
            "Jupiter" => &self.jupiter,
            "Uniswap" => &self.uniswap,
            "Kalshi" => &self.kalshi,
            _ => &self.jupiter,
        }
    }
}

async fn run_tick_loop(
    state: Arc<RwLock<BotEngineState>>,
    config: Arc<RwLock<AppConfig>>,
    app_handle: tauri::AppHandle,
    mut shutdown_rx: watch::Receiver<bool>,
    db: std::sync::Arc<Database>,
) {
    let providers = Providers {
        jupiter: JupiterProvider::new(),
        uniswap: UniswapProvider::new(),
        kalshi: KalshiProvider::new(),
    };

    let strategy_manager = StrategyManager::new();
    let mut risk_manager = RiskManager::new();

    // Initialize Claude AI evaluator (optional — runs only if API key is configured)
    // Users can set ANTHROPIC_API_KEY env var or store it in credentials
    let claude_api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let claude_evaluator = ClaudeEvaluator::new(ClaudeEvaluatorConfig {
        api_key: claude_api_key.clone(),
        ..Default::default()
    });
    if claude_evaluator.is_configured() {
        let _ = app_handle.emit("bot:log", LogEvent {
            level: "success".into(),
            message: "Claude AI evaluator enabled — will analyze markets every 60s".into(),
        });
    } else {
        let _ = app_handle.emit("bot:log", LogEvent {
            level: "info".into(),
            message: "Claude AI evaluator not configured (set ANTHROPIC_API_KEY to enable)".into(),
        });
    }

    let initial_balance = {
        let cfg = config.read().await;
        cfg.paper_balance
    };
    let mut order_manager = OrderManager::new(initial_balance);

    // Candle builders: key = "Exchange:Pair"
    let mut candle_builders: HashMap<String, CandleBuilder> = HashMap::new();
    for exc in EXCHANGES {
        for pair in exc.pairs {
            let key = format!("{}:{}", exc.name, pair);
            candle_builders.insert(key, CandleBuilder::new(Timeframe::M1));
        }
    }

    // Load candle history from database on startup
    {
        let conn = db.conn.lock().unwrap();
        for exc in EXCHANGES {
            for pair in exc.pairs {
                let key = format!("{}:{}", exc.name, pair);
                if let Some(builder) = candle_builders.get_mut(&key) {
                    match database::load_recent_candles(&conn, exc.name, pair, "1m", 200) {
                        Ok(rows) if !rows.is_empty() => {
                            for row in &rows {
                                builder.push_tick(row.close, row.volume, row.timestamp);
                            }
                            let _ = app_handle.emit("bot:log", LogEvent {
                                level: "info".into(),
                                message: format!("[{}] Loaded {} candles for {}", exc.name, rows.len(), pair),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Bulk-fetch Uniswap prices on first tick
    if let Err(e) = providers.uniswap.refresh_all_prices().await {
        let _ = app_handle.emit("bot:log", LogEvent {
            level: "warn".into(),
            message: format!("Uniswap initial price fetch failed: {e}"),
        });
    }

    let mut tick_count: u64 = 0;

    loop {
        if *shutdown_rx.borrow() {
            info!("Shutdown signal received");
            break;
        }

        let cfg = config.read().await.clone();
        tick_count += 1;

        // Bulk refresh Uniswap prices every 5 ticks (they cache for 5s anyway)
        if tick_count % 5 == 0 {
            let _ = providers.uniswap.refresh_all_prices().await;
        }

        // ── 1. Fetch prices from all exchanges ──────────────────────────
        let mut prices: HashMap<String, f64> = HashMap::new();
        let mut all_signals: Vec<Signal> = Vec::new();
        let mut connections: Vec<ConnectionStatus> = Vec::new();

        for exc in EXCHANGES {
            let provider = providers.get(exc.name);
            let mut exchange_connected = false;
            let mut exchange_latency: Option<u64> = None;

            for pair in exc.pairs {
                let start = std::time::Instant::now();
                match tokio::time::timeout(
                    std::time::Duration::from_secs(8),
                    provider.fetch_price(pair),
                ).await {
                    Ok(Ok(tick)) => {
                        let latency = start.elapsed().as_millis() as u64;
                        prices.insert(pair.to_string(), tick.price);
                        exchange_connected = true;
                        if exchange_latency.is_none() {
                            exchange_latency = Some(latency);
                        }

                        // Push to candle builder
                        let key = format!("{}:{}", exc.name, pair);
                        if let Some(builder) = candle_builders.get_mut(&key) {
                            if let Some(candle) = builder.push_tick(tick.price, tick.volume, tick.timestamp) {
                                // Persist candle to database
                                if let Ok(conn) = db.conn.lock() {
                                    let _ = database::insert_candle(
                                        &conn, exc.name, pair, &candle.timeframe,
                                        candle.open, candle.high, candle.low, candle.close,
                                        candle.volume, candle.timestamp,
                                    );
                                }
                                let _ = app_handle.emit("bot:log", LogEvent {
                                    level: "info".into(),
                                    message: format!("[{}] {} candle closed @ ${:.4}", exc.name, pair, candle.close),
                                });
                            }

                            // Compute signals
                            let candle_slice: Vec<Candle> = builder.candles().iter().cloned().collect();
                            if exc.name == "Kalshi" {
                                // For Kalshi, create a simple signal from price data
                                // (TA indicators don't apply to probability data the same way)
                                if candle_slice.len() >= 5 {
                                    all_signals.push(Signal {
                                        pair: pair.to_string(),
                                        exchange: exc.name.to_string(),
                                        direction: Direction::Neutral,
                                        strength: 0.5,
                                        indicators: std::collections::HashMap::new(),
                                    });
                                }
                            } else if let Some(sig) = compute_signal(pair, exc.name, &candle_slice) {
                                all_signals.push(sig);
                            }
                        }

                        // Log first successful connection per exchange
                        if tick_count == 1 {
                            let _ = app_handle.emit("bot:log", LogEvent {
                                level: "success".into(),
                                message: format!("[{}] {} @ ${:.4} ({latency}ms)", exc.name, pair, tick.price),
                            });
                        }
                    }
                    Ok(Err(e)) => {
                        if tick_count <= 2 {
                            let _ = app_handle.emit("bot:log", LogEvent {
                                level: "warn".into(),
                                message: format!("[{}] Failed: {} — {}", exc.name, pair, e),
                            });
                        }
                    }
                    Err(_) => {
                        warn!("[{}] Timeout fetching {}", exc.name, pair);
                    }
                }
            }

            connections.push(ConnectionStatus {
                name: exc.name.to_string(),
                connected: exchange_connected,
                latency_ms: exchange_latency,
            });
        }

        // ── 1b. Claude AI evaluation (every 60 ticks ≈ 1 minute) ────────
        if claude_evaluator.is_configured() && tick_count % 60 == 0 && !prices.is_empty() {
            // Pick the top pair by volume/activity for Claude analysis
            for exc in EXCHANGES {
                if exc.name == "Kalshi" { continue; } // Skip prediction markets for now
                if let Some(pair) = exc.pairs.first() {
                    let key = format!("{}:{}", exc.name, pair);
                    if let Some(builder) = candle_builders.get(&key) {
                        let candle_slice: Vec<Candle> = builder.candles().iter().cloned().collect();
                        if candle_slice.len() >= 20 {
                            // Get current indicators for context
                            let indicators: HashMap<String, f64> = all_signals.iter()
                                .find(|s| s.pair == *pair && s.exchange == exc.name)
                                .map(|s| s.indicators.clone())
                                .unwrap_or_default();

                            if let Some(&current_price) = prices.get(*pair) {
                                if let Some(claude_signal) = claude_evaluator.evaluate(
                                    pair, exc.name, &candle_slice, &indicators, current_price
                                ).await {
                                    let confidence = claude_signal.indicators.get("claude_confidence")
                                        .copied().unwrap_or(0.0);

                                    let _ = app_handle.emit("bot:log", LogEvent {
                                        level: "info".into(),
                                        message: format!(
                                            "[Claude AI] {}: {:?} ({:.0}% confidence)",
                                            pair, claude_signal.direction, confidence * 100.0
                                        ),
                                    });

                                    // Claude signal overrides TA signal if high confidence
                                    if confidence >= 0.8 {
                                        // Remove existing signal for this pair and replace
                                        all_signals.retain(|s| !(s.pair == *pair && s.exchange == exc.name));
                                        all_signals.push(claude_signal);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── 2. Update position prices ───────────────────────────────────
        order_manager.update_prices(&prices);

        // ── 3. Check barriers on open positions ─────────────────────────
        let position_ids: Vec<String> = order_manager.positions().iter().map(|p| p.id.clone()).collect();
        for pos_id in &position_ids {
            let (current_price, pair, exit_reason) = {
                let pos = match order_manager.positions().iter().find(|p| p.id == *pos_id) {
                    Some(p) => p,
                    None => continue,
                };
                match risk_manager.check_barriers(pos, pos.current_price) {
                    Some(reason) => (pos.current_price, pos.pair.clone(), reason),
                    None => continue,
                }
            };

            let reason_str = format!("{:?}", exit_reason);
            if let Some((closed_pos, pnl)) = order_manager.close_position(pos_id, current_price, exit_reason) {
                if matches!(exit_reason, ExitReason::StopLoss) {
                    risk_manager.record_stop_loss(&cfg);
                }

                // Persist close to database
                if let Ok(conn) = db.conn.lock() {
                    let pnl_pct = if closed_pos.entry_price > 0.0 {
                        (pnl / (closed_pos.entry_price * closed_pos.quantity)) * 100.0
                    } else { 0.0 };
                    let _ = database::close_trade(
                        &conn, &closed_pos.id, current_price, pnl, pnl_pct,
                        &reason_str, &chrono::Utc::now().to_rfc3339(),
                    );
                }

                let pnl_str = if pnl >= 0.0 { format!("+${pnl:.2}") } else { format!("-${:.2}", pnl.abs()) };
                let level = if pnl >= 0.0 { "success" } else { "warn" };

                let _ = app_handle.emit("bot:log", LogEvent {
                    level: level.into(),
                    message: format!("Closed {pair} — {reason_str}: {pnl_str}"),
                });
            }
        }

        // ── 4. Evaluate strategy + risk check + execute ─────────────────
        if cfg.paper_mode {
            for signal in &all_signals {
                let pair = &signal.pair;

                if order_manager.positions().iter().any(|p| p.pair == *pair) {
                    continue;
                }

                let key = format!("{}:{}", signal.exchange, pair);
                let candle_slice: Vec<Candle> = candle_builders
                    .get(&key)
                    .map(|b| b.candles().iter().cloned().collect())
                    .unwrap_or_default();

                if let Some((intent, strat_name)) = strategy_manager.evaluate_entry(signal, &candle_slice) {
                    let current_price = match prices.get(pair.as_str()) {
                        Some(&p) => p,
                        None => continue,
                    };

                    match risk_manager.pre_flight_check(
                        pair,
                        &intent.exchange,
                        intent.side,
                        intent.strength,
                        current_price,
                        &candle_slice,
                        order_manager.total_equity(),
                        order_manager.open_position_count(),
                        order_manager.pnl_summary().today_pnl,
                        &cfg,
                        strat_name,
                        &intent.reason,
                    ) {
                        Ok(approved) => {
                            let result = order_manager.execute_paper_trade(&approved, current_price);
                            risk_manager.record_trade(pair);

                            // Persist trade to database
                            if let Some(pos) = order_manager.positions().last() {
                                if let Ok(conn) = db.conn.lock() {
                                    let side_str = match approved.side { Side::Long => "LONG", Side::Short => "SHORT" };
                                    let _ = database::insert_trade(
                                        &conn, &pos.id, &approved.exchange, pair, side_str,
                                        pos.entry_price, pos.quantity, strat_name,
                                        approved.stop_loss, approved.take_profit,
                                        &pos.time_limit, &pos.opened_at,
                                    );
                                }
                            }

                            let _ = app_handle.emit("bot:log", LogEvent {
                                level: "success".into(),
                                message: format!(
                                    "[{}] {} opened {:?} {} — {} (SL: ${:.2}, TP: ${:.2})",
                                    approved.exchange, strat_name, approved.side, pair, result.message,
                                    approved.stop_loss, approved.take_profit,
                                ),
                            });
                        }
                        Err(veto) => {
                            if tick_count % 60 == 0 {
                                let _ = app_handle.emit("bot:log", LogEvent {
                                    level: "info".into(),
                                    message: format!("[{}] Risk veto on {}: {}", signal.exchange, pair, veto.reason),
                                });
                            }
                        }
                    }
                }
            }
        }

        // ── 5. Emit tick event ──────────────────────────────────────────
        let pnl = order_manager.pnl_summary();
        let positions: Vec<Position> = order_manager.positions().to_vec();

        let tick_event = TickEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            prices: prices.clone(),
            positions,
            signals: all_signals,
            pnl,
            connections: connections.clone(),
        };
        let _ = app_handle.emit("bot:tick", &tick_event);

        // ── 6. Periodic portfolio snapshot ───────────────────────────────
        if tick_count % 60 == 0 {
            if let Ok(conn) = db.conn.lock() {
                let _ = database::insert_portfolio_snapshot(
                    &conn,
                    &chrono::Utc::now().to_rfc3339(),
                    order_manager.total_equity(),
                    order_manager.paper_balance(),
                    pnl.total_pnl,
                    pnl.active_positions,
                    pnl.total_trades,
                );
            }
        }

        // ── 7. Sleep until next tick ────────────────────────────────────
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(cfg.tick_interval_ms)) => {},
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() { break; }
            }
        }
    }

    *state.write().await = BotEngineState::Idle;
    let _ = app_handle.emit("bot:state-change", StateChangeEvent { state: BotEngineState::Idle });
}

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
use crate::models::*;
use data_provider::candles::{CandleBuilder, Timeframe};
use data_provider::jupiter::JupiterProvider;
use data_provider::DataProvider;
use orders::OrderManager;
use risk::RiskManager;
use signals::compute_signal;
use strategy::ema_crossover::EmaCrossover;
use strategy::Strategy;

/// Pairs to scan on Jupiter (first vertical slice)
const JUPITER_PAIRS: &[&str] = &["SOL/USDC", "JUP/USDC"];

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

    pub async fn start(&mut self) {
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
                message: "Engine running — scanning markets".into(),
            });

            run_tick_loop(state, config, app_handle, shutdown_rx).await;
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

async fn run_tick_loop(
    state: Arc<RwLock<BotEngineState>>,
    config: Arc<RwLock<AppConfig>>,
    app_handle: tauri::AppHandle,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let jupiter = JupiterProvider::new();
    let strategy = EmaCrossover::default();
    let mut risk_manager = RiskManager::new();

    let initial_balance = {
        let cfg = config.read().await;
        cfg.paper_balance
    };
    let mut order_manager = OrderManager::new(initial_balance);

    // Candle builders per pair
    let mut candle_builders: HashMap<String, CandleBuilder> = HashMap::new();
    for pair in JUPITER_PAIRS {
        candle_builders.insert(pair.to_string(), CandleBuilder::new(Timeframe::M1));
    }

    let mut tick_count: u64 = 0;

    loop {
        if *shutdown_rx.borrow() {
            info!("Shutdown signal received");
            break;
        }

        let cfg = config.read().await.clone();
        tick_count += 1;

        // ── 1. Fetch prices ─────────────────────────────────────────────
        let mut prices: HashMap<String, f64> = HashMap::new();
        let mut all_signals: Vec<Signal> = Vec::new();
        let mut connections: Vec<ConnectionStatus> = Vec::new();

        for pair in JUPITER_PAIRS {
            let start = std::time::Instant::now();
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                jupiter.fetch_price(pair),
            ).await {
                Ok(Ok(tick)) => {
                    let latency = start.elapsed().as_millis() as u64;
                    prices.insert(pair.to_string(), tick.price);

                    if tick_count <= 2 {
                        connections.push(ConnectionStatus {
                            name: "Jupiter".into(),
                            connected: true,
                            latency_ms: Some(latency),
                        });
                    }

                    // Push to candle builder
                    if let Some(builder) = candle_builders.get_mut(*pair) {
                        let closed = builder.push_tick(tick.price, tick.volume, tick.timestamp);
                        if let Some(candle) = closed {
                            let _ = app_handle.emit("bot:log", LogEvent {
                                level: "info".into(),
                                message: format!("{} candle closed @ ${:.4}", pair, candle.close),
                            });
                        }

                        // Compute signal
                        let candle_slice: Vec<Candle> = builder.candles().iter().cloned().collect();
                        if let Some(sig) = compute_signal(pair, "Jupiter", &candle_slice) {
                            all_signals.push(sig);
                        }
                    }

                    // First tick — log connection
                    if tick_count == 1 {
                        let _ = app_handle.emit("bot:log", LogEvent {
                            level: "success".into(),
                            message: format!("Jupiter connected — {pair} @ ${:.4} ({latency}ms)", tick.price),
                        });
                    }
                }
                Ok(Err(e)) => {
                    if tick_count <= 3 {
                        let _ = app_handle.emit("bot:log", LogEvent {
                            level: "error".into(),
                            message: format!("Failed to fetch {pair}: {e}"),
                        });
                    }
                }
                Err(_) => {
                    warn!("Timeout fetching {}", pair);
                }
            }
        }

        // Placeholder connections for unimplemented exchanges
        if tick_count <= 2 {
            connections.push(ConnectionStatus { name: "Uniswap".into(), connected: false, latency_ms: None });
            connections.push(ConnectionStatus { name: "Kalshi".into(), connected: false, latency_ms: None });
        }

        // ── 2. Update position prices ───────────────────────────────────
        order_manager.update_prices(&prices);

        // ── 3. Check barriers on open positions ─────────────────────────
        let position_ids: Vec<String> = order_manager.positions().iter().map(|p| p.id.clone()).collect();
        for pos_id in &position_ids {
            if let Some(pos) = order_manager.positions().iter().find(|p| p.id == *pos_id) {
                if let Some(exit_reason) = risk_manager.check_barriers(pos, pos.current_price) {
                    let pair = pos.pair.clone();
                    let reason_str = format!("{:?}", exit_reason);

                    if let Some((closed_pos, pnl)) = order_manager.close_position(pos_id, pos.current_price, exit_reason) {
                        if matches!(exit_reason, ExitReason::StopLoss) {
                            risk_manager.record_stop_loss(&cfg);
                        }

                        let pnl_str = if pnl >= 0.0 { format!("+${pnl:.2}") } else { format!("-${:.2}", pnl.abs()) };
                        let level = if pnl >= 0.0 { "success" } else { "warn" };

                        let _ = app_handle.emit("bot:log", LogEvent {
                            level: level.into(),
                            message: format!("Closed {} — {reason_str}: {pnl_str}", pair),
                        });

                        let _ = app_handle.emit("bot:trade", TradeEvent {
                            trade_type: "close".into(),
                            position: closed_pos,
                            reason: reason_str,
                        });
                    }
                }
            }
        }

        // ── 4. Evaluate strategy + risk check + execute ─────────────────
        if cfg.paper_mode {
            for signal in &all_signals {
                let pair = &signal.pair;

                // Skip if already have a position in this pair
                if order_manager.positions().iter().any(|p| p.pair == *pair) {
                    continue;
                }

                let candle_slice: Vec<Candle> = candle_builders
                    .get(pair.as_str())
                    .map(|b| b.candles().iter().cloned().collect())
                    .unwrap_or_default();

                if let Some(intent) = strategy.should_enter(signal, &candle_slice) {
                    let current_price = match prices.get(pair.as_str()) {
                        Some(&p) => p,
                        None => continue,
                    };

                    // Risk check
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
                        strategy.name(),
                        &intent.reason,
                    ) {
                        Ok(approved) => {
                            let result = order_manager.execute_paper_trade(&approved, current_price);
                            risk_manager.record_trade(pair);

                            let _ = app_handle.emit("bot:log", LogEvent {
                                level: "success".into(),
                                message: format!(
                                    "Opened {:?} {} — {} (SL: ${:.2}, TP: ${:.2})",
                                    approved.side, pair, result.message,
                                    approved.stop_loss, approved.take_profit,
                                ),
                            });

                            if let Some(pos) = order_manager.positions().last() {
                                let _ = app_handle.emit("bot:trade", TradeEvent {
                                    trade_type: "open".into(),
                                    position: pos.clone(),
                                    reason: approved.reason,
                                });
                            }
                        }
                        Err(veto) => {
                            // Only log vetoes occasionally to avoid spam
                            if tick_count % 30 == 0 {
                                let _ = app_handle.emit("bot:log", LogEvent {
                                    level: "info".into(),
                                    message: format!("Risk veto on {pair}: {}", veto.reason),
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
            connections: if tick_count <= 2 { connections } else { vec![] },
        };
        let _ = app_handle.emit("bot:tick", &tick_event);

        // ── 6. Sleep until next tick ────────────────────────────────────
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

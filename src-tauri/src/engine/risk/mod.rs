use std::collections::VecDeque;
use std::time::Instant;
use tracing::warn;

use crate::models::*;
use crate::engine::signals::indicators;

/// A trade that passed all risk checks and is ready for execution
#[derive(Debug, Clone)]
pub struct ApprovedTrade {
    pub pair: String,
    pub exchange: String,
    pub side: Side,
    pub quantity: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub time_limit_hours: u64,
    pub strategy: String,
    pub reason: String,
}

/// Why a trade was rejected
#[derive(Debug, Clone)]
pub struct RiskVeto {
    pub reason: String,
}

pub struct RiskManager {
    /// Timestamps of recent stop-loss exits (for StoplossGuard)
    recent_stops: VecDeque<Instant>,
    /// If set, no new trades until this time
    circuit_breaker_until: Option<Instant>,
    /// Timestamps of recent trades per pair (for CooldownPeriod)
    pair_cooldowns: std::collections::HashMap<String, Instant>,
}

impl RiskManager {
    pub fn new() -> Self {
        Self {
            recent_stops: VecDeque::new(),
            circuit_breaker_until: None,
            pair_cooldowns: std::collections::HashMap::new(),
        }
    }

    /// Main pre-flight check. Returns ApprovedTrade or RiskVeto.
    pub fn pre_flight_check(
        &self,
        pair: &str,
        exchange: &str,
        side: Side,
        signal_strength: f64,
        current_price: f64,
        candles: &[Candle],
        portfolio_equity: f64,
        open_position_count: u32,
        daily_pnl: f64,
        config: &AppConfig,
        strategy_name: &str,
        reason: &str,
    ) -> Result<ApprovedTrade, RiskVeto> {
        // 1. Circuit breaker active?
        if let Some(until) = self.circuit_breaker_until {
            if Instant::now() < until {
                return Err(RiskVeto {
                    reason: "Circuit breaker active — trading paused".into(),
                });
            }
        }

        // 2. Max drawdown check
        let drawdown_pct = if portfolio_equity > 0.0 {
            (-daily_pnl / portfolio_equity) * 100.0
        } else {
            0.0
        };
        if drawdown_pct > config.max_drawdown_pct {
            return Err(RiskVeto {
                reason: format!(
                    "Daily drawdown {drawdown_pct:.1}% exceeds limit {:.1}%",
                    config.max_drawdown_pct
                ),
            });
        }

        // 3. Max concurrent positions
        if open_position_count >= config.max_concurrent_positions {
            return Err(RiskVeto {
                reason: format!(
                    "Max positions reached ({}/{})",
                    open_position_count, config.max_concurrent_positions
                ),
            });
        }

        // 4. Pair cooldown
        if let Some(last_trade) = self.pair_cooldowns.get(pair) {
            let cooldown_secs = config.cooldown_minutes as u64 * 60;
            if last_trade.elapsed().as_secs() < cooldown_secs {
                let remaining = cooldown_secs - last_trade.elapsed().as_secs();
                return Err(RiskVeto {
                    reason: format!("{pair} on cooldown ({remaining}s remaining)"),
                });
            }
        }

        // 5. Compute ATR-based stop-loss and take-profit
        let atr_value = if !candles.is_empty() {
            indicators::atr(candles, 14)
                .and_then(|v| v.last().copied())
                .unwrap_or(current_price * config.stop_loss_pct / 100.0)
        } else {
            current_price * config.stop_loss_pct / 100.0
        };

        let (stop_loss, take_profit) = match side {
            Side::Long => (
                current_price - (atr_value * config.stop_loss_atr_mult),
                current_price + (atr_value * config.take_profit_atr_mult),
            ),
            Side::Short => (
                current_price + (atr_value * config.stop_loss_atr_mult),
                current_price - (atr_value * config.take_profit_atr_mult),
            ),
        };

        // 6. Position sizing (% of equity, capped by config)
        let risk_amount = portfolio_equity * (config.position_size_pct / 100.0);
        let stop_distance = (current_price - stop_loss).abs();
        let quantity = if stop_distance > 0.0 {
            (risk_amount / stop_distance).min(risk_amount / current_price)
        } else {
            risk_amount / current_price
        };

        if quantity * current_price < 1.0 {
            return Err(RiskVeto {
                reason: "Position size too small (<$1)".into(),
            });
        }

        Ok(ApprovedTrade {
            pair: pair.to_string(),
            exchange: exchange.to_string(),
            side,
            quantity,
            entry_price: current_price,
            stop_loss,
            take_profit,
            time_limit_hours: config.max_hold_hours,
            strategy: strategy_name.to_string(),
            reason: reason.to_string(),
        })
    }

    /// Check triple barrier on an open position. Returns Some(ExitReason) if should exit.
    pub fn check_barriers(
        &self,
        position: &Position,
        current_price: f64,
    ) -> Option<ExitReason> {
        // Stop-loss
        match position.side {
            Side::Long => {
                if current_price <= position.stop_loss {
                    return Some(ExitReason::StopLoss);
                }
            }
            Side::Short => {
                if current_price >= position.stop_loss {
                    return Some(ExitReason::StopLoss);
                }
            }
        }

        // Take-profit
        match position.side {
            Side::Long => {
                if current_price >= position.take_profit {
                    return Some(ExitReason::TakeProfit);
                }
            }
            Side::Short => {
                if current_price <= position.take_profit {
                    return Some(ExitReason::TakeProfit);
                }
            }
        }

        // Time limit
        if let Ok(opened) = chrono::DateTime::parse_from_rfc3339(&position.time_limit) {
            if chrono::Utc::now() > opened {
                return Some(ExitReason::TimeLimit);
            }
        }

        None
    }

    /// Record a stop-loss exit. May trigger StoplossGuard circuit breaker.
    pub fn record_stop_loss(&mut self, config: &AppConfig) {
        self.recent_stops.push_back(Instant::now());

        // Clean old stops outside the window
        let window = std::time::Duration::from_secs(config.stops_window_minutes as u64 * 60);
        while let Some(front) = self.recent_stops.front() {
            if front.elapsed() > window {
                self.recent_stops.pop_front();
            } else {
                break;
            }
        }

        // Check if threshold exceeded
        if self.recent_stops.len() >= config.max_stops_before_pause as usize {
            let cooldown = std::time::Duration::from_secs(config.cooldown_minutes as u64 * 60);
            self.circuit_breaker_until = Some(Instant::now() + cooldown);
            warn!(
                "StoplossGuard triggered: {} stops in {}min window — pausing for {}min",
                self.recent_stops.len(),
                config.stops_window_minutes,
                config.cooldown_minutes
            );
        }
    }

    /// Record that a trade was made on a pair (for cooldown tracking)
    pub fn record_trade(&mut self, pair: &str) {
        self.pair_cooldowns.insert(pair.to_string(), Instant::now());
    }

    /// Check if circuit breaker is currently active
    pub fn is_circuit_breaker_active(&self) -> bool {
        match self.circuit_breaker_until {
            Some(until) => Instant::now() < until,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppConfig, Candle, ExitReason, Position, Side};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn default_config() -> AppConfig {
        AppConfig {
            paper_mode: true,
            max_drawdown_pct: 10.0,
            position_size_pct: 5.0,
            stop_loss_pct: 2.0,
            take_profit_atr_mult: 3.0,
            stop_loss_atr_mult: 2.0,
            max_hold_hours: 24,
            max_concurrent_positions: 5,
            cooldown_minutes: 0, // no cooldown for tests
            max_stops_before_pause: 3,
            stops_window_minutes: 60,
            max_slippage_pct: 1.0,
            min_liquidity_usd: 10_000.0,
            paper_balance: 10_000.0,
            tick_interval_ms: 1000,
        }
    }

    fn make_candles(n: usize, price: f64) -> Vec<Candle> {
        (0..n).map(|_| Candle {
            open: price,
            high: price * 1.02,
            low: price * 0.98,
            close: price,
            volume: 1000.0,
            timestamp: 0,
            timeframe: "1m".into(),
        }).collect()
    }

    fn make_position(side: Side, entry: f64, stop: f64, tp: f64) -> Position {
        Position {
            id: "test-1".into(),
            exchange: "paper".into(),
            pair: "BTC/USD".into(),
            side,
            entry_price: entry,
            current_price: entry,
            quantity: 1.0,
            unrealized_pnl: 0.0,
            unrealized_pnl_pct: 0.0,
            stop_loss: stop,
            take_profit: tp,
            // Set time_limit far in the future
            time_limit: (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339(),
            strategy: "test".into(),
            opened_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn run_preflight(
        rm: &RiskManager,
        config: &AppConfig,
        side: Side,
        portfolio_equity: f64,
        open_positions: u32,
        daily_pnl: f64,
    ) -> Result<ApprovedTrade, RiskVeto> {
        let candles = make_candles(20, 1000.0);
        rm.pre_flight_check(
            "BTC/USD",
            "paper",
            side,
            0.8,
            1000.0,
            &candles,
            portfolio_equity,
            open_positions,
            daily_pnl,
            config,
            "test_strategy",
            "test signal",
        )
    }

    // ── pre_flight_check — happy path ─────────────────────────────────────────

    #[test]
    fn preflight_approves_valid_trade() {
        let rm = RiskManager::new();
        let config = default_config();
        let result = run_preflight(&rm, &config, Side::Long, 10_000.0, 0, 0.0);
        assert!(result.is_ok(), "valid trade should be approved: {:?}", result);
    }

    #[test]
    fn preflight_approved_trade_has_correct_side() {
        let rm = RiskManager::new();
        let config = default_config();
        let trade = run_preflight(&rm, &config, Side::Short, 10_000.0, 0, 0.0).unwrap();
        assert_eq!(trade.side, Side::Short);
    }

    // ── pre_flight_check — max drawdown ───────────────────────────────────────

    #[test]
    fn preflight_rejects_when_max_drawdown_exceeded() {
        let rm = RiskManager::new();
        let mut config = default_config();
        config.max_drawdown_pct = 5.0;
        // daily_pnl = -600, equity = 10_000 → drawdown = 6% > 5%
        let result = run_preflight(&rm, &config, Side::Long, 10_000.0, 0, -600.0);
        assert!(result.is_err(), "should reject when drawdown exceeds limit");
        let veto = result.unwrap_err();
        assert!(veto.reason.contains("drawdown"), "veto reason should mention drawdown: {}", veto.reason);
    }

    #[test]
    fn preflight_allows_at_exactly_drawdown_limit() {
        let rm = RiskManager::new();
        let mut config = default_config();
        config.max_drawdown_pct = 5.0;
        // daily_pnl = -500, equity = 10_000 → drawdown = 5.0% == limit (not strictly greater)
        let result = run_preflight(&rm, &config, Side::Long, 10_000.0, 0, -500.0);
        // 5.0 > 5.0 is false, so the trade should be approved
        assert!(result.is_ok(), "should allow when drawdown equals (not exceeds) limit");
    }

    // ── pre_flight_check — max positions ─────────────────────────────────────

    #[test]
    fn preflight_rejects_when_max_positions_reached() {
        let rm = RiskManager::new();
        let mut config = default_config();
        config.max_concurrent_positions = 3;
        let result = run_preflight(&rm, &config, Side::Long, 10_000.0, 3, 0.0);
        assert!(result.is_err(), "should reject when at max positions");
        let veto = result.unwrap_err();
        assert!(veto.reason.contains("Max positions"), "reason: {}", veto.reason);
    }

    #[test]
    fn preflight_allows_one_below_max_positions() {
        let rm = RiskManager::new();
        let mut config = default_config();
        config.max_concurrent_positions = 3;
        let result = run_preflight(&rm, &config, Side::Long, 10_000.0, 2, 0.0);
        assert!(result.is_ok());
    }

    // ── pre_flight_check — position too small ─────────────────────────────────

    #[test]
    fn preflight_rejects_position_too_small() {
        let rm = RiskManager::new();
        let mut config = default_config();
        // Tiny equity: 5% of $0.10 = $0.005 → way below $1 minimum
        config.position_size_pct = 5.0;
        let candles = make_candles(20, 1000.0);
        let result = rm.pre_flight_check(
            "BTC/USD", "paper", Side::Long, 0.8, 1000.0,
            &candles, 0.10, 0, 0.0,
            &config, "test", "test",
        );
        assert!(result.is_err(), "tiny position should be rejected");
        let veto = result.unwrap_err();
        assert!(veto.reason.contains("too small"), "reason: {}", veto.reason);
    }

    // ── pre_flight_check — ATR stop placement ─────────────────────────────────

    #[test]
    fn stop_loss_below_entry_for_long() {
        let rm = RiskManager::new();
        let config = default_config();
        let trade = run_preflight(&rm, &config, Side::Long, 10_000.0, 0, 0.0).unwrap();
        assert!(
            trade.stop_loss < trade.entry_price,
            "stop_loss {} must be below entry {} for long",
            trade.stop_loss, trade.entry_price
        );
    }

    #[test]
    fn stop_loss_above_entry_for_short() {
        let rm = RiskManager::new();
        let config = default_config();
        let trade = run_preflight(&rm, &config, Side::Short, 10_000.0, 0, 0.0).unwrap();
        assert!(
            trade.stop_loss > trade.entry_price,
            "stop_loss {} must be above entry {} for short",
            trade.stop_loss, trade.entry_price
        );
    }

    #[test]
    fn take_profit_above_entry_for_long() {
        let rm = RiskManager::new();
        let config = default_config();
        let trade = run_preflight(&rm, &config, Side::Long, 10_000.0, 0, 0.0).unwrap();
        assert!(trade.take_profit > trade.entry_price);
    }

    #[test]
    fn take_profit_below_entry_for_short() {
        let rm = RiskManager::new();
        let config = default_config();
        let trade = run_preflight(&rm, &config, Side::Short, 10_000.0, 0, 0.0).unwrap();
        assert!(trade.take_profit < trade.entry_price);
    }

    // ── check_barriers — stop-loss ────────────────────────────────────────────

    #[test]
    fn check_barriers_triggers_stop_loss_for_long() {
        let rm = RiskManager::new();
        let pos = make_position(Side::Long, 1000.0, 950.0, 1100.0);
        // Price hits below stop
        let exit = rm.check_barriers(&pos, 940.0);
        assert_eq!(exit, Some(ExitReason::StopLoss));
    }

    #[test]
    fn check_barriers_triggers_stop_loss_at_exact_level_for_long() {
        let rm = RiskManager::new();
        let pos = make_position(Side::Long, 1000.0, 950.0, 1100.0);
        let exit = rm.check_barriers(&pos, 950.0);
        assert_eq!(exit, Some(ExitReason::StopLoss));
    }

    #[test]
    fn check_barriers_triggers_stop_loss_for_short() {
        let rm = RiskManager::new();
        let pos = make_position(Side::Short, 1000.0, 1050.0, 900.0);
        // Price rises above stop
        let exit = rm.check_barriers(&pos, 1060.0);
        assert_eq!(exit, Some(ExitReason::StopLoss));
    }

    // ── check_barriers — take-profit ──────────────────────────────────────────

    #[test]
    fn check_barriers_triggers_take_profit_for_long() {
        let rm = RiskManager::new();
        let pos = make_position(Side::Long, 1000.0, 950.0, 1100.0);
        let exit = rm.check_barriers(&pos, 1100.0);
        assert_eq!(exit, Some(ExitReason::TakeProfit));
    }

    #[test]
    fn check_barriers_triggers_take_profit_for_short() {
        let rm = RiskManager::new();
        let pos = make_position(Side::Short, 1000.0, 1050.0, 900.0);
        let exit = rm.check_barriers(&pos, 890.0);
        assert_eq!(exit, Some(ExitReason::TakeProfit));
    }

    // ── check_barriers — no exit ──────────────────────────────────────────────

    #[test]
    fn check_barriers_no_exit_when_price_inside_bands() {
        let rm = RiskManager::new();
        let pos = make_position(Side::Long, 1000.0, 950.0, 1100.0);
        let exit = rm.check_barriers(&pos, 1020.0);
        assert!(exit.is_none(), "price inside bands should not trigger exit");
    }

    #[test]
    fn check_barriers_time_limit_triggers_on_expired_position() {
        let rm = RiskManager::new();
        let mut pos = make_position(Side::Long, 1000.0, 950.0, 1100.0);
        // Set time_limit to the past
        pos.time_limit = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let exit = rm.check_barriers(&pos, 1020.0); // price is fine, time is not
        assert_eq!(exit, Some(ExitReason::TimeLimit));
    }

    // ── circuit breaker ───────────────────────────────────────────────────────

    #[test]
    fn circuit_breaker_not_active_by_default() {
        let rm = RiskManager::new();
        assert!(!rm.is_circuit_breaker_active());
    }

    #[test]
    fn circuit_breaker_activates_after_n_stops() {
        let mut rm = RiskManager::new();
        let mut config = default_config();
        config.max_stops_before_pause = 3;
        config.cooldown_minutes = 30;

        rm.record_stop_loss(&config);
        assert!(!rm.is_circuit_breaker_active(), "1 stop: no breaker");
        rm.record_stop_loss(&config);
        assert!(!rm.is_circuit_breaker_active(), "2 stops: no breaker");
        rm.record_stop_loss(&config);
        assert!(rm.is_circuit_breaker_active(), "3 stops: breaker should trip");
    }

    #[test]
    fn circuit_breaker_blocks_new_trades() {
        let mut rm = RiskManager::new();
        let mut config = default_config();
        config.max_stops_before_pause = 2;
        config.cooldown_minutes = 30;

        rm.record_stop_loss(&config);
        rm.record_stop_loss(&config);
        assert!(rm.is_circuit_breaker_active());

        let result = run_preflight(&rm, &config, Side::Long, 10_000.0, 0, 0.0);
        assert!(result.is_err(), "circuit breaker should block new trades");
        assert!(result.unwrap_err().reason.contains("Circuit breaker"));
    }

    // ── record_trade / cooldown ───────────────────────────────────────────────

    #[test]
    fn record_trade_inserts_pair_cooldown() {
        let mut rm = RiskManager::new();
        rm.record_trade("BTC/USD");
        assert!(rm.pair_cooldowns.contains_key("BTC/USD"));
    }

    #[test]
    fn preflight_rejects_pair_under_cooldown() {
        let mut rm = RiskManager::new();
        let mut config = default_config();
        config.cooldown_minutes = 30;

        rm.record_trade("BTC/USD");

        let candles = make_candles(20, 1000.0);
        let result = rm.pre_flight_check(
            "BTC/USD", "paper", Side::Long, 0.8, 1000.0,
            &candles, 10_000.0, 0, 0.0,
            &config, "test", "test",
        );
        assert!(result.is_err(), "pair under cooldown should be rejected");
        assert!(result.unwrap_err().reason.contains("cooldown"));
    }
}

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

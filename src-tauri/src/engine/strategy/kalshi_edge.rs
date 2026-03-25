use crate::models::{Signal, Candle, Direction, Side};
use super::{Strategy, TradeIntent};

/// Kalshi Probability Edge Strategy
///
/// For prediction markets, "price" = probability (0.00 to 1.00).
/// This strategy looks for mispricings where the market price
/// diverges from an estimated fair value based on recent momentum.
///
/// Key insight: Kalshi markets exhibit momentum — when a probability
/// is trending in one direction, it tends to continue. This strategy
/// buys YES when probability is rising and still below recent highs,
/// sells YES (buys NO) when falling and still above recent lows.
///
/// Uses Kelly Criterion for position sizing hint via signal strength.
pub struct KalshiEdge {
    /// Minimum price movement (in probability points) to trigger
    pub min_move: f64,
    /// Number of candles to look back for trend
    pub lookback: usize,
    /// Don't trade if price is in dead zone (0.10 - 0.90 only)
    pub min_price: f64,
    pub max_price: f64,
}

impl Default for KalshiEdge {
    fn default() -> Self {
        Self {
            min_move: 0.03, // 3 cent minimum move
            lookback: 10,
            min_price: 0.10,
            max_price: 0.90,
        }
    }
}

impl Strategy for KalshiEdge {
    fn name(&self) -> &str {
        "Kalshi Edge"
    }

    fn should_enter(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Option<TradeIntent> {
        // Only works on Kalshi
        if signal.exchange != "Kalshi" {
            return None;
        }

        if candles.len() < self.lookback {
            return None;
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let current = *closes.last()?;

        // Skip if outside tradeable range
        if current < self.min_price || current > self.max_price {
            return None;
        }

        // Calculate momentum over lookback period
        let lookback_start = closes[closes.len() - self.lookback];
        let momentum = current - lookback_start;

        // Calculate recent high and low
        let recent = &closes[closes.len() - self.lookback..];
        let recent_high = recent.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let recent_low = recent.iter().cloned().fold(f64::INFINITY, f64::min);
        let range = recent_high - recent_low;

        if range < self.min_move {
            return None; // Not enough movement to trade
        }

        // Kelly-inspired edge calculation
        // Estimated edge = momentum / range (how strong is the trend relative to noise)
        let edge = (momentum.abs() / range).min(1.0);

        if edge < 0.3 {
            return None; // Edge too small
        }

        if momentum > self.min_move && current < recent_high {
            // Probability rising, hasn't peaked — buy YES
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: edge,
                reason: format!(
                    "Kalshi momentum: +{:.1}% over {} periods, edge {:.0}% (buy YES at {:.2})",
                    momentum * 100.0, self.lookback, edge * 100.0, current
                ),
            })
        } else if momentum < -self.min_move && current > recent_low {
            // Probability falling, hasn't bottomed — buy NO (short YES)
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: edge,
                reason: format!(
                    "Kalshi momentum: {:.1}% over {} periods, edge {:.0}% (buy NO at {:.2})",
                    momentum * 100.0, self.lookback, edge * 100.0, current
                ),
            })
        } else {
            None
        }
    }

    fn should_exit(
        &self,
        _signal: &Signal,
        candles: &[Candle],
        entry_price: f64,
        side: Side,
    ) -> bool {
        if candles.len() < 3 { return false; }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let current = *closes.last().unwrap();

        // Exit if momentum reversed
        let prev = closes[closes.len() - 3];
        let short_momentum = current - prev;

        match side {
            Side::Long => {
                // Exit long (YES) if momentum flipped negative or target hit
                short_momentum < -0.02 || current >= entry_price + 0.08
            }
            Side::Short => {
                // Exit short (NO) if momentum flipped positive or target hit
                short_momentum > 0.02 || current <= entry_price - 0.08
            }
        }
    }
}

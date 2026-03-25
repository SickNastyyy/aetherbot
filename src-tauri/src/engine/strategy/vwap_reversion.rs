use crate::models::{Signal, Candle, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// VWAP Reversion Strategy
/// Trades mean reversion when price deviates significantly from VWAP.
pub struct VwapReversion {
    pub deviation_threshold: f64,
    pub confirmation_candles: usize,
}

impl Default for VwapReversion {
    fn default() -> Self {
        Self {
            deviation_threshold: 1.5, // % deviation from VWAP
            confirmation_candles: 2,
        }
    }
}

impl Strategy for VwapReversion {
    fn name(&self) -> &str { "VWAP Reversion" }

    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent> {
        if candles.len() < 30 { return None; }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let vwap_vals = indicators::vwap(candles, 1440)?;
        let n = closes.len();

        let current = closes[n - 1];
        let vwap = *vwap_vals.last()?;
        let deviation_pct = ((current - vwap) / vwap) * 100.0;

        if deviation_pct.abs() < self.deviation_threshold { return None; }

        // Check for reversion (price moving back toward VWAP)
        let reverting = if self.confirmation_candles <= n {
            let start = n - self.confirmation_candles;
            if deviation_pct < 0.0 {
                // Below VWAP — check for consecutive higher closes
                (start..n - 1).all(|i| closes[i + 1] > closes[i])
            } else {
                // Above VWAP — check for consecutive lower closes
                (start..n - 1).all(|i| closes[i + 1] < closes[i])
            }
        } else {
            false
        };

        if !reverting { return None; }

        let strength = (deviation_pct.abs() / 5.0).min(1.0) * 0.8;

        if deviation_pct < -self.deviation_threshold {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength,
                reason: format!("VWAP Reversion: {deviation_pct:.2}% below VWAP, reverting up"),
            })
        } else if deviation_pct > self.deviation_threshold {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength,
                reason: format!("VWAP Reversion: +{deviation_pct:.2}% above VWAP, reverting down"),
            })
        } else {
            None
        }
    }

    fn should_exit(&self, _signal: &Signal, candles: &[Candle], _entry_price: f64, _side: Side) -> bool {
        if candles.len() < 10 { return false; }
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let vwap_vals = match indicators::vwap(candles, 1440) {
            Some(v) => v, None => return false,
        };
        let current = *closes.last().unwrap();
        let vwap = *vwap_vals.last().unwrap();

        // Exit when price crosses back through VWAP
        let deviation_pct = ((current - vwap) / vwap).abs() * 100.0;
        deviation_pct < 0.3 // within 0.3% of VWAP = close enough
    }
}

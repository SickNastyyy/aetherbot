use crate::models::{Signal, Candle, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// Volume Profile Strategy
/// Trades bounces at Value Area boundaries (support/resistance from volume data).
pub struct VolumeProfileStrategy {
    pub lookback_candles: usize,
    pub proximity_pct: f64,
    pub num_bins: usize,
}

impl Default for VolumeProfileStrategy {
    fn default() -> Self {
        Self {
            lookback_candles: 120,
            proximity_pct: 0.5,
            num_bins: 24,
        }
    }
}

impl Strategy for VolumeProfileStrategy {
    fn name(&self) -> &str { "Volume Profile" }

    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent> {
        if candles.len() < self.lookback_candles { return None; }

        let window = &candles[candles.len() - self.lookback_candles..];
        let vp = indicators::volume_profile(window, self.num_bins)?;

        let current = candles.last()?.close;
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let rsi_vals = indicators::rsi(&closes, 14)?;
        let rsi = *rsi_vals.last()?;

        // Distance from Value Area boundaries
        let dist_to_val = ((current - vp.value_area_low) / current * 100.0).abs();
        let dist_to_vah = ((current - vp.value_area_high) / current * 100.0).abs();

        // Buy near Value Area Low with oversold RSI
        if dist_to_val < self.proximity_pct && current > vp.value_area_low && rsi < 45.0 {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: 0.7,
                reason: format!(
                    "Vol Profile: near VAL ${:.4} (dist {dist_to_val:.2}%), RSI={rsi:.1}, POC=${:.4}",
                    vp.value_area_low, vp.poc
                ),
            })
        }
        // Sell near Value Area High with overbought RSI
        else if dist_to_vah < self.proximity_pct && current < vp.value_area_high && rsi > 55.0 {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: 0.7,
                reason: format!(
                    "Vol Profile: near VAH ${:.4} (dist {dist_to_vah:.2}%), RSI={rsi:.1}, POC=${:.4}",
                    vp.value_area_high, vp.poc
                ),
            })
        } else {
            None
        }
    }

    fn should_exit(&self, _signal: &Signal, candles: &[Candle], _entry_price: f64, side: Side) -> bool {
        if candles.len() < self.lookback_candles { return false; }
        let window = &candles[candles.len() - self.lookback_candles..];
        let vp = match indicators::volume_profile(window, self.num_bins) {
            Some(v) => v, None => return false,
        };
        let current = candles.last().unwrap().close;

        // Exit at POC (mean reversion target)
        match side {
            Side::Long => current >= vp.poc,
            Side::Short => current <= vp.poc,
        }
    }
}

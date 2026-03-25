use crate::models::{Signal, Candle, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// Grid Trading Strategy
/// Places entries at ATR-spaced levels in ranging markets (low BB bandwidth).
pub struct GridTrading {
    pub grid_spacing_atr_mult: f64,
    pub bb_period: usize,
    pub bandwidth_threshold: f64,
    pub min_range_candles: usize,
}

impl Default for GridTrading {
    fn default() -> Self {
        Self {
            grid_spacing_atr_mult: 0.5,
            bb_period: 20,
            bandwidth_threshold: 0.04, // 4% bandwidth = ranging
            min_range_candles: 10,
        }
    }
}

impl Strategy for GridTrading {
    fn name(&self) -> &str { "Grid" }

    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        if closes.len() < self.bb_period + self.min_range_candles { return None; }

        // Check if market is ranging (low bandwidth)
        let bw = indicators::bollinger_bandwidth(&closes, self.bb_period, 2.0)?;
        let recent_bw = &bw[bw.len().saturating_sub(self.min_range_candles)..];
        let avg_bw: f64 = recent_bw.iter().sum::<f64>() / recent_bw.len() as f64;

        if avg_bw > self.bandwidth_threshold { return None; } // Not ranging

        let atr_vals = indicators::atr(candles, 14)?;
        let atr = *atr_vals.last()?;
        let current_price = *closes.last()?;

        // Get Bollinger bands for range boundaries
        let (upper, middle, lower) = indicators::bollinger_bands(&closes, self.bb_period, 2.0)?;
        let mid = *middle.last()?;
        let lo = *lower.last()?;
        let hi = *upper.last()?;

        let grid_step = atr * self.grid_spacing_atr_mult;

        // Buy near lower band, sell near upper band
        if current_price < mid - grid_step && current_price > lo {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: signal.strength * 0.8,
                reason: format!("Grid: buy at ${current_price:.4} (range {lo:.4}-{hi:.4}, bw={avg_bw:.3})"),
            })
        } else if current_price > mid + grid_step && current_price < hi {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: signal.strength * 0.8,
                reason: format!("Grid: sell at ${current_price:.4} (range {lo:.4}-{hi:.4}, bw={avg_bw:.3})"),
            })
        } else {
            None
        }
    }

    fn should_exit(&self, _signal: &Signal, candles: &[Candle], _entry_price: f64, side: Side) -> bool {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let bw = match indicators::bollinger_bandwidth(&closes, self.bb_period, 2.0) {
            Some(v) => v, None => return false,
        };
        let (_, middle, _) = match indicators::bollinger_bands(&closes, self.bb_period, 2.0) {
            Some(v) => v, None => return false,
        };
        let mid = *middle.last().unwrap();
        let current = *closes.last().unwrap();

        // Exit if bandwidth expands (breakout) or price reaches middle
        let bandwidth_expanding = *bw.last().unwrap() > self.bandwidth_threshold * 1.5;
        let at_mid = match side {
            Side::Long => current >= mid,
            Side::Short => current <= mid,
        };
        bandwidth_expanding || at_mid
    }
}

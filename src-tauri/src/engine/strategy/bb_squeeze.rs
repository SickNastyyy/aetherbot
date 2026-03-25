use crate::models::{Signal, Candle, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// Bollinger Band Squeeze Strategy
/// Detects low-volatility squeezes and trades the breakout.
pub struct BollingerSqueeze {
    pub bb_period: usize,
    pub squeeze_lookback: usize,
    pub squeeze_percentile: f64,
    pub confirmation_candles: usize,
}

impl Default for BollingerSqueeze {
    fn default() -> Self {
        Self {
            bb_period: 20,
            squeeze_lookback: 50,
            squeeze_percentile: 0.20, // bottom 20th percentile = squeeze
            confirmation_candles: 2,
        }
    }
}

impl Strategy for BollingerSqueeze {
    fn name(&self) -> &str { "BB Squeeze" }

    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        if closes.len() < self.squeeze_lookback + self.bb_period { return None; }

        let bw = indicators::bollinger_bandwidth(&closes, self.bb_period, 2.0)?;
        let pct_b = indicators::bollinger_pct_b(&closes, self.bb_period, 2.0)?;

        // Find squeeze threshold (bottom percentile of bandwidth)
        let mut sorted_bw: Vec<f64> = bw[bw.len() - self.squeeze_lookback..].to_vec();
        sorted_bw.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let threshold_idx = (sorted_bw.len() as f64 * self.squeeze_percentile) as usize;
        let squeeze_threshold = sorted_bw.get(threshold_idx).copied().unwrap_or(0.0);

        let n = bw.len();
        if n < self.confirmation_candles + 2 { return None; }

        // Check: was in squeeze, now breaking out
        let was_squeezed = bw[n - self.confirmation_candles - 1] <= squeeze_threshold;
        let expanding = bw[n - 1] > bw[n - 2] && bw[n - 2] > squeeze_threshold;

        if !was_squeezed || !expanding { return None; }

        // Direction from %B and MACD
        let current_pct_b = *pct_b.last()?;
        let macd_hist = signal.indicators.get("macd_hist").copied().unwrap_or(0.0);

        if current_pct_b > 0.8 && macd_hist > 0.0 {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: 0.75,
                reason: format!("BB Squeeze breakout UP (%B={current_pct_b:.2}, MACD hist={macd_hist:.4})"),
            })
        } else if current_pct_b < 0.2 && macd_hist < 0.0 {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: 0.75,
                reason: format!("BB Squeeze breakout DOWN (%B={current_pct_b:.2}, MACD hist={macd_hist:.4})"),
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
        let pct_b = match indicators::bollinger_pct_b(&closes, self.bb_period, 2.0) {
            Some(v) => v, None => return false,
        };
        let current_pct_b = *pct_b.last().unwrap();

        // Exit when bandwidth contracts again or opposite band tested
        let n = bw.len();
        let contracting = n >= 3 && bw[n - 1] < bw[n - 2] && bw[n - 2] < bw[n - 3];

        match side {
            Side::Long => contracting || current_pct_b < 0.3,
            Side::Short => contracting || current_pct_b > 0.7,
        }
    }
}

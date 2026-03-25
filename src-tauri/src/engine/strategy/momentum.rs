use crate::models::{Signal, Candle, Direction, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// Momentum Strategy
/// Trades in the direction of strong momentum, confirmed by multiple indicators:
/// - MACD histogram expanding
/// - RSI in momentum zone (40-60 avoided, only acts on strong readings)
/// - Price above/below EMA for trend confirmation
pub struct Momentum {
    pub ema_period: usize,
    pub rsi_long_threshold: f64,
    pub rsi_short_threshold: f64,
    pub min_strength: f64,
}

impl Default for Momentum {
    fn default() -> Self {
        Self {
            ema_period: 21,
            rsi_long_threshold: 55.0,
            rsi_short_threshold: 45.0,
            min_strength: 0.6,
        }
    }
}

impl Strategy for Momentum {
    fn name(&self) -> &str {
        "Momentum"
    }

    fn should_enter(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Option<TradeIntent> {
        if signal.strength < self.min_strength {
            return None;
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        if closes.len() < 26 {
            return None;
        }

        let rsi_vals = indicators::rsi(&closes, 14)?;
        let ema_vals = indicators::ema(&closes, self.ema_period)?;
        let (macd_line, signal_line, _) = indicators::macd(&closes, 12, 26, 9)?;

        let n = closes.len();
        let current_price = closes[n - 1];
        let current_rsi = *rsi_vals.last()?;
        let current_ema = *ema_vals.last()?;
        let current_macd = *macd_line.last()?;
        let current_macd_signal = *signal_line.last()?;
        let macd_hist = current_macd - current_macd_signal;

        // Check for expanding momentum in previous bars
        let prev_macd = if macd_line.len() >= 2 { macd_line[macd_line.len() - 2] } else { return None };
        let prev_signal = if signal_line.len() >= 2 { signal_line[signal_line.len() - 2] } else { return None };
        let prev_hist = prev_macd - prev_signal;

        let hist_expanding = macd_hist.abs() > prev_hist.abs();

        // Bullish momentum: price > EMA, RSI strong, MACD histogram positive and expanding
        if current_price > current_ema
            && current_rsi > self.rsi_long_threshold
            && macd_hist > 0.0
            && hist_expanding
        {
            return Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: signal.strength,
                reason: format!(
                    "Bullish momentum: RSI {:.1}, MACD hist {:.4} expanding, above EMA({})",
                    current_rsi, macd_hist, self.ema_period
                ),
            });
        }

        // Bearish momentum
        if current_price < current_ema
            && current_rsi < self.rsi_short_threshold
            && macd_hist < 0.0
            && hist_expanding
        {
            return Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: signal.strength,
                reason: format!(
                    "Bearish momentum: RSI {:.1}, MACD hist {:.4} expanding, below EMA({})",
                    current_rsi, macd_hist, self.ema_period
                ),
            });
        }

        None
    }

    fn should_exit(
        &self,
        _signal: &Signal,
        candles: &[Candle],
        _entry_price: f64,
        side: Side,
    ) -> bool {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        if closes.len() < 26 { return false; }

        let (macd_line, signal_line, _) = match indicators::macd(&closes, 12, 26, 9) {
            Some(v) => v,
            None => return false,
        };

        if macd_line.len() < 2 { return false; }

        let n = macd_line.len();
        let current_hist = macd_line[n - 1] - signal_line[n - 1];
        let prev_hist = macd_line[n - 2] - signal_line[n - 2];

        // Exit when momentum fades (histogram contracts)
        match side {
            Side::Long => current_hist < prev_hist && current_hist < 0.0,
            Side::Short => current_hist > prev_hist && current_hist > 0.0,
        }
    }
}

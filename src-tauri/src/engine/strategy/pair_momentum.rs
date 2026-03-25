use crate::models::{Signal, Candle, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// Pair Momentum Strategy
/// Trades strong rate-of-change momentum confirmed by volume.
pub struct PairMomentum {
    pub fast_lookback: usize,
    pub slow_lookback: usize,
    pub min_roc_pct: f64,
}

impl Default for PairMomentum {
    fn default() -> Self {
        Self {
            fast_lookback: 10,
            slow_lookback: 40,
            min_roc_pct: 2.0,
        }
    }
}

impl Strategy for PairMomentum {
    fn name(&self) -> &str { "Pair Momentum" }

    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        if closes.len() < self.slow_lookback + 5 { return None; }

        let fast_roc = indicators::roc(&closes, self.fast_lookback)?;
        let slow_roc = indicators::roc(&closes, self.slow_lookback)?;

        let fr = *fast_roc.last()?;
        let sr = *slow_roc.last()?;

        // Volume confirmation: current volume above 20-period SMA
        let volumes: Vec<f64> = candles.iter().map(|c| c.volume).collect();
        let vol_sma = indicators::sma(&volumes, 20)?;
        let current_vol = *volumes.last()?;
        let avg_vol = *vol_sma.last()?;
        let vol_confirmed = avg_vol <= 0.0 || current_vol > avg_vol; // skip if no volume data

        if !vol_confirmed { return None; }

        // Bullish: both ROCs positive, fast > min threshold, accelerating
        if fr > self.min_roc_pct && sr > 0.0 && fr > sr {
            let strength = (fr / 10.0).min(1.0);
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength,
                reason: format!("Pair Momentum: ROC({})={fr:.1}%, ROC({})={sr:.1}%, vol confirmed", self.fast_lookback, self.slow_lookback),
            })
        }
        // Bearish
        else if fr < -self.min_roc_pct && sr < 0.0 && fr < sr {
            let strength = (fr.abs() / 10.0).min(1.0);
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength,
                reason: format!("Pair Momentum: ROC({})={fr:.1}%, ROC({})={sr:.1}%, vol confirmed", self.fast_lookback, self.slow_lookback),
            })
        } else {
            None
        }
    }

    fn should_exit(&self, _signal: &Signal, candles: &[Candle], _entry_price: f64, side: Side) -> bool {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let fast_roc = match indicators::roc(&closes, self.fast_lookback) {
            Some(v) => v, None => return false,
        };
        let fr = *fast_roc.last().unwrap();

        // Exit when fast momentum flips
        match side {
            Side::Long => fr < 0.0,
            Side::Short => fr > 0.0,
        }
    }
}

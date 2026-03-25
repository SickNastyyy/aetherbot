use crate::models::{Signal, Candle, Direction, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// RSI Mean Reversion Strategy
/// Buys when RSI is oversold (< 30), sells when overbought (> 70)
/// Uses higher-timeframe EMA as trend filter — only trades in trend direction
pub struct RsiReversion {
    pub rsi_period: usize,
    pub oversold: f64,
    pub overbought: f64,
    pub trend_ema_period: usize,
    pub min_strength: f64,
}

impl Default for RsiReversion {
    fn default() -> Self {
        Self {
            rsi_period: 14,
            oversold: 30.0,
            overbought: 70.0,
            trend_ema_period: 50,
            min_strength: 0.5,
        }
    }
}

impl Strategy for RsiReversion {
    fn name(&self) -> &str {
        "RSI Mean Reversion"
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
        if closes.len() < self.trend_ema_period {
            return None;
        }

        let rsi_vals = indicators::rsi(&closes, self.rsi_period)?;
        let trend_ema = indicators::ema(&closes, self.trend_ema_period)?;

        let current_rsi = *rsi_vals.last()?;
        let current_price = *closes.last()?;
        let current_trend = *trend_ema.last()?;

        // Only long when price is above trend EMA (uptrend) and RSI is oversold
        if current_rsi < self.oversold && current_price > current_trend {
            return Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: signal.strength,
                reason: format!(
                    "RSI({}) = {:.1} (oversold < {:.0}), price above EMA({})",
                    self.rsi_period, current_rsi, self.oversold, self.trend_ema_period
                ),
            });
        }

        // Only short when price is below trend EMA (downtrend) and RSI is overbought
        if current_rsi > self.overbought && current_price < current_trend {
            return Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: signal.strength,
                reason: format!(
                    "RSI({}) = {:.1} (overbought > {:.0}), price below EMA({})",
                    self.rsi_period, current_rsi, self.overbought, self.trend_ema_period
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
        let rsi_vals = match indicators::rsi(&closes, self.rsi_period) {
            Some(v) => v,
            None => return false,
        };

        let current_rsi = match rsi_vals.last() {
            Some(&v) => v,
            None => return false,
        };

        // Exit long when RSI returns to neutral/overbought
        // Exit short when RSI returns to neutral/oversold
        match side {
            Side::Long => current_rsi > 55.0,  // RSI reverted above midpoint
            Side::Short => current_rsi < 45.0,  // RSI reverted below midpoint
        }
    }
}

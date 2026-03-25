use crate::models::{Signal, Candle, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

/// Dollar Cost Averaging Strategy
/// Accumulates on dips — buys when price drops X% from recent high.
pub struct DollarCostAvg {
    pub dip_threshold_pct: f64,
    pub lookback: usize,
    pub rsi_threshold: f64,
}

impl Default for DollarCostAvg {
    fn default() -> Self {
        Self {
            dip_threshold_pct: 3.0,
            lookback: 20,
            rsi_threshold: 40.0,
        }
    }
}

impl Strategy for DollarCostAvg {
    fn name(&self) -> &str { "DCA" }

    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent> {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        if closes.len() < self.lookback { return None; }

        let current = *closes.last()?;
        let recent = &closes[closes.len() - self.lookback..];
        let recent_high = recent.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let dip_pct = ((recent_high - current) / recent_high) * 100.0;

        if dip_pct < self.dip_threshold_pct { return None; }

        // Confirm with RSI
        let rsi_vals = indicators::rsi(&closes, 14)?;
        let rsi = *rsi_vals.last()?;
        if rsi > self.rsi_threshold { return None; }

        // Strength increases with dip magnitude
        let strength = (dip_pct / 10.0).min(1.0) * 0.7;

        Some(TradeIntent {
            pair: signal.pair.clone(),
            exchange: signal.exchange.clone(),
            side: Side::Long,
            strength,
            reason: format!("DCA: {dip_pct:.1}% dip from recent high, RSI={rsi:.1}"),
        })
    }

    fn should_exit(&self, _signal: &Signal, candles: &[Candle], entry_price: f64, _side: Side) -> bool {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let current = match closes.last() { Some(&p) => p, None => return false };

        // Exit when price recovers above entry or RSI normalizes
        let recovery = ((current - entry_price) / entry_price) * 100.0;
        if recovery > 2.0 { return true; }

        if let Some(rsi_vals) = indicators::rsi(&closes, 14) {
            if *rsi_vals.last().unwrap() > 60.0 { return true; }
        }
        false
    }
}

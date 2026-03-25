use crate::models::{Signal, Candle, Direction, Side};
use crate::engine::signals::indicators;
use super::{Strategy, TradeIntent};

pub struct EmaCrossover {
    pub fast_period: usize,
    pub slow_period: usize,
    pub min_strength: f64,
}

impl Default for EmaCrossover {
    fn default() -> Self {
        Self {
            fast_period: 9,
            slow_period: 21,
            min_strength: 0.5,
        }
    }
}

impl Strategy for EmaCrossover {
    fn name(&self) -> &str {
        "EMA Crossover"
    }

    fn should_enter(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Option<TradeIntent> {
        if signal.strength < self.min_strength {
            return None;
        }
        if signal.direction == Direction::Neutral {
            return None;
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let fast = indicators::ema(&closes, self.fast_period)?;
        let slow = indicators::ema(&closes, self.slow_period)?;

        // Check for crossover (current vs previous)
        if fast.len() < 2 || slow.len() < 2 { return None; }
        let n = fast.len();

        let cross_up = fast[n - 1] > slow[n - 1] && fast[n - 2] <= slow[n - 2];
        let cross_down = fast[n - 1] < slow[n - 1] && fast[n - 2] >= slow[n - 2];

        if cross_up {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Long,
                strength: signal.strength,
                reason: format!(
                    "EMA({}/{}) bullish crossover, strength {:.0}%",
                    self.fast_period,
                    self.slow_period,
                    signal.strength * 100.0
                ),
            })
        } else if cross_down {
            Some(TradeIntent {
                pair: signal.pair.clone(),
                exchange: signal.exchange.clone(),
                side: Side::Short,
                strength: signal.strength,
                reason: format!(
                    "EMA({}/{}) bearish crossover, strength {:.0}%",
                    self.fast_period,
                    self.slow_period,
                    signal.strength * 100.0
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
        _entry_price: f64,
        side: Side,
    ) -> bool {
        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
        let fast = match indicators::ema(&closes, self.fast_period) {
            Some(v) => v,
            None => return false,
        };
        let slow = match indicators::ema(&closes, self.slow_period) {
            Some(v) => v,
            None => return false,
        };

        let n = fast.len();
        if n < 2 { return false; }

        // Exit on reverse crossover
        match side {
            Side::Long => fast[n - 1] < slow[n - 1] && fast[n - 2] >= slow[n - 2],
            Side::Short => fast[n - 1] > slow[n - 1] && fast[n - 2] <= slow[n - 2],
        }
    }
}

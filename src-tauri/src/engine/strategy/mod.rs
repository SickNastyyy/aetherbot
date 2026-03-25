pub mod ema_crossover;
pub mod rsi_reversion;
pub mod momentum;
pub mod kalshi_edge;

use crate::models::{Signal, Candle, Side};

/// What a strategy wants to do
#[derive(Debug, Clone)]
pub struct TradeIntent {
    pub pair: String,
    pub exchange: String,
    pub side: Side,
    pub strength: f64,
    pub reason: String,
}

/// Pluggable strategy interface
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;

    /// Should we open a new position?
    fn should_enter(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Option<TradeIntent>;

    /// Should we exit an existing position?
    fn should_exit(
        &self,
        signal: &Signal,
        candles: &[Candle],
        entry_price: f64,
        side: Side,
    ) -> bool;
}

/// Multi-strategy evaluator — runs all strategies, picks the best signal
pub struct StrategyManager {
    strategies: Vec<Box<dyn Strategy>>,
}

impl StrategyManager {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(ema_crossover::EmaCrossover::default()),
                Box::new(rsi_reversion::RsiReversion::default()),
                Box::new(momentum::Momentum::default()),
                Box::new(kalshi_edge::KalshiEdge::default()),
            ],
        }
    }

    /// Evaluate all strategies and return the best trade intent (highest strength)
    pub fn evaluate_entry(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Option<(TradeIntent, &str)> {
        let mut best: Option<(TradeIntent, &str)> = None;

        for strategy in &self.strategies {
            if let Some(intent) = strategy.should_enter(signal, candles) {
                let dominated = match &best {
                    Some((existing, _)) => intent.strength > existing.strength,
                    None => true,
                };
                if dominated {
                    let name = strategy.name();
                    best = Some((intent, name));
                }
            }
        }

        best
    }

    /// Check if any strategy says we should exit
    pub fn evaluate_exit(
        &self,
        signal: &Signal,
        candles: &[Candle],
        entry_price: f64,
        side: Side,
        strategy_name: &str,
    ) -> bool {
        // Find the strategy that opened this trade
        for strategy in &self.strategies {
            if strategy.name() == strategy_name {
                return strategy.should_exit(signal, candles, entry_price, side);
            }
        }
        false
    }
}

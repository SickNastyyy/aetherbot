pub mod ema_crossover;
pub mod rsi_reversion;
pub mod momentum;
pub mod kalshi_edge;
pub mod grid;
pub mod dca;
pub mod bb_squeeze;
pub mod volume_profile_strat;
pub mod vwap_reversion;
pub mod pair_momentum;

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
    fn should_enter(&self, signal: &Signal, candles: &[Candle]) -> Option<TradeIntent>;
    fn should_exit(&self, signal: &Signal, candles: &[Candle], entry_price: f64, side: Side) -> bool;
}

/// Multi-strategy evaluator — 10 strategies competing for signals
pub struct StrategyManager {
    strategies: Vec<Box<dyn Strategy>>,
}

impl StrategyManager {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                // Trend following
                Box::new(ema_crossover::EmaCrossover::default()),
                Box::new(momentum::Momentum::default()),
                Box::new(pair_momentum::PairMomentum::default()),
                // Mean reversion
                Box::new(rsi_reversion::RsiReversion::default()),
                Box::new(vwap_reversion::VwapReversion::default()),
                // Volatility
                Box::new(bb_squeeze::BollingerSqueeze::default()),
                Box::new(grid::GridTrading::default()),
                // Volume-based
                Box::new(volume_profile_strat::VolumeProfileStrategy::default()),
                Box::new(dca::DollarCostAvg::default()),
                // Prediction markets
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
                    best = Some((intent, strategy.name()));
                }
            }
        }

        best
    }

    /// Get all strategy votes (for signal aggregation)
    pub fn evaluate_all(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Vec<(TradeIntent, &str)> {
        self.strategies.iter()
            .filter_map(|s| {
                s.should_enter(signal, candles).map(|intent| (intent, s.name()))
            })
            .collect()
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
        for strategy in &self.strategies {
            if strategy.name() == strategy_name {
                return strategy.should_exit(signal, candles, entry_price, side);
            }
        }
        false
    }

    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    pub fn strategy_names(&self) -> Vec<&str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }
}

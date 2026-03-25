use std::collections::HashMap;
use crate::models::{Direction, Side};
use crate::engine::strategy::TradeIntent;

/// A single strategy's vote on a trade
#[derive(Debug, Clone)]
pub struct StrategyVote {
    pub strategy: String,
    pub direction: Direction,
    pub strength: f64,
    pub weight: f64,
}

/// Aggregated result from all strategies
#[derive(Debug, Clone)]
pub struct AggregatedSignal {
    pub pair: String,
    pub exchange: String,
    pub direction: Direction,
    pub side: Side,
    pub consensus_score: f64,
    pub confluence_count: u32,
    pub total_voters: u32,
    pub top_strategy: String,
    pub reason: String,
}

/// Weighted consensus signal aggregation engine.
/// Collects votes from all strategies, weights them by historical performance,
/// and requires minimum confluence before approving a trade.
pub struct SignalAggregator {
    strategy_weights: HashMap<String, f64>,
    pub min_confluence: u32,
    pub min_consensus_score: f64,
}

impl SignalAggregator {
    pub fn new(min_confluence: u32, min_consensus_score: f64) -> Self {
        Self {
            strategy_weights: HashMap::new(),
            min_confluence,
            min_consensus_score,
        }
    }

    /// Update strategy weights from performance data
    pub fn update_weights(&mut self, weights: HashMap<String, f64>) {
        self.strategy_weights = weights;
    }

    /// Convert trade intents from strategy evaluation into votes
    pub fn intents_to_votes(&self, intents: &[(TradeIntent, &str)]) -> Vec<StrategyVote> {
        intents.iter().map(|(intent, name)| {
            let weight = self.strategy_weights.get(*name).copied().unwrap_or(1.0);
            let direction = match intent.side {
                Side::Long => Direction::Long,
                Side::Short => Direction::Short,
            };
            StrategyVote {
                strategy: name.to_string(),
                direction,
                strength: intent.strength,
                weight,
            }
        }).collect()
    }

    /// Aggregate votes into a consensus signal
    pub fn aggregate(
        &self,
        pair: &str,
        exchange: &str,
        votes: &[StrategyVote],
    ) -> Option<AggregatedSignal> {
        if votes.is_empty() { return None; }

        // Separate long and short votes
        let long_votes: Vec<&StrategyVote> = votes.iter()
            .filter(|v| v.direction == Direction::Long)
            .collect();
        let short_votes: Vec<&StrategyVote> = votes.iter()
            .filter(|v| v.direction == Direction::Short)
            .collect();

        // Compute weighted scores
        let total_weight: f64 = votes.iter().map(|v| v.weight).sum();
        if total_weight == 0.0 { return None; }

        let long_score: f64 = long_votes.iter()
            .map(|v| v.strength * v.weight)
            .sum::<f64>() / total_weight;
        let short_score: f64 = short_votes.iter()
            .map(|v| v.strength * v.weight)
            .sum::<f64>() / total_weight;

        // Pick winning direction
        let (direction, side, score, confluence, top_voters) = if long_score > short_score {
            (Direction::Long, Side::Long, long_score, long_votes.len() as u32, &long_votes)
        } else if short_score > long_score {
            (Direction::Short, Side::Short, short_score, short_votes.len() as u32, &short_votes)
        } else {
            return None; // Tie = no trade
        };

        // Check minimum confluence
        if confluence < self.min_confluence {
            return None;
        }

        // Check minimum consensus score
        if score < self.min_consensus_score {
            return None;
        }

        // Find top-contributing strategy
        let top_strategy = top_voters.iter()
            .max_by(|a, b| (a.strength * a.weight).partial_cmp(&(b.strength * b.weight)).unwrap())
            .map(|v| v.strategy.clone())
            .unwrap_or_default();

        let voter_names: Vec<String> = top_voters.iter().map(|v| v.strategy.clone()).collect();

        Some(AggregatedSignal {
            pair: pair.to_string(),
            exchange: exchange.to_string(),
            direction,
            side,
            consensus_score: score,
            confluence_count: confluence,
            total_voters: votes.len() as u32,
            top_strategy,
            reason: format!(
                "Consensus {:.0}% ({}/{} agree: {})",
                score * 100.0,
                confluence,
                votes.len(),
                voter_names.join(", ")
            ),
        })
    }
}

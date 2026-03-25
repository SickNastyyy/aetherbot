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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Direction, Side};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn long_vote(strategy: &str, strength: f64, weight: f64) -> StrategyVote {
        StrategyVote {
            strategy: strategy.to_string(),
            direction: Direction::Long,
            strength,
            weight,
        }
    }

    fn short_vote(strategy: &str, strength: f64, weight: f64) -> StrategyVote {
        StrategyVote {
            strategy: strategy.to_string(),
            direction: Direction::Short,
            strength,
            weight,
        }
    }

    fn aggregator(min_confluence: u32, min_score: f64) -> SignalAggregator {
        SignalAggregator::new(min_confluence, min_score)
    }

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() < tol }

    // ── empty input ───────────────────────────────────────────────────────────

    #[test]
    fn aggregate_empty_votes_returns_none() {
        let agg = aggregator(1, 0.0);
        let result = agg.aggregate("BTC/USD", "paper", &[]);
        assert!(result.is_none());
    }

    // ── all strategies agree ──────────────────────────────────────────────────

    #[test]
    fn all_long_votes_produce_long_signal() {
        let agg = aggregator(1, 0.0);
        let votes = vec![
            long_vote("A", 0.9, 1.0),
            long_vote("B", 0.8, 1.0),
            long_vote("C", 0.7, 1.0),
        ];
        let signal = agg.aggregate("BTC/USD", "paper", &votes).unwrap();
        assert_eq!(signal.direction, Direction::Long);
        assert_eq!(signal.side, Side::Long);
        assert_eq!(signal.confluence_count, 3);
    }

    #[test]
    fn all_short_votes_produce_short_signal() {
        let agg = aggregator(1, 0.0);
        let votes = vec![
            short_vote("A", 0.9, 1.0),
            short_vote("B", 0.8, 1.0),
        ];
        let signal = agg.aggregate("ETH/USD", "paper", &votes).unwrap();
        assert_eq!(signal.direction, Direction::Short);
        assert_eq!(signal.side, Side::Short);
    }

    #[test]
    fn all_agree_high_consensus_score() {
        let agg = aggregator(1, 0.0);
        let votes = vec![
            long_vote("A", 1.0, 1.0),
            long_vote("B", 1.0, 1.0),
            long_vote("C", 1.0, 1.0),
        ];
        let signal = agg.aggregate("BTC/USD", "paper", &votes).unwrap();
        // total_weight=3, long_weighted_sum=3 → score = 3/3 = 1.0
        assert!(approx_eq(signal.consensus_score, 1.0, 1e-9));
    }

    // ── tie = no trade ────────────────────────────────────────────────────────

    #[test]
    fn equal_long_and_short_scores_returns_none() {
        let agg = aggregator(1, 0.0);
        let votes = vec![
            long_vote("A",  0.5, 1.0),
            short_vote("B", 0.5, 1.0),
        ];
        // long_score = 0.5/2 = 0.25; short_score = 0.5/2 = 0.25 → tie
        let result = agg.aggregate("BTC/USD", "paper", &votes);
        assert!(result.is_none(), "tie should produce no signal");
    }

    // ── minimum confluence ────────────────────────────────────────────────────

    #[test]
    fn below_min_confluence_returns_none() {
        let agg = aggregator(2, 0.0);
        let votes = vec![long_vote("A", 0.9, 1.0)]; // only 1 long vote
        let result = agg.aggregate("BTC/USD", "paper", &votes);
        assert!(result.is_none(), "1 vote with min_confluence=2 should be rejected");
    }

    #[test]
    fn at_min_confluence_produces_signal() {
        let agg = aggregator(2, 0.0);
        let votes = vec![
            long_vote("A", 0.8, 1.0),
            long_vote("B", 0.9, 1.0),
        ];
        let result = agg.aggregate("BTC/USD", "paper", &votes);
        assert!(result.is_some(), "2 long votes with min_confluence=2 should be approved");
    }

    #[test]
    fn one_long_one_short_minimum_confluence_two_still_none_for_losing_side() {
        // Long: 2 votes. Short: 1 vote. min_confluence=2.
        // Long wins, and long has 2 votes → signal produced.
        let agg = aggregator(2, 0.0);
        let votes = vec![
            long_vote("A", 0.9, 1.0),
            long_vote("B", 0.8, 1.0),
            short_vote("C", 0.5, 1.0),
        ];
        let result = agg.aggregate("BTC/USD", "paper", &votes);
        assert!(result.is_some());
        assert_eq!(result.unwrap().direction, Direction::Long);
    }

    // ── minimum consensus score ───────────────────────────────────────────────

    #[test]
    fn below_min_consensus_score_returns_none() {
        let agg = aggregator(1, 0.9);
        // strength=0.5, weight=1; total_weight=1 → score = 0.5 < 0.9
        let votes = vec![long_vote("A", 0.5, 1.0)];
        let result = agg.aggregate("BTC/USD", "paper", &votes);
        assert!(result.is_none(), "score below min threshold should be rejected");
    }

    #[test]
    fn at_min_consensus_score_produces_signal() {
        let agg = aggregator(1, 0.5);
        let votes = vec![long_vote("A", 0.5, 1.0)];
        let result = agg.aggregate("BTC/USD", "paper", &votes);
        assert!(result.is_some());
    }

    // ── weighted scoring ──────────────────────────────────────────────────────

    #[test]
    fn higher_weight_strategy_dominates_direction() {
        let agg = aggregator(1, 0.0);
        // Strategy A: long, weight=10, strength=0.5 → weighted = 5.0
        // Strategy B: short, weight=1, strength=0.9 → weighted = 0.9
        // total_weight = 11
        // long_score = 5.0 / 11 ≈ 0.454
        // short_score = 0.9 / 11 ≈ 0.082
        // Long wins despite lower strength because weight is much higher.
        let votes = vec![
            long_vote("A", 0.5, 10.0),
            short_vote("B", 0.9, 1.0),
        ];
        let signal = agg.aggregate("BTC/USD", "paper", &votes).unwrap();
        assert_eq!(signal.direction, Direction::Long, "high-weight long should win");
    }

    #[test]
    fn weighted_score_computed_correctly() {
        // 2 long votes: A(strength=1.0, weight=2.0), B(strength=0.0, weight=1.0)
        // total_weight = 3; long_score = (1.0*2.0 + 0.0*1.0) / 3 = 2/3 ≈ 0.667
        let agg = aggregator(1, 0.0);
        let votes = vec![
            long_vote("A", 1.0, 2.0),
            long_vote("B", 0.0, 1.0),
        ];
        let signal = agg.aggregate("BTC/USD", "paper", &votes).unwrap();
        let expected = 2.0 / 3.0;
        assert!(
            approx_eq(signal.consensus_score, expected, 1e-9),
            "expected score {expected:.6}, got {:.6}",
            signal.consensus_score
        );
    }

    // ── total_voters ──────────────────────────────────────────────────────────

    #[test]
    fn total_voters_counts_all_votes_including_losing_side() {
        let agg = aggregator(1, 0.0);
        let votes = vec![
            long_vote("A", 0.9, 1.0),
            long_vote("B", 0.8, 1.0),
            short_vote("C", 0.3, 1.0),
        ];
        let signal = agg.aggregate("BTC/USD", "paper", &votes).unwrap();
        assert_eq!(signal.total_voters, 3, "total_voters should count all submitted votes");
    }

    // ── top_strategy ──────────────────────────────────────────────────────────

    #[test]
    fn top_strategy_is_highest_weighted_strength_on_winning_side() {
        let agg = aggregator(1, 0.0);
        let votes = vec![
            long_vote("weak",   0.2, 1.0),
            long_vote("strong", 0.9, 1.0),
        ];
        let signal = agg.aggregate("BTC/USD", "paper", &votes).unwrap();
        assert_eq!(signal.top_strategy, "strong");
    }

    // ── update_weights ────────────────────────────────────────────────────────

    #[test]
    fn update_weights_affects_scoring() {
        let mut agg = aggregator(1, 0.0);
        // Give strategy A a huge weight
        let mut weights = std::collections::HashMap::new();
        weights.insert("A".to_string(), 100.0);
        weights.insert("B".to_string(), 1.0);
        agg.update_weights(weights);

        let votes = vec![
            long_vote("A",  0.3, 1.0), // default weight=1, but update_weights overrides on intents
            short_vote("B", 0.9, 1.0),
        ];
        // Note: StrategyVote.weight is set by caller (not looked up again in aggregate),
        // so we test that weights are applied through intents_to_votes.
        // Build votes via intents_to_votes to exercise the lookup path.
        use crate::engine::strategy::TradeIntent;
        let intents: Vec<(TradeIntent, &str)> = vec![
            (TradeIntent {
                pair: "BTC/USD".into(),
                exchange: "paper".into(),
                side: Side::Long,
                strength: 0.3,
                reason: "A signal".into(),
            }, "A"),
            (TradeIntent {
                pair: "BTC/USD".into(),
                exchange: "paper".into(),
                side: Side::Short,
                strength: 0.9,
                reason: "B signal".into(),
            }, "B"),
        ];
        let agg_votes = agg.intents_to_votes(&intents);
        assert_eq!(agg_votes[0].weight, 100.0, "weight for A should be 100");
        assert_eq!(agg_votes[1].weight, 1.0,   "weight for B should be 1");
    }

    #[test]
    fn intents_to_votes_defaults_unknown_strategy_weight_to_one() {
        let agg = aggregator(1, 0.0); // no weights registered
        use crate::engine::strategy::TradeIntent;
        let intents: Vec<(TradeIntent, &str)> = vec![
            (TradeIntent {
                pair: "BTC/USD".into(),
                exchange: "paper".into(),
                side: Side::Long,
                strength: 0.7,
                reason: "signal".into(),
            }, "unknown_strategy"),
        ];
        let votes = agg.intents_to_votes(&intents);
        assert_eq!(votes[0].weight, 1.0, "unknown strategy should default to weight 1.0");
    }

    // ── pair / exchange forwarded ─────────────────────────────────────────────

    #[test]
    fn aggregate_forwards_pair_and_exchange() {
        let agg = aggregator(1, 0.0);
        let votes = vec![long_vote("A", 0.9, 1.0)];
        let signal = agg.aggregate("SOL/USDT", "jupiter", &votes).unwrap();
        assert_eq!(signal.pair, "SOL/USDT");
        assert_eq!(signal.exchange, "jupiter");
    }
}

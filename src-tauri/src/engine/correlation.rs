use std::collections::{HashMap, VecDeque};
use crate::models::Direction;

/// A signal generated from cross-pair correlation analysis
#[derive(Debug, Clone)]
pub struct CorrelationSignal {
    pub pair: String,
    pub direction: Direction,
    pub strength: f64,
    pub leader_pair: String,
    pub divergence_pct: f64,
    pub reason: String,
}

/// Tracks cross-pair correlations and detects divergences
pub struct CorrelationEngine {
    /// Rolling price returns per pair
    returns: HashMap<String, VecDeque<f64>>,
    /// Last known price per pair
    last_prices: HashMap<String, f64>,
    /// Pairwise correlations
    correlations: HashMap<(String, String), f64>,
    lookback: usize,
}

impl CorrelationEngine {
    pub fn new(lookback: usize) -> Self {
        Self {
            returns: HashMap::new(),
            last_prices: HashMap::new(),
            correlations: HashMap::new(),
            lookback,
        }
    }

    /// Push a new price observation
    pub fn push_price(&mut self, pair: &str, price: f64) {
        if let Some(&last) = self.last_prices.get(pair) {
            if last > 0.0 {
                let ret = (price - last) / last;
                let returns = self.returns.entry(pair.to_string()).or_insert_with(VecDeque::new);
                returns.push_back(ret);
                if returns.len() > self.lookback {
                    returns.pop_front();
                }
            }
        }
        self.last_prices.insert(pair.to_string(), price);
    }

    /// Compute Pearson correlations between all pairs
    pub fn compute_correlations(&mut self) {
        let pairs: Vec<String> = self.returns.keys().cloned().collect();

        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                let a = &pairs[i];
                let b = &pairs[j];
                if let (Some(ra), Some(rb)) = (self.returns.get(a), self.returns.get(b)) {
                    if let Some(corr) = pearson(ra, rb) {
                        self.correlations.insert((a.clone(), b.clone()), corr);
                        self.correlations.insert((b.clone(), a.clone()), corr);
                    }
                }
            }
        }
    }

    /// Detect divergences: when a correlated pair lags the leader
    pub fn detect_divergences(&self, recent_window: usize) -> Vec<CorrelationSignal> {
        let mut signals = Vec::new();

        // Use ETH and SOL as leaders
        let leaders = ["ETH/USDC", "SOL/USDC"];

        for leader in &leaders {
            let leader_returns = match self.returns.get(*leader) {
                Some(r) if r.len() >= recent_window => r,
                _ => continue,
            };

            let leader_recent: f64 = leader_returns.iter().rev().take(recent_window).sum::<f64>() * 100.0;

            for (pair, returns) in &self.returns {
                if pair == *leader { continue; }
                if returns.len() < recent_window { continue; }

                // Check correlation
                let corr = self.correlations.get(&(leader.to_string(), pair.clone()))
                    .copied()
                    .unwrap_or(0.0);

                if corr < 0.5 { continue; } // Not correlated enough

                let pair_recent: f64 = returns.iter().rev().take(recent_window).sum::<f64>() * 100.0;
                let divergence = leader_recent - pair_recent;

                // Significant divergence: leader moved but follower didn't
                if divergence.abs() < 1.0 { continue; }

                let strength = (divergence.abs() / 5.0).min(1.0) * corr;

                if divergence > 1.0 {
                    // Leader went up, follower lagged → expect catch-up (long)
                    signals.push(CorrelationSignal {
                        pair: pair.clone(),
                        direction: Direction::Long,
                        strength,
                        leader_pair: leader.to_string(),
                        divergence_pct: divergence,
                        reason: format!(
                            "Divergence: {} +{:.1}% but {} only {:.1}% (corr={:.2})",
                            leader, leader_recent, pair, pair_recent, corr
                        ),
                    });
                } else if divergence < -1.0 {
                    // Leader went down, follower hasn't yet → expect drop (short)
                    signals.push(CorrelationSignal {
                        pair: pair.clone(),
                        direction: Direction::Short,
                        strength,
                        leader_pair: leader.to_string(),
                        divergence_pct: divergence,
                        reason: format!(
                            "Divergence: {} {:.1}% but {} still {:.1}% (corr={:.2})",
                            leader, leader_recent, pair, pair_recent, corr
                        ),
                    });
                }
            }
        }

        signals
    }
}

/// Pearson correlation coefficient between two series
fn pearson(a: &VecDeque<f64>, b: &VecDeque<f64>) -> Option<f64> {
    let n = a.len().min(b.len());
    if n < 10 { return None; }

    let a_slice: Vec<f64> = a.iter().rev().take(n).cloned().collect();
    let b_slice: Vec<f64> = b.iter().rev().take(n).cloned().collect();

    let n_f = n as f64;
    let mean_a: f64 = a_slice.iter().sum::<f64>() / n_f;
    let mean_b: f64 = b_slice.iter().sum::<f64>() / n_f;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..n {
        let da = a_slice[i] - mean_a;
        let db = b_slice[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom == 0.0 { return None; }

    Some(cov / denom)
}

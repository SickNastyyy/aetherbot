use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::models::{Direction, Side};
use crate::engine::aggregator::AggregatedSignal;
use crate::engine::correlation::CorrelationSignal;
use crate::engine::regime::MarketRegime;
use crate::engine::sentiment::TokenSentiment;

/// Claude's final trading decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainDecision {
    pub action: String,         // "LONG", "SHORT", "HOLD"
    pub confidence: f64,        // 0.0 to 1.0
    pub reasoning: String,
    pub risk_notes: String,
}

/// Performance memory for adaptive behavior
struct RecentPerformance {
    last_decisions: Vec<bool>, // true = profitable, false = loss
}

impl RecentPerformance {
    fn new() -> Self { Self { last_decisions: Vec::new() } }

    fn record(&mut self, profitable: bool) {
        self.last_decisions.push(profitable);
        if self.last_decisions.len() > 10 {
            self.last_decisions.remove(0);
        }
    }

    fn recent_win_rate(&self) -> f64 {
        if self.last_decisions.is_empty() { return 0.5; }
        let wins = self.last_decisions.iter().filter(|&&w| w).count();
        wins as f64 / self.last_decisions.len() as f64
    }

    fn is_on_losing_streak(&self) -> bool {
        self.last_decisions.len() >= 3
            && self.last_decisions.iter().rev().take(3).all(|&w| !w)
    }
}

/// Central AI trading brain that synthesizes all signal sources
pub struct TradingBrain {
    client: Client,
    api_key: String,
    model: String,
    min_confidence: f64,
    performance: RecentPerformance,
}

impl TradingBrain {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            min_confidence: 0.65,
            performance: RecentPerformance::new(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Record trade outcome for adaptive behavior
    pub fn record_outcome(&mut self, profitable: bool) {
        self.performance.record(profitable);
    }

    /// Make a final trading decision by synthesizing ALL available signals
    pub async fn decide(
        &self,
        pair: &str,
        exchange: &str,
        current_price: f64,
        // All the intelligence layers
        aggregated: Option<&AggregatedSignal>,
        indicators: &HashMap<String, f64>,
        sentiment: Option<&TokenSentiment>,
        correlations: &[CorrelationSignal],
        regime: MarketRegime,
    ) -> Option<BrainDecision> {
        if !self.is_configured() { return None; }

        // Build comprehensive context
        let mut context = format!(
            "## Trading Decision Request\n\
             **Pair:** {pair} on {exchange}\n\
             **Price:** ${current_price:.4}\n\
             **Market Regime:** {regime}\n\n",
            regime = regime.label(),
        );

        // Strategy consensus
        if let Some(agg) = aggregated {
            context.push_str(&format!(
                "## Strategy Consensus\n\
                 Direction: {:?} | Score: {:.0}% | Confluence: {}/{} strategies agree\n\
                 Lead strategy: {}\n\
                 Detail: {}\n\n",
                agg.direction, agg.consensus_score * 100.0,
                agg.confluence_count, agg.total_voters,
                agg.top_strategy, agg.reason,
            ));
        } else {
            context.push_str("## Strategy Consensus\nNo consensus reached (strategies disagree).\n\n");
        }

        // Technical indicators
        context.push_str("## Technical Indicators\n");
        let key_indicators = [
            ("rsi", "RSI(14)"),
            ("ema_fast", "EMA(9)"),
            ("ema_slow", "EMA(21)"),
            ("macd_hist", "MACD Histogram"),
            ("atr", "ATR(14)"),
            ("bb_bandwidth", "BB Bandwidth"),
            ("bb_pct_b", "BB %B"),
            ("vwap", "VWAP"),
            ("roc_10", "ROC(10)"),
        ];
        for (key, label) in &key_indicators {
            if let Some(&val) = indicators.get(*key) {
                context.push_str(&format!("- {label}: {val:.4}\n"));
            }
        }
        context.push('\n');

        // Sentiment
        if let Some(sent) = sentiment {
            context.push_str(&format!(
                "## Sentiment Analysis\n\
                 Composite: {:.0}% | Direction: {:?} | Confidence: {:.0}%\n\
                 {}\n\n",
                sent.composite_score * 100.0, sent.direction,
                sent.confidence * 100.0, sent.summary,
            ));
        }

        // Correlations
        let relevant_corrs: Vec<&CorrelationSignal> = correlations.iter()
            .filter(|c| c.pair == pair && c.strength > 0.3)
            .collect();
        if !relevant_corrs.is_empty() {
            context.push_str("## Cross-Pair Signals\n");
            for c in &relevant_corrs {
                context.push_str(&format!("- {}\n", c.reason));
            }
            context.push('\n');
        }

        // Performance context (adaptive behavior)
        let win_rate = self.performance.recent_win_rate();
        let on_streak = self.performance.is_on_losing_streak();
        context.push_str(&format!(
            "## Recent Performance\n\
             Win rate (last 10): {:.0}%{}\n\n",
            win_rate * 100.0,
            if on_streak { " ⚠️ ON LOSING STREAK — be more conservative" } else { "" },
        ));

        // The decision prompt
        let system = "You are an elite quantitative trading AI. Your job is to make the final \
            trade/no-trade decision by synthesizing technical analysis, strategy consensus, \
            sentiment data, cross-pair correlations, and market regime into one clear call.\n\n\
            Rules:\n\
            - Only recommend trades with genuine edge (multiple confirming signals)\n\
            - If signals conflict, default to HOLD\n\
            - In bear regime, heavily favor HOLD or SHORT\n\
            - If on a losing streak, raise your confidence threshold\n\
            - Consider risk/reward: is the potential gain worth the risk?\n\
            - Be concise in reasoning (1-2 sentences)\n\n\
            Respond ONLY with valid JSON:\n\
            {\"action\": \"LONG\" or \"SHORT\" or \"HOLD\", \"confidence\": 0.0-1.0, \
            \"reasoning\": \"why\", \"risk_notes\": \"key risks\"}";

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 300,
            "system": system,
            "messages": [{ "role": "user", "content": context }]
        });

        let resp = match self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("[Brain] API call failed: {e}");
                return None;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            warn!("[Brain] API error {status}");
            return None;
        }

        let resp_json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                warn!("[Brain] Parse error: {e}");
                return None;
            }
        };

        let text = resp_json["content"][0]["text"].as_str()?;

        // Extract JSON
        let json_str = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else { text }
        } else { text };

        let decision: BrainDecision = match serde_json::from_str(json_str) {
            Ok(d) => d,
            Err(e) => {
                warn!("[Brain] Failed to parse decision: {e}. Raw: {text}");
                return None;
            }
        };

        // Adaptive threshold: raise minimum confidence on losing streaks
        let effective_min = if on_streak {
            (self.min_confidence + 0.15).min(0.90)
        } else {
            self.min_confidence
        };

        if decision.confidence < effective_min {
            info!(
                "[Brain] {pair}: {} at {:.0}% confidence (below {:.0}% threshold) — {}",
                decision.action, decision.confidence * 100.0,
                effective_min * 100.0, decision.reasoning
            );
            return None;
        }

        info!(
            "[Brain] {pair}: {} at {:.0}% — {} | Risks: {}",
            decision.action, decision.confidence * 100.0,
            decision.reasoning, decision.risk_notes
        );

        Some(decision)
    }
}

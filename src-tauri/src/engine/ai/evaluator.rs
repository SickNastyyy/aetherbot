use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::models::{Candle, Direction, Signal};

/// Claude's trading evaluation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeEvaluation {
    pub direction: String,
    pub confidence: f64,
    pub reasoning: String,
}

/// Configuration for the Claude evaluator
#[derive(Debug, Clone)]
pub struct ClaudeEvaluatorConfig {
    pub api_key: String,
    pub model: String,
    pub min_confidence: f64,
}

impl Default for ClaudeEvaluatorConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-sonnet-4-20250514".to_string(),
            min_confidence: 0.70,
        }
    }
}

/// AI-powered market evaluator using Claude
pub struct ClaudeEvaluator {
    client: Client,
    config: ClaudeEvaluatorConfig,
}

impl ClaudeEvaluator {
    pub fn new(config: ClaudeEvaluatorConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            config,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.config.api_key.is_empty()
    }

    /// Ask Claude to evaluate a market based on technical data
    pub async fn evaluate(
        &self,
        pair: &str,
        exchange: &str,
        candles: &[Candle],
        indicators: &HashMap<String, f64>,
        current_price: f64,
    ) -> Option<Signal> {
        if !self.is_configured() {
            return None;
        }

        if candles.len() < 20 {
            return None;
        }

        // Build market context for Claude
        let recent_closes: Vec<f64> = candles.iter().rev().take(20).map(|c| c.close).collect();
        let price_change_1h = if recent_closes.len() >= 60 {
            ((recent_closes[0] - recent_closes[59]) / recent_closes[59]) * 100.0
        } else {
            ((recent_closes[0] - recent_closes[recent_closes.len() - 1])
                / recent_closes[recent_closes.len() - 1])
                * 100.0
        };

        let rsi = indicators.get("rsi").copied().unwrap_or(50.0);
        let ema_fast = indicators.get("ema_fast").copied().unwrap_or(0.0);
        let ema_slow = indicators.get("ema_slow").copied().unwrap_or(0.0);
        let macd = indicators.get("macd").copied().unwrap_or(0.0);
        let macd_signal = indicators.get("macd_signal").copied().unwrap_or(0.0);
        let atr = indicators.get("atr").copied().unwrap_or(0.0);

        let prompt = format!(
            r#"You are a quantitative trading analyst. Analyze this market data and provide a trading signal.

Market: {pair} on {exchange}
Current Price: ${current_price:.4}
Recent Price Change: {price_change_1h:.2}%

Technical Indicators:
- RSI(14): {rsi:.1}
- EMA(9): ${ema_fast:.4}
- EMA(21): ${ema_slow:.4}
- EMA Trend: {ema_trend}
- MACD: {macd:.4}
- MACD Signal: {macd_signal:.4}
- MACD Histogram: {macd_hist:.4}
- ATR(14): ${atr:.4} ({atr_pct:.2}% of price)

Last 20 closing prices (newest first): {closes}

Respond ONLY with valid JSON in this exact format:
{{"direction": "LONG" or "SHORT" or "NEUTRAL", "confidence": 0.0 to 1.0, "reasoning": "brief explanation"}}"#,
            ema_trend = if ema_fast > ema_slow { "Bullish (fast > slow)" } else { "Bearish (fast < slow)" },
            macd_hist = macd - macd_signal,
            atr_pct = if current_price > 0.0 { (atr / current_price) * 100.0 } else { 0.0 },
            closes = recent_closes.iter().map(|p| format!("{p:.4}")).collect::<Vec<_>>().join(", "),
        );

        match self.call_claude(&prompt).await {
            Ok(eval) => {
                info!(
                    "[Claude] {pair}: {} ({:.0}%) — {}",
                    eval.direction, eval.confidence * 100.0, eval.reasoning
                );

                if eval.confidence < self.config.min_confidence {
                    info!("[Claude] Confidence {:.0}% below threshold {:.0}%, skipping",
                        eval.confidence * 100.0, self.config.min_confidence * 100.0);
                    return None;
                }

                let direction = match eval.direction.to_uppercase().as_str() {
                    "LONG" => Direction::Long,
                    "SHORT" => Direction::Short,
                    _ => Direction::Neutral,
                };

                if direction == Direction::Neutral {
                    return None;
                }

                let mut signal_indicators = indicators.clone();
                signal_indicators.insert("claude_confidence".into(), eval.confidence);

                Some(Signal {
                    pair: pair.to_string(),
                    exchange: exchange.to_string(),
                    direction,
                    strength: eval.confidence,
                    indicators: signal_indicators,
                })
            }
            Err(e) => {
                warn!("[Claude] Evaluation failed for {pair}: {e}");
                None
            }
        }
    }

    async fn call_claude(&self, prompt: &str) -> Result<ClaudeEvaluation, String> {
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": 256,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {text}"));
        }

        let resp_json: serde_json::Value = resp.json().await
            .map_err(|e| format!("Parse error: {e}"))?;

        // Extract text from Claude's response
        let text = resp_json["content"][0]["text"]
            .as_str()
            .ok_or("Missing response text")?;

        // Parse the JSON from Claude's response (handle markdown code blocks)
        let json_str = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else {
                text
            }
        } else {
            text
        };

        let eval: ClaudeEvaluation = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse Claude response as JSON: {e}. Raw: {text}"))?;

        // Validate
        if eval.confidence < 0.0 || eval.confidence > 1.0 {
            return Err(format!("Invalid confidence value: {}", eval.confidence));
        }

        Ok(eval)
    }
}

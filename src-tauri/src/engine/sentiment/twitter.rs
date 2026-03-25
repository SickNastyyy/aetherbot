use reqwest::Client;
use super::SentimentSignal;

/// Uses Grok (xAI API) to analyze real-time X/Twitter sentiment.
/// Grok has native access to tweets, making it ideal for crypto sentiment.
pub struct GrokTwitterAnalyzer {
    client: Client,
    api_key: String,
    model: String,
}

impl GrokTwitterAnalyzer {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            api_key,
            model: "grok-3-mini".to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Ask Grok to analyze current X/Twitter sentiment for a crypto token
    pub async fn analyze_sentiment(&self, token: &str) -> Result<SentimentSignal, String> {
        if !self.is_configured() {
            return Err("Grok API key not configured".into());
        }

        let prompt = format!(
            r#"Analyze the current sentiment on X/Twitter for the cryptocurrency token {token}.

Look at recent tweets, discussions, and trending topics about {token}. Consider:
- Overall bullish vs bearish sentiment
- Volume of discussion (is it trending?)
- Key influencer opinions
- Any major news or announcements
- Hype level vs substantive analysis

Respond with ONLY valid JSON:
{{"score": -1.0 to 1.0, "magnitude": 0.0 to 1.0, "summary": "one sentence"}}"#
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a crypto market sentiment analyst with access to real-time X/Twitter data. Be precise and quantitative. Score represents bearish (-1) to bullish (+1). Magnitude represents how strong/significant the sentiment signal is (0=noise, 1=very significant)."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 150,
            "temperature": 0.3
        });

        let resp = self.client
            .post("https://api.x.ai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Grok API error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Grok API {status}: {text}"));
        }

        let resp_json: serde_json::Value = resp.json().await
            .map_err(|e| format!("Parse error: {e}"))?;

        let text = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("Missing response content")?;

        // Parse JSON from response
        let json_str = extract_json(text);
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse Grok response: {e}. Raw: {text}"))?;

        let score = parsed["score"].as_f64().unwrap_or(0.0).clamp(-1.0, 1.0);
        let magnitude = parsed["magnitude"].as_f64().unwrap_or(0.5).clamp(0.0, 1.0);
        let summary = parsed["summary"].as_str().unwrap_or("No summary").to_string();

        Ok(SentimentSignal {
            token: token.to_string(),
            source: "Twitter/X".to_string(),
            score,
            magnitude,
            summary,
        })
    }
}

fn extract_json(text: &str) -> &str {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return &text[start..=end];
        }
    }
    text
}

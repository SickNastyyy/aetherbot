use reqwest::Client;
use super::SentimentSignal;

/// Maps token symbols to CryptoCompare categories
fn categories_for_token(token: &str) -> &'static str {
    match token.to_uppercase().as_str() {
        "BTC" | "BITCOIN" => "BTC",
        "ETH" | "ETHEREUM" => "ETH",
        "SOL" | "SOLANA" => "SOL",
        "LINK" | "CHAINLINK" => "LINK",
        "UNI" | "UNISWAP" => "UNI",
        "ARB" | "ARBITRUM" => "ARB",
        _ => "Blockchain",
    }
}

/// Fetches crypto news sentiment from CryptoCompare News API (free, no auth)
pub struct NewsFetcher {
    client: Client,
}

impl NewsFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub async fn fetch_sentiment(&self, token: &str) -> Result<SentimentSignal, String> {
        let category = categories_for_token(token);
        let url = format!(
            "https://min-api.cryptocompare.com/data/v2/news/?categories={category}&extraParams=AetherBot"
        );

        let resp = self.client.get(&url)
            .send()
            .await
            .map_err(|e| format!("News API error: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("News API returned {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await
            .map_err(|e| format!("News parse error: {e}"))?;

        let articles = match body["Data"].as_array() {
            Some(a) => a,
            None => return Err("No articles in response".into()),
        };

        if articles.is_empty() {
            return Ok(SentimentSignal {
                token: token.to_string(),
                source: "News".to_string(),
                score: 0.0,
                magnitude: 0.1,
                summary: format!("No recent news for {token}"),
            });
        }

        // Analyze recent articles (last 10)
        let recent = &articles[..articles.len().min(10)];
        let token_lower = token.to_lowercase();

        let mut positive_count = 0u32;
        let mut negative_count = 0u32;
        let mut neutral_count = 0u32;
        let mut relevant_count = 0u32;

        let positive_words = ["bullish", "surge", "rally", "pump", "gain", "rise", "breakout",
            "adoption", "partnership", "launch", "upgrade", "growth", "record", "milestone"];
        let negative_words = ["bearish", "crash", "dump", "fall", "drop", "decline", "hack",
            "exploit", "scam", "ban", "regulation", "lawsuit", "sec", "warning", "risk"];

        for article in recent {
            let title = article["title"].as_str().unwrap_or("").to_lowercase();
            let body_text = article["body"].as_str().unwrap_or("").to_lowercase();
            let combined = format!("{title} {body_text}");

            // Check relevance
            if !combined.contains(&token_lower) && !combined.contains(category.to_lowercase().as_str()) {
                continue;
            }
            relevant_count += 1;

            let pos = positive_words.iter().filter(|w| combined.contains(**w)).count();
            let neg = negative_words.iter().filter(|w| combined.contains(**w)).count();

            if pos > neg { positive_count += 1; }
            else if neg > pos { negative_count += 1; }
            else { neutral_count += 1; }
        }

        if relevant_count == 0 {
            return Ok(SentimentSignal {
                token: token.to_string(),
                source: "News".to_string(),
                score: 0.0,
                magnitude: 0.1,
                summary: format!("No relevant news for {token}"),
            });
        }

        let total = (positive_count + negative_count + neutral_count) as f64;
        let score = if total > 0.0 {
            (positive_count as f64 - negative_count as f64) / total
        } else {
            0.0
        };

        let magnitude = (relevant_count as f64 / 5.0).clamp(0.1, 1.0);

        let summary = format!(
            "{} articles: {} positive, {} negative, {} neutral",
            relevant_count, positive_count, negative_count, neutral_count
        );

        Ok(SentimentSignal {
            token: token.to_string(),
            source: "News".to_string(),
            score,
            magnitude,
            summary,
        })
    }
}

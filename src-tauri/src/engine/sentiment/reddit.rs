use reqwest::Client;
use super::SentimentSignal;

/// Token to subreddit mapping
fn subreddits_for_token(token: &str) -> Vec<&'static str> {
    match token.to_uppercase().as_str() {
        "SOL" | "SOLANA" => vec!["solana", "cryptocurrency"],
        "ETH" | "ETHEREUM" => vec!["ethereum", "cryptocurrency"],
        "BTC" | "BITCOIN" => vec!["bitcoin", "cryptocurrency"],
        "JUP" | "JUPITER" => vec!["solana", "cryptocurrency"],
        "LINK" | "CHAINLINK" => vec!["chainlink", "cryptocurrency"],
        "UNI" | "UNISWAP" => vec!["uniswap", "cryptocurrency"],
        "ARB" | "ARBITRUM" => vec!["arbitrum", "cryptocurrency"],
        _ => vec!["cryptocurrency"],
    }
}

/// Scrapes Reddit's public JSON API for crypto sentiment
pub struct RedditScraper {
    client: Client,
}

impl RedditScraper {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("AetherBot/0.2")
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub async fn fetch_sentiment(&self, token: &str) -> Result<SentimentSignal, String> {
        let subreddits = subreddits_for_token(token);
        let mut total_score = 0i64;
        let mut total_comments = 0u64;
        let mut mention_count = 0u32;
        let mut post_titles: Vec<String> = Vec::new();

        for sub in &subreddits {
            let url = format!("https://www.reddit.com/r/{sub}/hot.json?limit=25");

            let resp = self.client.get(&url)
                .send()
                .await
                .map_err(|e| format!("Reddit fetch error: {e}"))?;

            if !resp.status().is_success() { continue; }

            let body: serde_json::Value = resp.json().await
                .map_err(|e| format!("Reddit parse error: {e}"))?;

            let posts = match body["data"]["children"].as_array() {
                Some(p) => p,
                None => continue,
            };

            let token_upper = token.to_uppercase();
            let token_lower = token.to_lowercase();

            for post in posts {
                let data = &post["data"];
                let title = data["title"].as_str().unwrap_or("");
                let score = data["score"].as_i64().unwrap_or(0);
                let comments = data["num_comments"].as_u64().unwrap_or(0);

                // Check if post mentions the token
                let title_lower = title.to_lowercase();
                if title_lower.contains(&token_lower)
                    || title_lower.contains(&format!("${}", token_lower))
                {
                    mention_count += 1;
                    total_score += score;
                    total_comments += comments;
                    if post_titles.len() < 5 {
                        post_titles.push(title.chars().take(80).collect());
                    }
                }
            }
        }

        if mention_count == 0 {
            return Ok(SentimentSignal {
                token: token.to_string(),
                source: "Reddit".to_string(),
                score: 0.0,
                magnitude: 0.1, // low magnitude = not much signal
                summary: format!("No recent Reddit mentions of {token}"),
            });
        }

        // Heuristic scoring:
        // High upvotes on mentions = positive sentiment
        // Many comments = high engagement
        let avg_score = total_score as f64 / mention_count as f64;
        let avg_comments = total_comments as f64 / mention_count as f64;

        // Normalize score: Reddit posts with >100 upvotes are significant
        let sentiment_score = (avg_score / 200.0).clamp(-1.0, 1.0);

        // Magnitude: how much signal we have (more mentions + comments = stronger)
        let magnitude = ((mention_count as f64 / 10.0) * (avg_comments / 50.0).min(1.0))
            .clamp(0.1, 1.0);

        let summary = format!(
            "{} mentions, avg score {:.0}, avg comments {:.0}",
            mention_count, avg_score, avg_comments
        );

        Ok(SentimentSignal {
            token: token.to_string(),
            source: "Reddit".to_string(),
            score: sentiment_score,
            magnitude,
            summary,
        })
    }
}

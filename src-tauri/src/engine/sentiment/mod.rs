pub mod twitter;
pub mod reddit;
pub mod news;

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

use crate::models::Direction;
use twitter::GrokTwitterAnalyzer;
use reddit::RedditScraper;
use news::NewsFetcher;

/// Sentiment signal from any source
#[derive(Debug, Clone)]
pub struct SentimentSignal {
    pub token: String,
    pub source: String,
    pub score: f64,       // -1.0 (bearish) to +1.0 (bullish)
    pub magnitude: f64,   // 0.0 to 1.0 (signal strength)
    pub summary: String,
}

/// Aggregated sentiment across all sources for a token
#[derive(Debug, Clone)]
pub struct TokenSentiment {
    pub token: String,
    pub direction: Direction,
    pub composite_score: f64,  // -1.0 to 1.0
    pub confidence: f64,       // 0.0 to 1.0
    pub sources: Vec<SentimentSignal>,
    pub summary: String,
}

/// Manages all sentiment data sources and produces composite scores
pub struct SentimentEngine {
    twitter: GrokTwitterAnalyzer,
    reddit: RedditScraper,
    news: NewsFetcher,
    /// Cache: token -> (sentiment, timestamp)
    cache: HashMap<String, (TokenSentiment, Instant)>,
    cache_ttl_secs: u64,
}

impl SentimentEngine {
    pub fn new(grok_api_key: String) -> Self {
        Self {
            twitter: GrokTwitterAnalyzer::new(grok_api_key),
            reddit: RedditScraper::new(),
            news: NewsFetcher::new(),
            cache: HashMap::new(),
            cache_ttl_secs: 300, // 5 minute cache
        }
    }

    pub fn is_configured(&self) -> bool {
        self.twitter.is_configured()
    }

    /// Analyze sentiment for a token across all sources
    pub async fn analyze(&mut self, token: &str) -> Option<TokenSentiment> {
        // Check cache
        if let Some((cached, ts)) = self.cache.get(token) {
            if ts.elapsed().as_secs() < self.cache_ttl_secs {
                return Some(cached.clone());
            }
        }

        let mut signals: Vec<SentimentSignal> = Vec::new();

        // 1. Twitter/X via Grok
        match self.twitter.analyze_sentiment(token).await {
            Ok(sig) => {
                info!("[Sentiment] Twitter/{}: score={:.2}, {}", token, sig.score, sig.summary);
                signals.push(sig);
            }
            Err(e) => {
                warn!("[Sentiment] Twitter/{} failed: {}", token, e);
            }
        }

        // 2. Reddit
        match self.reddit.fetch_sentiment(token).await {
            Ok(sig) => {
                info!("[Sentiment] Reddit/{}: score={:.2}, {}", token, sig.score, sig.summary);
                signals.push(sig);
            }
            Err(e) => {
                warn!("[Sentiment] Reddit/{} failed: {}", token, e);
            }
        }

        // 3. News
        match self.news.fetch_sentiment(token).await {
            Ok(sig) => {
                info!("[Sentiment] News/{}: score={:.2}, {}", token, sig.score, sig.summary);
                signals.push(sig);
            }
            Err(e) => {
                warn!("[Sentiment] News/{} failed: {}", token, e);
            }
        }

        if signals.is_empty() {
            return None;
        }

        // Composite: weighted average (Twitter gets 2x weight due to real-time nature)
        let mut total_weight = 0.0;
        let mut weighted_score = 0.0;

        for sig in &signals {
            let weight = if sig.source == "Twitter/X" { 2.0 } else { 1.0 };
            weighted_score += sig.score * sig.magnitude * weight;
            total_weight += sig.magnitude * weight;
        }

        let composite = if total_weight > 0.0 { weighted_score / total_weight } else { 0.0 };
        let confidence = (signals.len() as f64 / 3.0).min(1.0) * composite.abs();

        let direction = if composite > 0.2 {
            Direction::Long
        } else if composite < -0.2 {
            Direction::Short
        } else {
            Direction::Neutral
        };

        let summary = signals.iter()
            .map(|s| format!("{}: {:.0}%", s.source, s.score * 100.0))
            .collect::<Vec<_>>()
            .join(", ");

        let result = TokenSentiment {
            token: token.to_string(),
            direction,
            composite_score: composite,
            confidence,
            sources: signals,
            summary: format!("Sentiment {:.0}% [{}]", composite * 100.0, summary),
        };

        self.cache.insert(token.to_string(), (result.clone(), Instant::now()));

        Some(result)
    }

    /// Get cached sentiment for a token (no API calls)
    pub fn get_cached(&self, token: &str) -> Option<&TokenSentiment> {
        self.cache.get(token).map(|(s, _)| s)
    }
}

use async_trait::async_trait;
use reqwest::Client;
use crate::models::PriceTick;
use crate::engine::error::EngineError;
use super::DataProvider;

const KALSHI_API_BASE: &str = "https://api.elections.kalshi.com/trade-api/v2";

/// Map event display names to Kalshi series tickers for market search
fn series_ticker_for_event(event: &str) -> Option<&'static str> {
    match event {
        "Fed Rate Cut by June" => Some("FED"),
        "BTC above $100k by Q3" => Some("KXBTC"),
        "US GDP Growth > 3%" => Some("GDP"),
        "S&P 500 above 6000 by Dec" => Some("INXD"),
        "Next Supreme Court Retirement" => Some("SCOTUS"),
        _ => None,
    }
}

/// Fetches Kalshi prediction market prices via their public REST API.
/// Market read endpoints require no authentication.
pub struct KalshiProvider {
    client: Client,
    /// Cache: event_name -> (yes_price, timestamp)
    cache: std::sync::Mutex<std::collections::HashMap<String, (f64, i64)>>,
}

impl KalshiProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
            cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Search for markets by series ticker, return the first open market's yes price
    async fn fetch_market_price(&self, event: &str) -> Result<f64, EngineError> {
        let series = series_ticker_for_event(event).ok_or_else(|| EngineError::DataFetch {
            exchange: "Kalshi".into(),
            pair: event.into(),
            msg: format!("No series ticker mapped for event: {event}"),
        })?;

        let url = format!(
            "{KALSHI_API_BASE}/markets?limit=5&status=open&series_ticker={series}"
        );

        let resp: serde_json::Value = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| EngineError::DataFetch {
                exchange: "Kalshi".into(),
                pair: event.into(),
                msg: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| EngineError::DataFetch {
                exchange: "Kalshi".into(),
                pair: event.into(),
                msg: e.to_string(),
            })?;

        let markets = resp["markets"]
            .as_array()
            .ok_or_else(|| EngineError::DataFetch {
                exchange: "Kalshi".into(),
                pair: event.into(),
                msg: "Missing 'markets' array in response".into(),
            })?;

        if markets.is_empty() {
            return Err(EngineError::DataFetch {
                exchange: "Kalshi".into(),
                pair: event.into(),
                msg: format!("No open markets found for series {series}"),
            });
        }

        // Use the first open market's last_price or yes_ask
        let market = &markets[0];

        // Kalshi v2 returns prices as integer cents (e.g., 42 = $0.42)
        let price_cents = market["last_price"]
            .as_i64()
            .or_else(|| market["yes_ask"].as_i64())
            .or_else(|| market["yes_bid"].as_i64())
            // Fallback: try string-format dollar fields (some endpoints)
            .or_else(|| {
                market["last_price_dollars"].as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|d| (d * 100.0) as i64)
            })
            .ok_or_else(|| EngineError::DataFetch {
                exchange: "Kalshi".into(),
                pair: event.into(),
                msg: "No price found in market data".into(),
            })?;

        Ok(price_cents as f64 / 100.0)
    }
}

#[async_trait]
impl DataProvider for KalshiProvider {
    async fn fetch_price(&self, pair: &str) -> Result<PriceTick, EngineError> {
        // Check cache (valid for 10 seconds — Kalshi data changes slower)
        {
            let cache = self.cache.lock().unwrap();
            if let Some(&(price, ts)) = cache.get(pair) {
                let age = chrono::Utc::now().timestamp() - ts;
                if age < 10 {
                    return Ok(PriceTick {
                        price,
                        volume: 0.0,
                        timestamp: ts,
                        bid: None,
                        ask: None,
                    });
                }
            }
        }

        let price = self.fetch_market_price(pair).await?;
        let now = chrono::Utc::now().timestamp();

        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(pair.to_string(), (price, now));
        }

        Ok(PriceTick {
            price,
            volume: 0.0,
            timestamp: now,
            bid: None,
            ask: None,
        })
    }

    fn exchange_name(&self) -> &str {
        "Kalshi"
    }
}

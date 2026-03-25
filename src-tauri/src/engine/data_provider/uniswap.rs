use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use crate::models::PriceTick;
use crate::engine::error::EngineError;
use super::DataProvider;

/// Map trading pair to DefiLlama coin ID (using coingecko IDs for simplicity)
fn defillama_id_for_pair(pair: &str) -> Option<&'static str> {
    let base = pair.split('/').next()?;
    match base {
        "ETH" | "WETH" => Some("coingecko:ethereum"),
        "WBTC" => Some("coingecko:wrapped-bitcoin"),
        "LINK" => Some("coingecko:chainlink"),
        "UNI" => Some("coingecko:uniswap"),
        "ARB" => Some("coingecko:arbitrum"),
        "AAVE" => Some("coingecko:aave"),
        "LDO" => Some("coingecko:lido-dao"),
        "MKR" => Some("coingecko:maker"),
        "CRV" => Some("coingecko:curve-dao-token"),
        "PEPE" => Some("coingecko:pepe"),
        _ => None,
    }
}

/// Fetches EVM token prices via DefiLlama Coins API (free, no auth)
pub struct UniswapProvider {
    client: Client,
    /// Cache: coin_id -> (price, timestamp)
    cache: std::sync::Mutex<HashMap<String, (f64, i64)>>,
}

impl UniswapProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
            cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Bulk fetch all supported token prices in one API call
    pub async fn refresh_all_prices(&self) -> Result<(), EngineError> {
        let coin_ids = [
            "coingecko:ethereum",
            "coingecko:wrapped-bitcoin",
            "coingecko:chainlink",
            "coingecko:uniswap",
            "coingecko:arbitrum",
            "coingecko:aave",
            "coingecko:lido-dao",
            "coingecko:maker",
            "coingecko:curve-dao-token",
            "coingecko:pepe",
        ];
        let ids_param = coin_ids.join(",");

        let url = format!("https://coins.llama.fi/prices/current/{ids_param}");

        let resp: serde_json::Value = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let coins = resp["coins"].as_object().ok_or_else(|| EngineError::DataFetch {
            exchange: "Uniswap".into(),
            pair: "all".into(),
            msg: "Missing 'coins' in DefiLlama response".into(),
        })?;

        let mut cache = self.cache.lock().unwrap();
        let now = chrono::Utc::now().timestamp();

        for (coin_id, data) in coins {
            if let Some(price) = data["price"].as_f64() {
                let confidence = data["confidence"].as_f64().unwrap_or(0.0);
                if confidence >= 0.8 {
                    cache.insert(coin_id.clone(), (price, now));
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl DataProvider for UniswapProvider {
    async fn fetch_price(&self, pair: &str) -> Result<PriceTick, EngineError> {
        let coin_id = defillama_id_for_pair(pair).ok_or_else(|| EngineError::DataFetch {
            exchange: "Uniswap".into(),
            pair: pair.into(),
            msg: format!("Unknown pair: {pair}"),
        })?;

        // Check cache first (valid for 5 seconds)
        {
            let cache = self.cache.lock().unwrap();
            if let Some(&(price, ts)) = cache.get(coin_id) {
                let age = chrono::Utc::now().timestamp() - ts;
                if age < 5 {
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

        // Cache miss or stale — fetch single token
        let url = format!("https://coins.llama.fi/prices/current/{coin_id}");
        let resp: serde_json::Value = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| EngineError::DataFetch {
                exchange: "Uniswap".into(),
                pair: pair.into(),
                msg: e.to_string(),
            })?
            .json()
            .await
            .map_err(|e| EngineError::DataFetch {
                exchange: "Uniswap".into(),
                pair: pair.into(),
                msg: e.to_string(),
            })?;

        let price = resp["coins"][coin_id]["price"]
            .as_f64()
            .ok_or_else(|| EngineError::DataFetch {
                exchange: "Uniswap".into(),
                pair: pair.into(),
                msg: "Missing price in DefiLlama response".into(),
            })?;

        let now = chrono::Utc::now().timestamp();

        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(coin_id.to_string(), (price, now));
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
        "Uniswap"
    }
}

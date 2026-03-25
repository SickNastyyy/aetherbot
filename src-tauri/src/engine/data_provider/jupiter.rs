use async_trait::async_trait;
use reqwest::Client;
use crate::models::PriceTick;
use crate::engine::error::EngineError;
use super::DataProvider;

/// Solana token mint addresses for Jupiter pairs
fn mint_for_token(token: &str) -> Option<&'static str> {
    match token {
        "SOL" => Some("So11111111111111111111111111111111111111112"),
        "USDC" => Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
        "JUP" => Some("JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN"),
        "BONK" => Some("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"),
        "WIF" => Some("EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm"),
        "PYTH" => Some("HZ1JovNiVvGrGNiiYvEozEVgZ58xaU3RKwX8eACQBCt3"),
        "RAY" => Some("4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R"),
        "ORCA" => Some("orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE"),
        "MSOL" => Some("mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So"),
        "JITO" => Some("J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn"),
        "HNT" => Some("hntyVP6YFm1Hg25TN9WGLqM12b8TQmcknKrdu1oxWux"),
        _ => None,
    }
}

/// Parse "SOL/USDC" into (base_mint, quote_token)
fn parse_pair(pair: &str) -> Option<(&'static str, &str)> {
    let parts: Vec<&str> = pair.split('/').collect();
    if parts.len() != 2 { return None; }
    let base_mint = mint_for_token(parts[0])?;
    Some((base_mint, parts[1]))
}

pub struct JupiterProvider {
    client: Client,
}

impl JupiterProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

#[async_trait]
impl DataProvider for JupiterProvider {
    async fn fetch_price(&self, pair: &str) -> Result<PriceTick, EngineError> {
        let (base_mint, _quote) = parse_pair(pair).ok_or_else(|| EngineError::DataFetch {
            exchange: "Jupiter".into(),
            pair: pair.into(),
            msg: format!("Unknown pair: {pair}"),
        })?;

        let url = format!(
            "https://api.jup.ag/price/v2?ids={base_mint}"
        );

        let resp = self.client.get(&url).send().await.map_err(|e| EngineError::DataFetch {
            exchange: "Jupiter".into(),
            pair: pair.into(),
            msg: e.to_string(),
        })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| EngineError::DataFetch {
            exchange: "Jupiter".into(),
            pair: pair.into(),
            msg: e.to_string(),
        })?;

        let price_str = body["data"][base_mint]["price"]
            .as_str()
            .ok_or_else(|| EngineError::DataFetch {
                exchange: "Jupiter".into(),
                pair: pair.into(),
                msg: "Missing price in response".into(),
            })?;

        let price: f64 = price_str.parse().map_err(|_| EngineError::DataFetch {
            exchange: "Jupiter".into(),
            pair: pair.into(),
            msg: format!("Invalid price value: {price_str}"),
        })?;

        Ok(PriceTick {
            price,
            volume: 0.0,
            timestamp: chrono::Utc::now().timestamp(),
            bid: None,
            ask: None,
        })
    }

    fn exchange_name(&self) -> &str {
        "Jupiter"
    }
}

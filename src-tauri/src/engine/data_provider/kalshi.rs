use async_trait::async_trait;
use crate::models::PriceTick;
use crate::engine::error::EngineError;
use super::DataProvider;

/// Placeholder — will be implemented in Phase 7
pub struct KalshiProvider;

impl KalshiProvider {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DataProvider for KalshiProvider {
    async fn fetch_price(&self, pair: &str) -> Result<PriceTick, EngineError> {
        Err(EngineError::DataFetch {
            exchange: "Kalshi".into(),
            pair: pair.into(),
            msg: "Kalshi provider not yet implemented".into(),
        })
    }

    fn exchange_name(&self) -> &str {
        "Kalshi"
    }
}

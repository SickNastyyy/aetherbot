use async_trait::async_trait;
use crate::models::PriceTick;
use crate::engine::error::EngineError;
use super::DataProvider;

/// Placeholder — will be implemented in Phase 7
pub struct UniswapProvider;

impl UniswapProvider {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl DataProvider for UniswapProvider {
    async fn fetch_price(&self, pair: &str) -> Result<PriceTick, EngineError> {
        Err(EngineError::DataFetch {
            exchange: "Uniswap".into(),
            pair: pair.into(),
            msg: "Uniswap provider not yet implemented".into(),
        })
    }

    fn exchange_name(&self) -> &str {
        "Uniswap"
    }
}

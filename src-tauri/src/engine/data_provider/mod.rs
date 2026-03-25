pub mod candles;
pub mod jupiter;
pub mod uniswap;
pub mod kalshi;

use async_trait::async_trait;
use crate::models::PriceTick;
use crate::engine::error::EngineError;

#[async_trait]
pub trait DataProvider: Send + Sync {
    async fn fetch_price(&self, pair: &str) -> Result<PriceTick, EngineError>;
    fn exchange_name(&self) -> &str;
}

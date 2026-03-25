pub mod ema_crossover;

use crate::models::{Signal, Candle, Side};

/// What a strategy wants to do
#[derive(Debug, Clone)]
pub struct TradeIntent {
    pub pair: String,
    pub exchange: String,
    pub side: Side,
    pub strength: f64,
    pub reason: String,
}

/// Pluggable strategy interface
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;

    /// Should we open a new position?
    fn should_enter(
        &self,
        signal: &Signal,
        candles: &[Candle],
    ) -> Option<TradeIntent>;

    /// Should we exit an existing position?
    fn should_exit(
        &self,
        signal: &Signal,
        candles: &[Candle],
        entry_price: f64,
        side: Side,
    ) -> bool;
}

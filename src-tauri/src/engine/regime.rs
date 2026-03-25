use crate::engine::signals::indicators;
use crate::models::Candle;

/// Market regime based on BTC trend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketRegime {
    /// BTC above EMA-200 and EMA-50 — strong uptrend
    Bullish,
    /// BTC above EMA-200 but below EMA-50 — weakening
    Cautious,
    /// BTC below EMA-200 — bear market
    Bearish,
    /// Not enough data to determine
    Unknown,
}

impl MarketRegime {
    /// Should we allow new long entries in this regime?
    pub fn allow_longs(&self) -> bool {
        matches!(self, MarketRegime::Bullish | MarketRegime::Cautious)
    }

    /// Should we allow new short entries in this regime?
    pub fn allow_shorts(&self) -> bool {
        matches!(self, MarketRegime::Bearish | MarketRegime::Cautious)
    }

    pub fn label(&self) -> &'static str {
        match self {
            MarketRegime::Bullish => "BULLISH",
            MarketRegime::Cautious => "CAUTIOUS",
            MarketRegime::Bearish => "BEARISH",
            MarketRegime::Unknown => "UNKNOWN",
        }
    }
}

/// Determine the current market regime from BTC/ETH candle data.
/// Uses EMA-50 and EMA-200 on the reference asset.
pub fn detect_regime(reference_candles: &[Candle]) -> MarketRegime {
    if reference_candles.len() < 200 {
        return MarketRegime::Unknown;
    }

    let closes: Vec<f64> = reference_candles.iter().map(|c| c.close).collect();

    let ema_50 = match indicators::ema(&closes, 50) {
        Some(v) => *v.last().unwrap(),
        None => return MarketRegime::Unknown,
    };

    let ema_200 = match indicators::ema(&closes, 200) {
        Some(v) => *v.last().unwrap(),
        None => return MarketRegime::Unknown,
    };

    let current_price = *closes.last().unwrap();

    if current_price > ema_200 && current_price > ema_50 {
        MarketRegime::Bullish
    } else if current_price > ema_200 {
        MarketRegime::Cautious
    } else {
        MarketRegime::Bearish
    }
}

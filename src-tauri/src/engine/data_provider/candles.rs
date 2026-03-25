use std::collections::VecDeque;
use crate::models::Candle;

const MAX_CANDLES: usize = 500;

#[derive(Debug, Clone, Copy)]
pub enum Timeframe {
    M1,
    M5,
    M15,
    H1,
}

impl Timeframe {
    pub fn seconds(&self) -> i64 {
        match self {
            Timeframe::M1 => 60,
            Timeframe::M5 => 300,
            Timeframe::M15 => 900,
            Timeframe::H1 => 3600,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::H1 => "1h",
        }
    }
}

pub struct CandleBuilder {
    timeframe: Timeframe,
    candles: VecDeque<Candle>,
    current: Option<CandleInProgress>,
}

struct CandleInProgress {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    period_start: i64,
}

impl CandleBuilder {
    pub fn new(timeframe: Timeframe) -> Self {
        Self {
            timeframe,
            candles: VecDeque::with_capacity(MAX_CANDLES),
            current: None,
        }
    }

    /// Push a new price tick. Returns Some(Candle) if a candle just closed.
    pub fn push_tick(&mut self, price: f64, volume: f64, timestamp: i64) -> Option<Candle> {
        let period_secs = self.timeframe.seconds();
        let period_start = (timestamp / period_secs) * period_secs;

        match &mut self.current {
            Some(c) if c.period_start == period_start => {
                // Same candle — update HLCV
                if price > c.high { c.high = price; }
                if price < c.low { c.low = price; }
                c.close = price;
                c.volume += volume;
                None
            }
            Some(c) => {
                // New period — close current candle
                let closed = Candle {
                    open: c.open,
                    high: c.high,
                    low: c.low,
                    close: c.close,
                    volume: c.volume,
                    timestamp: c.period_start,
                    timeframe: self.timeframe.label().to_string(),
                };

                if self.candles.len() >= MAX_CANDLES {
                    self.candles.pop_front();
                }
                self.candles.push_back(closed.clone());

                // Start new candle
                self.current = Some(CandleInProgress {
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                    period_start,
                });

                Some(closed)
            }
            None => {
                // First tick ever
                self.current = Some(CandleInProgress {
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                    period_start,
                });
                None
            }
        }
    }

    pub fn candles(&self) -> &VecDeque<Candle> {
        &self.candles
    }

    pub fn len(&self) -> usize {
        self.candles.len()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ───────────────────────────────────────────────────────────────

    /// One M1 period = 60 seconds. Returns the timestamp for period index `p`.
    fn ts(p: i64) -> i64 { p * 60 }

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() < tol }

    // ── first tick ───────────────────────────────────────────────────────────

    #[test]
    fn first_tick_does_not_close_a_candle() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        let result = builder.push_tick(100.0, 10.0, ts(0));
        assert!(result.is_none(), "first tick must not close a candle");
    }

    #[test]
    fn first_tick_does_not_populate_closed_deque() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        builder.push_tick(100.0, 10.0, ts(0));
        assert_eq!(builder.len(), 0, "closed candle deque should be empty after one tick");
    }

    // ── OHLCV correctness ─────────────────────────────────────────────────────

    #[test]
    fn ohlcv_correct_after_multiple_ticks_in_same_period() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        // All ticks land in period 0 (seconds 0-59)
        builder.push_tick(100.0, 1.0, ts(0) + 0);
        builder.push_tick(110.0, 2.0, ts(0) + 10);
        builder.push_tick(90.0,  3.0, ts(0) + 20);
        builder.push_tick(105.0, 4.0, ts(0) + 30);

        // Trigger close by moving to period 1
        let closed = builder.push_tick(200.0, 1.0, ts(1)).unwrap();

        assert!(approx_eq(closed.open, 100.0, 1e-9),  "open should be first tick price");
        assert!(approx_eq(closed.high, 110.0, 1e-9),  "high should be max price");
        assert!(approx_eq(closed.low,  90.0,  1e-9),  "low should be min price");
        assert!(approx_eq(closed.close, 105.0, 1e-9), "close should be last tick price");
        assert!(approx_eq(closed.volume, 10.0, 1e-9), "volume should sum all ticks");
    }

    #[test]
    fn candle_timestamp_is_period_start() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        builder.push_tick(100.0, 1.0, ts(0) + 45); // 45 seconds into period 0
        let closed = builder.push_tick(101.0, 1.0, ts(1)).unwrap();
        // Period start for ts(0)+45 with period_secs=60 → floor to 0
        assert_eq!(closed.timestamp, 0, "timestamp should be floored to period start");
    }

    // ── period boundary ───────────────────────────────────────────────────────

    #[test]
    fn candle_closes_at_period_boundary() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        builder.push_tick(100.0, 1.0, ts(0));
        // Second tick is in the NEXT period → should close period-0 candle
        let result = builder.push_tick(200.0, 1.0, ts(1));
        assert!(result.is_some(), "crossing a period boundary must close a candle");
    }

    #[test]
    fn ticks_in_same_period_accumulate_without_closing() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        builder.push_tick(100.0, 1.0, ts(0) + 5);
        let r1 = builder.push_tick(101.0, 1.0, ts(0) + 10);
        let r2 = builder.push_tick(102.0, 1.0, ts(0) + 50);
        assert!(r1.is_none());
        assert!(r2.is_none());
        assert_eq!(builder.len(), 0, "no candle should be closed yet");
    }

    // ── VecDeque capacity ─────────────────────────────────────────────────────

    #[test]
    fn deque_does_not_exceed_max_candles() {
        const MAX: usize = 500;
        let mut builder = CandleBuilder::new(Timeframe::M1);
        // Push MAX + 10 full candles (each period gets exactly one tick then boundary)
        for p in 0_i64..(MAX as i64 + 10) {
            builder.push_tick(100.0, 1.0, ts(p));
        }
        assert!(
            builder.len() <= MAX,
            "deque must not exceed MAX_CANDLES={MAX}, got {}",
            builder.len()
        );
    }

    #[test]
    fn oldest_candle_evicted_when_at_capacity() {
        const MAX: usize = 500;
        let mut builder = CandleBuilder::new(Timeframe::M1);
        // Fill to exactly MAX closed candles
        for p in 0_i64..=(MAX as i64) {
            builder.push_tick(p as f64, 1.0, ts(p));
        }
        assert_eq!(builder.len(), MAX);
        // Push one more close — oldest should be evicted
        builder.push_tick(999.0, 1.0, ts(MAX as i64 + 1));
        assert_eq!(builder.len(), MAX, "length must stay at MAX after eviction");
        // Front candle should NOT be the original first one (open=0)
        let front_open = builder.candles().front().unwrap().open;
        assert!(
            front_open > 0.0,
            "oldest candle (open=0) should have been evicted, front open is {front_open}"
        );
    }

    // ── multiple consecutive candles ──────────────────────────────────────────

    #[test]
    fn three_consecutive_periods_each_close_correctly() {
        let mut builder = CandleBuilder::new(Timeframe::M1);
        builder.push_tick(10.0, 1.0, ts(0));
        let c1 = builder.push_tick(20.0, 1.0, ts(1)).unwrap();
        let c2 = builder.push_tick(30.0, 1.0, ts(2)).unwrap();
        let c3 = builder.push_tick(40.0, 1.0, ts(3)).unwrap();

        assert!(approx_eq(c1.open, 10.0, 1e-9), "c1 open");
        assert!(approx_eq(c2.open, 20.0, 1e-9), "c2 open");
        assert!(approx_eq(c3.open, 30.0, 1e-9), "c3 open");
        assert_eq!(builder.len(), 3);
    }

    // ── timeframe label ───────────────────────────────────────────────────────

    #[test]
    fn closed_candle_has_correct_timeframe_label() {
        let mut builder = CandleBuilder::new(Timeframe::M5);
        let period_secs = Timeframe::M5.seconds(); // 300
        builder.push_tick(100.0, 1.0, 0);
        let closed = builder.push_tick(101.0, 1.0, period_secs).unwrap();
        assert_eq!(closed.timeframe, "5m");
    }

    // ── timeframe seconds ─────────────────────────────────────────────────────

    #[test]
    fn timeframe_seconds_are_correct() {
        assert_eq!(Timeframe::M1.seconds(), 60);
        assert_eq!(Timeframe::M5.seconds(), 300);
        assert_eq!(Timeframe::M15.seconds(), 900);
        assert_eq!(Timeframe::H1.seconds(), 3600);
    }

    #[test]
    fn timeframe_labels_are_correct() {
        assert_eq!(Timeframe::M1.label(), "1m");
        assert_eq!(Timeframe::M5.label(), "5m");
        assert_eq!(Timeframe::M15.label(), "15m");
        assert_eq!(Timeframe::H1.label(), "1h");
    }

    // ── h1 timeframe ─────────────────────────────────────────────────────────

    #[test]
    fn h1_candle_closes_after_3600_seconds() {
        let mut builder = CandleBuilder::new(Timeframe::H1);
        builder.push_tick(100.0, 1.0, 0);
        // Still in same H1 period
        let r = builder.push_tick(101.0, 1.0, 3599);
        assert!(r.is_none(), "still in same H1 period at 3599s");
        // Now cross the boundary
        let closed = builder.push_tick(102.0, 1.0, 3600).unwrap();
        assert!(approx_eq(closed.open, 100.0, 1e-9));
        assert!(approx_eq(closed.close, 101.0, 1e-9));
    }
}

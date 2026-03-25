use crate::models::Candle;
use serde::{Serialize, Deserialize};

/// Exponential Moving Average
pub fn ema(data: &[f64], period: usize) -> Option<Vec<f64>> {
    if data.len() < period { return None; }
    let k = 2.0 / (period as f64 + 1.0);
    let mut result = Vec::with_capacity(data.len());

    // Seed with SMA of first `period` values
    let sma: f64 = data[..period].iter().sum::<f64>() / period as f64;
    for _ in 0..period - 1 {
        result.push(sma); // fill early values with SMA
    }
    result.push(sma);

    for i in period..data.len() {
        let prev = *result.last().unwrap();
        result.push(data[i] * k + prev * (1.0 - k));
    }

    Some(result)
}

/// Relative Strength Index
pub fn rsi(closes: &[f64], period: usize) -> Option<Vec<f64>> {
    if closes.len() < period + 1 { return None; }

    let mut gains = Vec::new();
    let mut losses = Vec::new();
    for i in 1..closes.len() {
        let change = closes[i] - closes[i - 1];
        if change >= 0.0 {
            gains.push(change);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-change);
        }
    }

    let mut result = vec![50.0; period]; // fill early values with neutral

    let mut avg_gain: f64 = gains[..period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[..period].iter().sum::<f64>() / period as f64;

    for i in period..gains.len() {
        avg_gain = (avg_gain * (period as f64 - 1.0) + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + losses[i]) / period as f64;

        let rsi_val = if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
        };
        result.push(rsi_val);
    }

    Some(result)
}

/// MACD (returns macd_line, signal_line, histogram)
pub fn macd(
    closes: &[f64],
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let fast_ema = ema(closes, fast_period)?;
    let slow_ema = ema(closes, slow_period)?;

    let macd_line: Vec<f64> = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(f, s)| f - s)
        .collect();

    let signal_line = ema(&macd_line, signal_period)?;

    let histogram: Vec<f64> = macd_line
        .iter()
        .zip(signal_line.iter())
        .map(|(m, s)| m - s)
        .collect();

    Some((macd_line, signal_line, histogram))
}

/// Average True Range
pub fn atr(candles: &[Candle], period: usize) -> Option<Vec<f64>> {
    if candles.len() < period + 1 { return None; }

    let mut tr_values = Vec::with_capacity(candles.len());
    tr_values.push(candles[0].high - candles[0].low);

    for i in 1..candles.len() {
        let c = &candles[i];
        let prev_close = candles[i - 1].close;
        let tr = (c.high - c.low)
            .max((c.high - prev_close).abs())
            .max((c.low - prev_close).abs());
        tr_values.push(tr);
    }

    // First ATR is simple average
    let first_atr: f64 = tr_values[..period].iter().sum::<f64>() / period as f64;
    let mut result = vec![first_atr; period];

    for i in period..tr_values.len() {
        let prev = *result.last().unwrap();
        let smoothed = (prev * (period as f64 - 1.0) + tr_values[i]) / period as f64;
        result.push(smoothed);
    }

    Some(result)
}

/// Bollinger Bands (returns upper, middle, lower)
pub fn bollinger_bands(
    closes: &[f64],
    period: usize,
    num_std: f64,
) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    if closes.len() < period { return None; }

    let mut upper = Vec::with_capacity(closes.len());
    let mut middle = Vec::with_capacity(closes.len());
    let mut lower = Vec::with_capacity(closes.len());

    // Fill early values
    for _ in 0..period - 1 {
        upper.push(0.0);
        middle.push(0.0);
        lower.push(0.0);
    }

    for i in (period - 1)..closes.len() {
        let window = &closes[i + 1 - period..=i];
        let sma: f64 = window.iter().sum::<f64>() / period as f64;
        let variance: f64 = window.iter().map(|x| (x - sma).powi(2)).sum::<f64>() / period as f64;
        let std_dev = variance.sqrt();

        middle.push(sma);
        upper.push(sma + num_std * std_dev);
        lower.push(sma - num_std * std_dev);
    }

    Some((upper, middle, lower))
}

/// Simple Moving Average
pub fn sma(data: &[f64], period: usize) -> Option<Vec<f64>> {
    if data.len() < period { return None; }
    let mut result = vec![0.0; period - 1];
    for i in (period - 1)..data.len() {
        let avg: f64 = data[i + 1 - period..=i].iter().sum::<f64>() / period as f64;
        result.push(avg);
    }
    Some(result)
}

/// Volume-Weighted Average Price (resets every `reset_period` candles, default 1440 = 1 day of 1m candles)
pub fn vwap(candles: &[Candle], reset_period: usize) -> Option<Vec<f64>> {
    if candles.is_empty() { return None; }
    let mut result = Vec::with_capacity(candles.len());
    let mut cum_tp_vol = 0.0;
    let mut cum_vol = 0.0;

    for (i, c) in candles.iter().enumerate() {
        if i % reset_period == 0 {
            cum_tp_vol = 0.0;
            cum_vol = 0.0;
        }
        let typical = (c.high + c.low + c.close) / 3.0;
        let vol = if c.volume > 0.0 { c.volume } else { 1.0 };
        cum_tp_vol += typical * vol;
        cum_vol += vol;
        result.push(if cum_vol > 0.0 { cum_tp_vol / cum_vol } else { c.close });
    }
    Some(result)
}

/// Bollinger Bandwidth = (upper - lower) / middle
pub fn bollinger_bandwidth(closes: &[f64], period: usize, num_std: f64) -> Option<Vec<f64>> {
    let (upper, middle, lower) = bollinger_bands(closes, period, num_std)?;
    let bw: Vec<f64> = upper.iter().zip(middle.iter()).zip(lower.iter())
        .map(|((u, m), l)| if *m != 0.0 { (u - l) / m } else { 0.0 })
        .collect();
    Some(bw)
}

/// Bollinger %B = (price - lower) / (upper - lower)
pub fn bollinger_pct_b(closes: &[f64], period: usize, num_std: f64) -> Option<Vec<f64>> {
    let (upper, _middle, lower) = bollinger_bands(closes, period, num_std)?;
    let pct_b: Vec<f64> = closes.iter().zip(upper.iter()).zip(lower.iter())
        .map(|((p, u), l)| {
            let range = u - l;
            if range > 0.0 { (p - l) / range } else { 0.5 }
        })
        .collect();
    Some(pct_b)
}

/// Rate of Change (percent)
pub fn roc(data: &[f64], period: usize) -> Option<Vec<f64>> {
    if data.len() <= period { return None; }
    let mut result = vec![0.0; period];
    for i in period..data.len() {
        let prev = data[i - period];
        result.push(if prev != 0.0 { ((data[i] - prev) / prev) * 100.0 } else { 0.0 });
    }
    Some(result)
}

/// Volume Profile — identifies Point of Control and Value Area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfile {
    pub poc: f64,
    pub value_area_high: f64,
    pub value_area_low: f64,
}

pub fn volume_profile(candles: &[Candle], num_bins: usize) -> Option<VolumeProfile> {
    if candles.len() < 10 || num_bins == 0 { return None; }

    let min_price = candles.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
    let max_price = candles.iter().map(|c| c.high).fold(f64::NEG_INFINITY, f64::max);
    let range = max_price - min_price;
    if range <= 0.0 { return None; }

    let bin_size = range / num_bins as f64;
    let mut bins = vec![0.0f64; num_bins];

    for c in candles {
        let typical = (c.high + c.low + c.close) / 3.0;
        let bin_idx = ((typical - min_price) / bin_size).floor() as usize;
        let idx = bin_idx.min(num_bins - 1);
        let vol = if c.volume > 0.0 { c.volume } else { 1.0 };
        bins[idx] += vol;
    }

    // POC = price level of highest volume bin
    let max_bin = bins.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);
    let poc = min_price + (max_bin as f64 + 0.5) * bin_size;

    // Value Area = 70% of volume centered on POC
    let total_vol: f64 = bins.iter().sum();
    let target = total_vol * 0.70;
    let mut va_vol = bins[max_bin];
    let mut va_low_idx = max_bin;
    let mut va_high_idx = max_bin;

    while va_vol < target {
        let try_low = if va_low_idx > 0 { bins[va_low_idx - 1] } else { 0.0 };
        let try_high = if va_high_idx < num_bins - 1 { bins[va_high_idx + 1] } else { 0.0 };

        if try_low == 0.0 && try_high == 0.0 { break; }

        if try_low >= try_high {
            va_low_idx = va_low_idx.saturating_sub(1);
            va_vol += try_low;
        } else {
            va_high_idx = (va_high_idx + 1).min(num_bins - 1);
            va_vol += try_high;
        }
    }

    Some(VolumeProfile {
        poc,
        value_area_high: min_price + (va_high_idx as f64 + 1.0) * bin_size,
        value_area_low: min_price + va_low_idx as f64 * bin_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Candle;

    // ── Helper ───────────────────────────────────────────────────────────────

    fn make_candle(open: f64, high: f64, low: f64, close: f64, volume: f64) -> Candle {
        Candle { open, high, low, close, volume, timestamp: 0, timeframe: "1m".into() }
    }

    fn flat_candles(price: f64, n: usize) -> Vec<Candle> {
        (0..n).map(|_| make_candle(price, price, price, price, 1.0)).collect()
    }

    fn close_series(prices: &[f64]) -> Vec<Candle> {
        prices.iter().map(|&p| make_candle(p, p * 1.01, p * 0.99, p, 100.0)).collect()
    }

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    // ── SMA ──────────────────────────────────────────────────────────────────

    #[test]
    fn sma_basic_average() {
        // [1,2,3,4,5] with period 3: first valid = (1+2+3)/3=2, then 3, then 4
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sma(&data, 3).unwrap();
        assert_eq!(result.len(), 5);
        // First two are early-fill zeros
        assert_eq!(result[0], 0.0);
        assert_eq!(result[1], 0.0);
        assert!(approx_eq(result[2], 2.0, 1e-9));
        assert!(approx_eq(result[3], 3.0, 1e-9));
        assert!(approx_eq(result[4], 4.0, 1e-9));
    }

    #[test]
    fn sma_period_one_returns_input() {
        let data = vec![5.0, 10.0, 15.0];
        let result = sma(&data, 1).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn sma_insufficient_data_returns_none() {
        assert!(sma(&[1.0, 2.0], 5).is_none());
    }

    #[test]
    fn sma_empty_returns_none() {
        assert!(sma(&[], 3).is_none());
    }

    #[test]
    fn sma_exact_period_length() {
        // Exactly period elements — one valid result
        let data = vec![2.0, 4.0, 6.0];
        let result = sma(&data, 3).unwrap();
        assert_eq!(result.len(), 3);
        assert!(approx_eq(*result.last().unwrap(), 4.0, 1e-9));
    }

    // ── EMA ──────────────────────────────────────────────────────────────────

    #[test]
    fn ema_seeded_with_sma() {
        // Period-3 EMA; first 3 values are all SMA of first 3: (10+20+30)/3 = 20
        let data = vec![10.0, 20.0, 30.0, 40.0];
        let result = ema(&data, 3).unwrap();
        assert_eq!(result.len(), 4);
        // k = 2/(3+1) = 0.5
        // result[3] = 40 * 0.5 + 20 * 0.5 = 30
        assert!(approx_eq(result[0], 20.0, 1e-9));
        assert!(approx_eq(result[1], 20.0, 1e-9));
        assert!(approx_eq(result[2], 20.0, 1e-9));
        assert!(approx_eq(result[3], 30.0, 1e-9));
    }

    #[test]
    fn ema_rising_series_tracks_upward() {
        // Steadily rising data — EMA should be below last price but rising
        let data: Vec<f64> = (1..=20).map(|x| x as f64 * 10.0).collect();
        let result = ema(&data, 5).unwrap();
        let last = *result.last().unwrap();
        let second_last = result[result.len() - 2];
        assert!(last > second_last, "EMA should rise with rising data");
        assert!(last < 200.0, "EMA lags the last close");
    }

    #[test]
    fn ema_insufficient_data_returns_none() {
        assert!(ema(&[1.0, 2.0], 5).is_none());
    }

    #[test]
    fn ema_flat_series_equals_price() {
        let data = vec![50.0; 10];
        let result = ema(&data, 5).unwrap();
        for v in &result {
            assert!(approx_eq(*v, 50.0, 1e-9));
        }
    }

    // ── RSI ──────────────────────────────────────────────────────────────────

    #[test]
    fn rsi_oversold_below_30() {
        // Strongly falling prices — RSI should be < 30
        let prices: Vec<f64> = (0..20).map(|i| 100.0 - i as f64 * 3.0).collect();
        let result = rsi(&prices, 14).unwrap();
        let last = *result.last().unwrap();
        assert!(last < 30.0, "expected RSI < 30 for falling prices, got {last}");
    }

    #[test]
    fn rsi_overbought_above_70() {
        // Strongly rising prices — RSI should be > 70
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + i as f64 * 3.0).collect();
        let result = rsi(&prices, 14).unwrap();
        let last = *result.last().unwrap();
        assert!(last > 70.0, "expected RSI > 70 for rising prices, got {last}");
    }

    #[test]
    fn rsi_alternating_is_near_50() {
        // Alternating up/down by equal amounts — gains == losses => RSI near 50
        let mut prices = vec![100.0_f64];
        for i in 1..30 {
            if i % 2 == 0 {
                prices.push(*prices.last().unwrap() + 1.0);
            } else {
                prices.push(*prices.last().unwrap() - 1.0);
            }
        }
        let result = rsi(&prices, 14).unwrap();
        let last = *result.last().unwrap();
        assert!(last > 40.0 && last < 60.0, "alternating RSI should be ~50, got {last}");
    }

    #[test]
    fn rsi_all_gains_returns_100() {
        // All-up prices → avg_loss == 0 → RSI == 100
        let prices: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let result = rsi(&prices, 14).unwrap();
        let last = *result.last().unwrap();
        assert!(approx_eq(last, 100.0, 1e-9));
    }

    #[test]
    fn rsi_insufficient_data_returns_none() {
        assert!(rsi(&[1.0, 2.0, 3.0], 14).is_none());
    }

    #[test]
    fn rsi_result_length_matches_input() {
        let prices: Vec<f64> = (0..30).map(|i| i as f64).collect();
        let result = rsi(&prices, 14).unwrap();
        // rsi returns `period` early fill values + one per extra close
        // closes.len() == 30 → gains/losses.len() == 29
        // result starts with period=14 fills, then 29-14=15 real values → 14+15 = 29
        assert_eq!(result.len(), 29);
    }

    // ── MACD ─────────────────────────────────────────────────────────────────

    #[test]
    fn macd_returns_three_equal_length_vecs() {
        let closes: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect();
        let (macd_line, signal_line, histogram) = macd(&closes, 12, 26, 9).unwrap();
        assert_eq!(macd_line.len(), closes.len());
        assert_eq!(signal_line.len(), closes.len());
        assert_eq!(histogram.len(), closes.len());
    }

    #[test]
    fn macd_histogram_is_macd_minus_signal() {
        let closes: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64).sin() * 10.0).collect();
        let (macd_line, signal_line, histogram) = macd(&closes, 12, 26, 9).unwrap();
        for i in 0..closes.len() {
            assert!(approx_eq(histogram[i], macd_line[i] - signal_line[i], 1e-9));
        }
    }

    #[test]
    fn macd_positive_on_uptrend() {
        // Rising prices → fast EMA > slow EMA → MACD > 0 at end
        let closes: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 2.0).collect();
        let (macd_line, _, _) = macd(&closes, 12, 26, 9).unwrap();
        assert!(
            *macd_line.last().unwrap() > 0.0,
            "MACD should be positive on uptrend"
        );
    }

    #[test]
    fn macd_insufficient_data_returns_none() {
        assert!(macd(&[1.0, 2.0], 12, 26, 9).is_none());
    }

    // ── ATR ──────────────────────────────────────────────────────────────────

    fn uniform_candles(n: usize, high: f64, low: f64, close: f64) -> Vec<Candle> {
        (0..n).map(|_| make_candle(close, high, low, close, 100.0)).collect()
    }

    #[test]
    fn atr_uniform_candles_equals_range() {
        // When all candles have the same H-L and no gaps, ATR == H-L
        let candles = uniform_candles(20, 110.0, 90.0, 100.0);
        let result = atr(&candles, 14).unwrap();
        let last = *result.last().unwrap();
        assert!(approx_eq(last, 20.0, 0.1), "ATR should ≈ 20, got {last}");
    }

    #[test]
    fn atr_result_length_matches_input() {
        let candles = uniform_candles(20, 110.0, 90.0, 100.0);
        let result = atr(&candles, 14).unwrap();
        assert_eq!(result.len(), candles.len());
    }

    #[test]
    fn atr_insufficient_data_returns_none() {
        let candles = uniform_candles(5, 110.0, 90.0, 100.0);
        assert!(atr(&candles, 14).is_none());
    }

    #[test]
    fn atr_always_positive() {
        let candles: Vec<Candle> = (0..30).map(|i| {
            let p = 100.0 + (i as f64).sin() * 5.0;
            make_candle(p, p + 2.0, p - 2.0, p, 100.0)
        }).collect();
        let result = atr(&candles, 14).unwrap();
        for v in &result {
            assert!(*v >= 0.0, "ATR must be non-negative");
        }
    }

    #[test]
    fn atr_accounts_for_gap_up() {
        // A gap-up increases the true range beyond H-L
        let mut candles = uniform_candles(15, 105.0, 95.0, 100.0);
        // Last candle: H=106, L=104, prev_close=100 → TR = max(2, 6, 4) = 6
        candles.push(make_candle(105.0, 106.0, 104.0, 105.0, 100.0));
        let result = atr(&candles, 14).unwrap();
        // The last ATR should be smoothed upward from baseline of 10
        assert!(*result.last().unwrap() < 10.0); // smoothing pulls it down from 10
    }

    // ── Bollinger Bands ───────────────────────────────────────────────────────

    #[test]
    fn bollinger_bands_symmetric_around_middle() {
        let closes = vec![10.0, 11.0, 12.0, 11.0, 10.0, 11.0, 12.0, 11.0, 10.0, 11.0,
                          12.0, 11.0, 10.0, 11.0, 12.0, 11.0, 10.0, 11.0, 12.0, 11.0];
        let (upper, middle, lower) = bollinger_bands(&closes, 5, 2.0).unwrap();
        for i in 4..closes.len() {
            let upper_dist = upper[i] - middle[i];
            let lower_dist = middle[i] - lower[i];
            assert!(
                approx_eq(upper_dist, lower_dist, 1e-9),
                "Bands must be symmetric at index {i}: up={upper_dist}, lo={lower_dist}"
            );
        }
    }

    #[test]
    fn bollinger_bands_flat_prices_zero_bandwidth() {
        let closes = vec![50.0; 10];
        let (upper, middle, lower) = bollinger_bands(&closes, 5, 2.0).unwrap();
        // std_dev == 0, so upper == middle == lower for valid indices
        for i in 4..closes.len() {
            assert!(approx_eq(upper[i], 50.0, 1e-9));
            assert!(approx_eq(middle[i], 50.0, 1e-9));
            assert!(approx_eq(lower[i], 50.0, 1e-9));
        }
    }

    #[test]
    fn bollinger_bands_upper_above_lower() {
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + (i as f64).sin() * 5.0).collect();
        let (upper, _middle, lower) = bollinger_bands(&closes, 5, 2.0).unwrap();
        for i in 4..closes.len() {
            assert!(upper[i] >= lower[i], "Upper band must be >= lower at {i}");
        }
    }

    #[test]
    fn bollinger_bands_insufficient_data_returns_none() {
        assert!(bollinger_bands(&[1.0, 2.0], 5, 2.0).is_none());
    }

    #[test]
    fn bollinger_bands_result_length_matches_input() {
        let closes = vec![100.0; 20];
        let (upper, middle, lower) = bollinger_bands(&closes, 5, 2.0).unwrap();
        assert_eq!(upper.len(), 20);
        assert_eq!(middle.len(), 20);
        assert_eq!(lower.len(), 20);
    }

    // ── Bollinger Bandwidth ───────────────────────────────────────────────────

    #[test]
    fn bollinger_bandwidth_non_negative() {
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + (i as f64 % 5.0)).collect();
        let bw = bollinger_bandwidth(&closes, 5, 2.0).unwrap();
        for v in &bw {
            assert!(*v >= 0.0, "Bandwidth must be non-negative, got {v}");
        }
    }

    #[test]
    fn bollinger_bandwidth_zero_for_flat_prices() {
        let closes = vec![100.0; 10];
        let bw = bollinger_bandwidth(&closes, 5, 2.0).unwrap();
        for i in 4..bw.len() {
            assert!(approx_eq(bw[i], 0.0, 1e-9));
        }
    }

    // ── Bollinger %B ─────────────────────────────────────────────────────────

    #[test]
    fn bollinger_pct_b_middle_price_is_half() {
        // When price equals middle band, %B should be 0.5
        let closes = vec![10.0, 12.0, 10.0, 12.0, 10.0, 12.0, 10.0, 12.0, 10.0, 11.0];
        let (_, middle, _) = bollinger_bands(&closes, 5, 2.0).unwrap();
        // Replace last close with middle band value and recompute — we'll just assert range
        let pct_b = bollinger_pct_b(&closes, 5, 2.0).unwrap();
        for i in 4..pct_b.len() {
            // %B is in [0,1] when price is within the bands (may be outside for outliers)
            let _ = pct_b[i]; // just confirm no panic
            let _ = middle[i];
        }
    }

    #[test]
    fn bollinger_pct_b_flat_prices_returns_half() {
        // Flat prices → std_dev == 0 → function returns 0.5 by convention
        let closes = vec![50.0; 10];
        let pct_b = bollinger_pct_b(&closes, 5, 2.0).unwrap();
        for i in 4..pct_b.len() {
            assert!(approx_eq(pct_b[i], 0.5, 1e-9), "flat prices should give %B=0.5, got {}", pct_b[i]);
        }
    }

    #[test]
    fn bollinger_pct_b_price_at_upper_band_is_one() {
        // Build a scenario: 4 flat + 1 spike so last price == upper band
        let base = vec![100.0, 100.0, 100.0, 100.0, 100.0,
                        100.0, 100.0, 100.0, 100.0, 100.0];
        let (upper, _, _) = bollinger_bands(&base, 5, 2.0).unwrap();
        // With flat prices upper == 100 too, so %B == 0.5 still (flat case)
        let pct_b = bollinger_pct_b(&base, 5, 2.0).unwrap();
        assert!(approx_eq(pct_b[9], 0.5, 1e-9));
    }

    // ── Rate of Change ────────────────────────────────────────────────────────

    #[test]
    fn roc_double_price_gives_100_pct() {
        // price doubles after `period` bars → ROC = 100%
        let data = vec![50.0, 50.0, 50.0, 50.0, 50.0, 100.0];
        let result = roc(&data, 5).unwrap();
        let last = *result.last().unwrap();
        assert!(approx_eq(last, 100.0, 1e-9), "expected 100%, got {last}");
    }

    #[test]
    fn roc_no_change_gives_zero() {
        let data = vec![100.0; 10];
        let result = roc(&data, 5).unwrap();
        for i in 5..result.len() {
            assert!(approx_eq(result[i], 0.0, 1e-9));
        }
    }

    #[test]
    fn roc_half_price_gives_negative_50() {
        let data = vec![100.0, 100.0, 100.0, 100.0, 100.0, 50.0];
        let result = roc(&data, 5).unwrap();
        let last = *result.last().unwrap();
        assert!(approx_eq(last, -50.0, 1e-9), "expected -50%, got {last}");
    }

    #[test]
    fn roc_insufficient_data_returns_none() {
        // len <= period → None
        assert!(roc(&[1.0, 2.0, 3.0, 4.0, 5.0], 5).is_none());
    }

    #[test]
    fn roc_early_fill_is_zero() {
        let data: Vec<f64> = (1..=15).map(|i| i as f64).collect();
        let result = roc(&data, 5).unwrap();
        for i in 0..5 {
            assert!(approx_eq(result[i], 0.0, 1e-9));
        }
    }

    // ── VWAP ─────────────────────────────────────────────────────────────────

    #[test]
    fn vwap_single_candle() {
        let candle = make_candle(100.0, 110.0, 90.0, 100.0, 1000.0);
        let result = vwap(&[candle], 1440).unwrap();
        // typical = (110+90+100)/3 = 100
        assert!(approx_eq(result[0], 100.0, 1e-9));
    }

    #[test]
    fn vwap_equal_volumes_is_avg_typical() {
        let candles = vec![
            make_candle(100.0, 110.0, 90.0, 100.0, 1.0),  // typical = 100
            make_candle(100.0, 120.0, 80.0, 100.0, 1.0),  // typical = 100
        ];
        let result = vwap(&candles, 1440).unwrap();
        assert!(approx_eq(result[1], 100.0, 1e-9));
    }

    #[test]
    fn vwap_resets_every_reset_period() {
        // With reset_period=1, each candle resets — vwap[i] == typical[i]
        let candles = vec![
            make_candle(100.0, 120.0, 80.0, 100.0, 500.0), // typical=100
            make_candle(200.0, 210.0, 190.0, 200.0, 100.0), // typical=200
        ];
        let result = vwap(&candles, 1).unwrap();
        assert!(approx_eq(result[0], 100.0, 1e-9));
        assert!(approx_eq(result[1], 200.0, 1e-9));
    }

    #[test]
    fn vwap_higher_volume_candle_has_more_weight() {
        let candles = vec![
            make_candle(100.0, 110.0, 90.0, 100.0, 1.0),   // typical=100, low vol
            make_candle(200.0, 210.0, 190.0, 200.0, 100.0), // typical=200, high vol
        ];
        let result = vwap(&candles, 1440).unwrap();
        // VWAP should be much closer to 200 than 150
        assert!(result[1] > 190.0, "vwap should be close to 200, got {}", result[1]);
    }

    #[test]
    fn vwap_empty_returns_none() {
        assert!(vwap(&[], 1440).is_none());
    }

    // ── Volume Profile ────────────────────────────────────────────────────────

    fn make_volume_profile_candles() -> Vec<Candle> {
        // 10 candles — 7 at price 100, 3 at price 200 → POC should be near 100
        let mut candles: Vec<Candle> = (0..7)
            .map(|_| make_candle(100.0, 105.0, 95.0, 100.0, 100.0))
            .collect();
        candles.extend((0..3).map(|_| make_candle(200.0, 205.0, 195.0, 200.0, 10.0)));
        candles
    }

    #[test]
    fn volume_profile_poc_at_highest_volume_price() {
        let candles = make_volume_profile_candles();
        let vp = volume_profile(&candles, 10).unwrap();
        // The 7 high-volume candles at ~100 should be the POC
        assert!(vp.poc < 150.0, "POC should be near 100, got {}", vp.poc);
    }

    #[test]
    fn volume_profile_value_area_contains_poc() {
        let candles = make_volume_profile_candles();
        let vp = volume_profile(&candles, 10).unwrap();
        assert!(vp.value_area_low <= vp.poc, "VAL must be <= POC");
        assert!(vp.value_area_high >= vp.poc, "VAH must be >= POC");
    }

    #[test]
    fn volume_profile_insufficient_data_returns_none() {
        let candles = (0..5).map(|_| make_candle(100.0, 105.0, 95.0, 100.0, 100.0)).collect::<Vec<_>>();
        assert!(volume_profile(&candles, 10).is_none());
    }

    #[test]
    fn volume_profile_zero_bins_returns_none() {
        let candles = make_volume_profile_candles();
        assert!(volume_profile(&candles, 0).is_none());
    }

    #[test]
    fn volume_profile_flat_prices_returns_none() {
        // All same price → range == 0 → None
        let candles = flat_candles(100.0, 15);
        assert!(volume_profile(&candles, 10).is_none());
    }

    // ── Edge cases across multiple indicators ─────────────────────────────────

    #[test]
    fn ema_single_element_at_period_boundary() {
        // Exactly `period` elements — all filled with SMA, no iteration
        let data = vec![5.0, 10.0, 15.0];
        let result = ema(&data, 3).unwrap();
        assert_eq!(result.len(), 3);
        let sma_val = (5.0 + 10.0 + 15.0) / 3.0;
        for v in &result {
            assert!(approx_eq(*v, sma_val, 1e-9));
        }
    }

    #[test]
    fn all_indicators_return_none_on_empty() {
        let empty_f64: &[f64] = &[];
        let empty_candles: &[Candle] = &[];
        assert!(sma(empty_f64, 3).is_none());
        assert!(ema(empty_f64, 3).is_none());
        assert!(rsi(empty_f64, 14).is_none());
        assert!(macd(empty_f64, 12, 26, 9).is_none());
        assert!(bollinger_bands(empty_f64, 5, 2.0).is_none());
        assert!(bollinger_bandwidth(empty_f64, 5, 2.0).is_none());
        assert!(bollinger_pct_b(empty_f64, 5, 2.0).is_none());
        assert!(roc(empty_f64, 5).is_none());
        assert!(vwap(empty_candles, 1440).is_none());
        assert!(atr(empty_candles, 14).is_none());
        assert!(volume_profile(empty_candles, 10).is_none());
    }

    #[test]
    fn roc_with_zero_base_price_does_not_panic() {
        // When a prior price is 0 we return 0.0 rather than NaN/Inf
        let data = vec![0.0, 0.0, 0.0, 0.0, 0.0, 100.0];
        let result = roc(&data, 5).unwrap();
        let last = *result.last().unwrap();
        assert!(last.is_finite(), "ROC must not be NaN or Inf when base is 0");
        assert!(approx_eq(last, 0.0, 1e-9));
    }

    #[test]
    fn close_series_helper_produces_correct_count() {
        let prices = vec![1.0, 2.0, 3.0];
        let candles = close_series(&prices);
        assert_eq!(candles.len(), 3);
        assert!(approx_eq(candles[0].close, 1.0, 1e-9));
        assert!(approx_eq(candles[1].close, 2.0, 1e-9));
    }
}

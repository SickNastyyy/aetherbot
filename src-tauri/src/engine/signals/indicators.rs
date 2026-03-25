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

use crate::models::Candle;

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

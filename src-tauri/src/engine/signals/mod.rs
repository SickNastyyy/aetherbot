pub mod indicators;

use crate::models::{Candle, Signal, Direction};
use std::collections::HashMap;

/// Compute a composite signal from candle data
pub fn compute_signal(
    pair: &str,
    exchange: &str,
    candles: &[Candle],
) -> Option<Signal> {
    if candles.len() < 26 {
        return None; // Need at least 26 candles for MACD
    }

    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let mut indicator_values = HashMap::new();
    let mut votes: Vec<Direction> = Vec::new();

    // RSI
    if let Some(rsi_vals) = indicators::rsi(&closes, 14) {
        let rsi = *rsi_vals.last().unwrap();
        indicator_values.insert("rsi".into(), rsi);
        if rsi < 30.0 {
            votes.push(Direction::Long);
        } else if rsi > 70.0 {
            votes.push(Direction::Short);
        } else {
            votes.push(Direction::Neutral);
        }
    }

    // EMA crossover (9/21)
    if let (Some(fast), Some(slow)) = (
        indicators::ema(&closes, 9),
        indicators::ema(&closes, 21),
    ) {
        let f = *fast.last().unwrap();
        let s = *slow.last().unwrap();
        indicator_values.insert("ema_fast".into(), f);
        indicator_values.insert("ema_slow".into(), s);
        if f > s {
            votes.push(Direction::Long);
        } else if f < s {
            votes.push(Direction::Short);
        } else {
            votes.push(Direction::Neutral);
        }
    }

    // MACD
    if let Some((macd_line, signal_line, _hist)) = indicators::macd(&closes, 12, 26, 9) {
        let m = *macd_line.last().unwrap();
        let s = *signal_line.last().unwrap();
        indicator_values.insert("macd".into(), m);
        indicator_values.insert("macd_signal".into(), s);
        if m > s {
            votes.push(Direction::Long);
        } else {
            votes.push(Direction::Short);
        }
    }

    // ATR (informational, not directional)
    if let Some(atr_vals) = indicators::atr(candles, 14) {
        indicator_values.insert("atr".into(), *atr_vals.last().unwrap());
    }

    if votes.is_empty() {
        return None;
    }

    // Aggregate: majority vote
    let long_count = votes.iter().filter(|v| **v == Direction::Long).count();
    let short_count = votes.iter().filter(|v| **v == Direction::Short).count();
    let total = votes.len() as f64;

    let (direction, strength) = if long_count > short_count {
        (Direction::Long, long_count as f64 / total)
    } else if short_count > long_count {
        (Direction::Short, short_count as f64 / total)
    } else {
        (Direction::Neutral, 0.0)
    };

    Some(Signal {
        pair: pair.to_string(),
        exchange: exchange.to_string(),
        direction,
        strength,
        indicators: indicator_values,
    })
}

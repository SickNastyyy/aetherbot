use crate::models::Side;
use super::OrderResult;

/// Simulate a trade fill with realistic slippage
pub fn simulate_fill(current_price: f64, quantity: f64, side: &Side) -> OrderResult {
    // Random slippage between 0.05% and 0.30%
    let slippage_pct = 0.05 + rand::random::<f64>() * 0.25;
    let slippage_mult = slippage_pct / 100.0;

    let filled_price = match side {
        Side::Long => current_price * (1.0 + slippage_mult),  // buy slightly higher
        Side::Short => current_price * (1.0 - slippage_mult), // sell slightly lower
    };

    // Simulate trading fee (0.1%)
    let fee = filled_price * quantity * 0.001;

    OrderResult {
        success: true,
        filled_price,
        filled_quantity: quantity,
        slippage_pct,
        fee,
        message: format!(
            "Paper fill: {:.6} @ ${:.4} (slippage: {:.2}%, fee: ${:.4})",
            quantity, filled_price, slippage_pct, fee
        ),
    }
}

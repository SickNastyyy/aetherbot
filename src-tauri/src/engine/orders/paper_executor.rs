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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Side;

    // Run multiple fills and check invariants that must hold on every sample.
    // Because slippage is random we run several iterations.
    const ITERATIONS: usize = 100;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() < tol }

    // ── always succeeds ───────────────────────────────────────────────────────

    #[test]
    fn fill_always_succeeds() {
        for _ in 0..ITERATIONS {
            let result = simulate_fill(1000.0, 1.0, &Side::Long);
            assert!(result.success, "paper fill must always succeed");
        }
    }

    #[test]
    fn fill_quantity_unchanged() {
        let qty = 2.5;
        for _ in 0..ITERATIONS {
            let result = simulate_fill(1000.0, qty, &Side::Long);
            assert!(
                approx_eq(result.filled_quantity, qty, 1e-9),
                "filled_quantity must match input quantity"
            );
        }
    }

    // ── slippage direction ────────────────────────────────────────────────────

    #[test]
    fn long_fill_price_higher_than_market() {
        for _ in 0..ITERATIONS {
            let result = simulate_fill(1000.0, 1.0, &Side::Long);
            assert!(
                result.filled_price > 1000.0,
                "long fill must be above market price, got {}",
                result.filled_price
            );
        }
    }

    #[test]
    fn short_fill_price_lower_than_market() {
        for _ in 0..ITERATIONS {
            let result = simulate_fill(1000.0, 1.0, &Side::Short);
            assert!(
                result.filled_price < 1000.0,
                "short fill must be below market price, got {}",
                result.filled_price
            );
        }
    }

    // ── slippage bounds ───────────────────────────────────────────────────────

    #[test]
    fn slippage_pct_within_documented_range() {
        // Documented range: 0.05% – 0.30%
        for _ in 0..ITERATIONS {
            let result = simulate_fill(1000.0, 1.0, &Side::Long);
            assert!(
                result.slippage_pct >= 0.05,
                "slippage below minimum 0.05%, got {}",
                result.slippage_pct
            );
            assert!(
                result.slippage_pct <= 0.30,
                "slippage above maximum 0.30%, got {}",
                result.slippage_pct
            );
        }
    }

    // ── fee calculation ───────────────────────────────────────────────────────

    #[test]
    fn fee_is_positive() {
        for _ in 0..ITERATIONS {
            let result = simulate_fill(1000.0, 1.0, &Side::Long);
            assert!(result.fee > 0.0, "fee must be positive");
        }
    }

    #[test]
    fn fee_equals_01_pct_of_filled_value() {
        // fee = filled_price * quantity * 0.001
        for _ in 0..ITERATIONS {
            let qty = 3.0;
            let result = simulate_fill(500.0, qty, &Side::Long);
            let expected_fee = result.filled_price * qty * 0.001;
            assert!(
                approx_eq(result.fee, expected_fee, 1e-9),
                "fee should be 0.1% of filled value, expected {expected_fee:.6}, got {:.6}",
                result.fee
            );
        }
    }

    // ── filled price is different from input price ────────────────────────────

    #[test]
    fn filled_price_differs_from_market_price() {
        // With slippage always > 0, filled != market for both sides
        let long_result  = simulate_fill(1000.0, 1.0, &Side::Long);
        let short_result = simulate_fill(1000.0, 1.0, &Side::Short);
        assert_ne!(long_result.filled_price,  1000.0, "long fill price should differ from market");
        assert_ne!(short_result.filled_price, 1000.0, "short fill price should differ from market");
    }

    // ── message format ────────────────────────────────────────────────────────

    #[test]
    fn message_contains_paper_fill_prefix() {
        let result = simulate_fill(1000.0, 1.0, &Side::Long);
        assert!(
            result.message.starts_with("Paper fill:"),
            "message should start with 'Paper fill:', got: {}",
            result.message
        );
    }

    // ── zero price edge case ──────────────────────────────────────────────────

    #[test]
    fn zero_price_produces_zero_fee() {
        let result = simulate_fill(0.0, 1.0, &Side::Long);
        // filled_price = 0 * (1 + slip) = 0 → fee = 0 * qty * 0.001 = 0
        assert!(approx_eq(result.fee, 0.0, 1e-9));
    }

    // ── large quantity ────────────────────────────────────────────────────────

    #[test]
    fn large_quantity_scales_fee_correctly() {
        let qty = 10_000.0;
        let result = simulate_fill(100.0, qty, &Side::Short);
        let expected = result.filled_price * qty * 0.001;
        assert!(approx_eq(result.fee, expected, 1e-6));
    }
}

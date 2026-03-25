pub mod paper_executor;
pub mod jupiter_executor;
pub mod uniswap_executor;
pub mod kalshi_executor;

use crate::models::*;
use crate::engine::risk::ApprovedTrade;

/// Result of an order execution attempt
#[derive(Debug, Clone)]
pub struct OrderResult {
    pub success: bool,
    pub filled_price: f64,
    pub filled_quantity: f64,
    pub slippage_pct: f64,
    pub fee: f64,
    pub message: String,
}

/// Manages order execution and position tracking
pub struct OrderManager {
    positions: Vec<Position>,
    paper_balance: f64,
    trade_count: u32,
    closed_pnl: f64,
    winning_trades: u32,
}

impl OrderManager {
    pub fn new(paper_balance: f64) -> Self {
        Self {
            positions: Vec::new(),
            paper_balance,
            trade_count: 0,
            closed_pnl: 0.0,
            winning_trades: 0,
        }
    }

    /// Execute an approved trade (paper mode)
    pub fn execute_paper_trade(&mut self, trade: &ApprovedTrade, current_price: f64) -> OrderResult {
        let result = paper_executor::simulate_fill(current_price, trade.quantity, &trade.side);

        if result.success {
            let time_limit = chrono::Utc::now()
                + chrono::Duration::hours(trade.time_limit_hours as i64);

            let pnl = match trade.side {
                Side::Long => (current_price - trade.entry_price) * trade.quantity,
                Side::Short => (trade.entry_price - current_price) * trade.quantity,
            };

            let position = Position {
                id: format!("paper-{}", self.trade_count + 1),
                exchange: trade.exchange.clone(),
                pair: trade.pair.clone(),
                side: trade.side,
                entry_price: result.filled_price,
                current_price,
                quantity: result.filled_quantity,
                unrealized_pnl: pnl,
                unrealized_pnl_pct: if trade.entry_price > 0.0 {
                    (pnl / (trade.entry_price * trade.quantity)) * 100.0
                } else {
                    0.0
                },
                stop_loss: trade.stop_loss,
                take_profit: trade.take_profit,
                time_limit: time_limit.to_rfc3339(),
                strategy: trade.strategy.clone(),
                opened_at: chrono::Utc::now().to_rfc3339(),
            };

            self.positions.push(position);
            self.trade_count += 1;
            self.paper_balance -= result.filled_quantity * result.filled_price + result.fee;
        }

        result
    }

    /// Close a position (paper mode)
    pub fn close_position(&mut self, position_id: &str, current_price: f64, reason: ExitReason) -> Option<(Position, f64)> {
        let idx = self.positions.iter().position(|p| p.id == position_id)?;
        let mut pos = self.positions.remove(idx);

        let pnl = match pos.side {
            Side::Long => (current_price - pos.entry_price) * pos.quantity,
            Side::Short => (pos.entry_price - current_price) * pos.quantity,
        };

        pos.current_price = current_price;
        pos.unrealized_pnl = pnl;

        self.closed_pnl += pnl;
        self.paper_balance += pos.quantity * current_price;
        if pnl > 0.0 {
            self.winning_trades += 1;
        }

        Some((pos, pnl))
    }

    /// Update all position current prices and P&L
    pub fn update_prices(&mut self, prices: &std::collections::HashMap<String, f64>) {
        for pos in &mut self.positions {
            let key = format!("{}:{}", pos.exchange, pos.pair);
            let simple_key = &pos.pair;

            let price = prices.get(&key)
                .or_else(|| prices.get(simple_key.as_str()));

            if let Some(&current_price) = price {
                pos.current_price = current_price;
                let pnl = match pos.side {
                    Side::Long => (current_price - pos.entry_price) * pos.quantity,
                    Side::Short => (pos.entry_price - current_price) * pos.quantity,
                };
                pos.unrealized_pnl = pnl;
                pos.unrealized_pnl_pct = if pos.entry_price > 0.0 {
                    (pnl / (pos.entry_price * pos.quantity)) * 100.0
                } else {
                    0.0
                };
            }
        }
    }

    pub fn positions(&self) -> &[Position] {
        &self.positions
    }

    pub fn open_position_count(&self) -> u32 {
        self.positions.len() as u32
    }

    pub fn paper_balance(&self) -> f64 {
        self.paper_balance
    }

    pub fn total_equity(&self) -> f64 {
        let unrealized: f64 = self.positions.iter()
            .map(|p| p.quantity * p.current_price)
            .sum();
        self.paper_balance + unrealized
    }

    pub fn pnl_summary(&self) -> PnlSummary {
        let unrealized_pnl: f64 = self.positions.iter().map(|p| p.unrealized_pnl).sum();
        let total_pnl = self.closed_pnl + unrealized_pnl;
        let equity = self.total_equity();
        let total_pnl_pct = if equity > 0.0 { (total_pnl / equity) * 100.0 } else { 0.0 };

        PnlSummary {
            total_pnl,
            total_pnl_pct,
            today_pnl: total_pnl, // simplified: all in one session
            win_rate: if self.trade_count > 0 {
                (self.winning_trades as f64 / self.trade_count as f64) * 100.0
            } else {
                0.0
            },
            total_trades: self.trade_count,
            active_positions: self.positions.len() as u32,
        }
    }

    /// Get IDs of all open positions (for liquidation)
    pub fn all_position_ids(&self) -> Vec<String> {
        self.positions.iter().map(|p| p.id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::risk::ApprovedTrade;
    use crate::models::{ExitReason, Side};

    // ── helpers ───────────────────────────────────────────────────────────────

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() < tol }

    fn approved_long(entry: f64, qty: f64) -> ApprovedTrade {
        ApprovedTrade {
            pair: "BTC/USD".into(),
            exchange: "paper".into(),
            side: Side::Long,
            quantity: qty,
            entry_price: entry,
            stop_loss: entry * 0.95,
            take_profit: entry * 1.10,
            time_limit_hours: 24,
            strategy: "test".into(),
            reason: "unit test".into(),
        }
    }

    fn approved_short(entry: f64, qty: f64) -> ApprovedTrade {
        ApprovedTrade {
            pair: "ETH/USD".into(),
            exchange: "paper".into(),
            side: Side::Short,
            quantity: qty,
            entry_price: entry,
            stop_loss: entry * 1.05,
            take_profit: entry * 0.90,
            time_limit_hours: 24,
            strategy: "test".into(),
            reason: "unit test".into(),
        }
    }

    // ── position tracking: open ───────────────────────────────────────────────

    #[test]
    fn execute_paper_trade_opens_position() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        assert_eq!(om.open_position_count(), 1);
    }

    #[test]
    fn execute_paper_trade_returns_success() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        assert!(result.success);
    }

    #[test]
    fn position_id_is_sequential() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 0.1), 1000.0);
        om.execute_paper_trade(&approved_long(1000.0, 0.1), 1000.0);
        let ids = om.all_position_ids();
        assert_eq!(ids[0], "paper-1");
        assert_eq!(ids[1], "paper-2");
    }

    #[test]
    fn position_records_correct_side() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        assert_eq!(om.positions()[0].side, Side::Long);

        let mut om2 = OrderManager::new(10_000.0);
        om2.execute_paper_trade(&approved_short(1000.0, 1.0), 1000.0);
        assert_eq!(om2.positions()[0].side, Side::Short);
    }

    // ── balance changes on entry ──────────────────────────────────────────────

    #[test]
    fn paper_balance_decreases_on_long_entry() {
        let initial = 10_000.0;
        let mut om = OrderManager::new(initial);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        assert!(
            om.paper_balance() < initial,
            "balance should decrease after buying, got {}",
            om.paper_balance()
        );
    }

    #[test]
    fn paper_balance_decreases_by_cost_plus_fee() {
        let initial = 10_000.0;
        let mut om = OrderManager::new(initial);
        let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        let expected_balance = initial - (result.filled_quantity * result.filled_price + result.fee);
        assert!(
            approx_eq(om.paper_balance(), expected_balance, 1e-6),
            "balance should be initial minus (cost + fee), expected {expected_balance:.4}, got {:.4}",
            om.paper_balance()
        );
    }

    // ── close position ────────────────────────────────────────────────────────

    #[test]
    fn close_position_removes_it_from_list() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        let id = om.all_position_ids()[0].clone();
        om.close_position(&id, 1100.0, ExitReason::TakeProfit);
        assert_eq!(om.open_position_count(), 0);
    }

    #[test]
    fn close_nonexistent_position_returns_none() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.close_position("nonexistent", 1000.0, ExitReason::Manual);
        assert!(result.is_none());
    }

    // ── P&L for longs ─────────────────────────────────────────────────────────

    #[test]
    fn long_position_pnl_positive_when_price_rises() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        let id = om.all_position_ids()[0].clone();
        // Close at price above filled_price
        let exit_price = result.filled_price + 100.0;
        let (_, pnl) = om.close_position(&id, exit_price, ExitReason::TakeProfit).unwrap();
        assert!(pnl > 0.0, "long P&L should be positive when price rises, got {pnl}");
    }

    #[test]
    fn long_position_pnl_negative_when_price_falls() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        let id = om.all_position_ids()[0].clone();
        let exit_price = result.filled_price - 100.0;
        let (_, pnl) = om.close_position(&id, exit_price, ExitReason::StopLoss).unwrap();
        assert!(pnl < 0.0, "long P&L should be negative when price falls, got {pnl}");
    }

    #[test]
    fn long_pnl_calculation_is_correct() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_long(1000.0, 2.0), 1000.0);
        // filled_price has slippage; P&L = (exit - entry) * qty
        let exit = result.filled_price + 50.0;
        let id = om.all_position_ids()[0].clone();
        let (_, pnl) = om.close_position(&id, exit, ExitReason::TakeProfit).unwrap();
        let expected = (exit - result.filled_price) * 2.0;
        assert!(
            approx_eq(pnl, expected, 1e-6),
            "long P&L expected {expected:.4}, got {pnl:.4}"
        );
    }

    // ── P&L for shorts ────────────────────────────────────────────────────────

    #[test]
    fn short_position_pnl_positive_when_price_falls() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_short(1000.0, 1.0), 1000.0);
        let id = om.all_position_ids()[0].clone();
        // Short profits when price falls
        let exit_price = result.filled_price - 100.0;
        let (_, pnl) = om.close_position(&id, exit_price, ExitReason::TakeProfit).unwrap();
        assert!(pnl > 0.0, "short P&L should be positive when price falls, got {pnl}");
    }

    #[test]
    fn short_position_pnl_negative_when_price_rises() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_short(1000.0, 1.0), 1000.0);
        let id = om.all_position_ids()[0].clone();
        let exit_price = result.filled_price + 100.0;
        let (_, pnl) = om.close_position(&id, exit_price, ExitReason::StopLoss).unwrap();
        assert!(pnl < 0.0, "short P&L should be negative when price rises, got {pnl}");
    }

    #[test]
    fn short_pnl_calculation_is_correct() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_short(1000.0, 3.0), 1000.0);
        // P&L for short = (entry - exit) * qty
        let exit = result.filled_price - 50.0;
        let id = om.all_position_ids()[0].clone();
        let (_, pnl) = om.close_position(&id, exit, ExitReason::TakeProfit).unwrap();
        let expected = (result.filled_price - exit) * 3.0;
        assert!(
            approx_eq(pnl, expected, 1e-6),
            "short P&L expected {expected:.4}, got {pnl:.4}"
        );
    }

    // ── balance after close ───────────────────────────────────────────────────

    #[test]
    fn paper_balance_increases_after_closing_profitable_long() {
        let mut om = OrderManager::new(10_000.0);
        let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        let balance_after_entry = om.paper_balance();
        let id = om.all_position_ids()[0].clone();
        om.close_position(&id, result.filled_price + 200.0, ExitReason::TakeProfit);
        assert!(
            om.paper_balance() > balance_after_entry,
            "balance should increase after closing profitable long"
        );
    }

    // ── update_prices ─────────────────────────────────────────────────────────

    #[test]
    fn update_prices_updates_unrealized_pnl_for_long() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);

        let entry = om.positions()[0].entry_price;
        let new_price = entry + 200.0;
        let mut prices = std::collections::HashMap::new();
        prices.insert("BTC/USD".to_string(), new_price);

        om.update_prices(&prices);

        let pnl = om.positions()[0].unrealized_pnl;
        assert!(pnl > 0.0, "unrealized P&L should be positive after price rise, got {pnl}");
        assert!(
            approx_eq(pnl, 200.0, 1.0),
            "unrealized P&L should be ~200, got {pnl:.4}"
        );
    }

    #[test]
    fn update_prices_with_exchange_prefixed_key() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        let entry = om.positions()[0].entry_price;
        let new_price = entry + 100.0;

        let mut prices = std::collections::HashMap::new();
        // Key format: "exchange:pair"
        prices.insert("paper:BTC/USD".to_string(), new_price);
        om.update_prices(&prices);

        assert!(
            approx_eq(om.positions()[0].current_price, new_price, 1e-9),
            "update_prices should support 'exchange:pair' key"
        );
    }

    // ── trade count & win rate ────────────────────────────────────────────────

    #[test]
    fn trade_count_increments_per_open() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 0.1), 1000.0);
        om.execute_paper_trade(&approved_long(1000.0, 0.1), 1000.0);
        assert_eq!(om.pnl_summary().total_trades, 2);
    }

    #[test]
    fn win_rate_100_pct_after_all_winning_trades() {
        let mut om = OrderManager::new(50_000.0);
        // Open and close two profitable longs
        for _ in 0..2 {
            let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
            let id = om.all_position_ids().last().unwrap().clone();
            om.close_position(&id, result.filled_price + 100.0, ExitReason::TakeProfit);
        }
        let summary = om.pnl_summary();
        assert!(
            approx_eq(summary.win_rate, 100.0, 1e-9),
            "win rate should be 100%, got {}",
            summary.win_rate
        );
    }

    #[test]
    fn win_rate_0_pct_after_all_losing_trades() {
        let mut om = OrderManager::new(50_000.0);
        for _ in 0..2 {
            let result = om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
            let id = om.all_position_ids().last().unwrap().clone();
            om.close_position(&id, result.filled_price - 100.0, ExitReason::StopLoss);
        }
        let summary = om.pnl_summary();
        assert!(
            approx_eq(summary.win_rate, 0.0, 1e-9),
            "win rate should be 0%, got {}",
            summary.win_rate
        );
    }

    // ── total equity ──────────────────────────────────────────────────────────

    #[test]
    fn total_equity_includes_open_position_value() {
        let mut om = OrderManager::new(10_000.0);
        om.execute_paper_trade(&approved_long(1000.0, 1.0), 1000.0);
        // total_equity = cash + sum(qty * current_price)
        let pos = &om.positions()[0];
        let expected = om.paper_balance() + pos.quantity * pos.current_price;
        assert!(
            approx_eq(om.total_equity(), expected, 1e-6),
            "total equity should include open position value"
        );
    }

    #[test]
    fn initial_total_equity_equals_paper_balance() {
        let om = OrderManager::new(10_000.0);
        assert!(approx_eq(om.total_equity(), 10_000.0, 1e-9));
    }
}

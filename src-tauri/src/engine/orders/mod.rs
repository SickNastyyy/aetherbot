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

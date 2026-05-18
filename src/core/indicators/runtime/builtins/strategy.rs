//! Strategy namespace builtins for AionDSL.
//!
//! Provides strategy execution functions for backtesting and paper trading:
//! - Order functions: entry, exit, close, cancel, cancel_all
//! - Position tracking: position_size, position_avg_price, opentrades, closedtrades
//! - PnL tracking: netprofit, grossprofit, grossloss, openprofit
//!
//! Strategy state is maintained per-script execution and supports deterministic backtesting.

use crate::core::indicators::runtime::value::RayValue;
use std::collections::HashMap;

/// Direction of a trade or order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Long,
    Short,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Long => "long",
            Direction::Short => "short",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "long" | "strategy.long" => Some(Direction::Long),
            "short" | "strategy.short" => Some(Direction::Short),
            _ => None,
        }
    }
}

/// Order type for strategy execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

impl OrderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::Stop => "stop",
            OrderType::StopLimit => "stop_limit",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "market" | "order.market" => Some(OrderType::Market),
            "limit" | "order.limit" => Some(OrderType::Limit),
            "stop" | "order.stop" => Some(OrderType::Stop),
            "stop_limit" | "stoplimit" | "order.stop_limit" => Some(OrderType::StopLimit),
            _ => None,
        }
    }
}

/// A pending order in the strategy.
#[derive(Debug, Clone)]
pub struct Order {
    pub id: String,
    pub direction: Direction,
    pub qty: f64,
    pub order_type: OrderType,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub comment: Option<String>,
    pub bar_index: usize,
}

/// An open trade/position.
#[derive(Debug, Clone)]
pub struct Trade {
    pub entry_id: String,
    pub direction: Direction,
    pub qty: f64,
    pub entry_price: f64,
    pub entry_bar: usize,
    pub entry_time: i64,
    pub comment: Option<String>,
}

/// A closed trade with realized PnL.
#[derive(Debug, Clone)]
pub struct ClosedTrade {
    pub entry_id: String,
    pub exit_id: Option<String>,
    pub direction: Direction,
    pub qty: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub entry_bar: usize,
    pub exit_bar: usize,
    pub entry_time: i64,
    pub exit_time: i64,
    pub profit: f64,
    pub profit_percent: f64,
}

/// Strategy execution context.
/// Maintains all state for backtesting including positions, orders, and PnL.
#[derive(Debug, Clone)]
pub struct StrategyContext {
    /// Current bar index
    pub bar_index: usize,
    /// Current bar time (Unix timestamp ms)
    pub bar_time: i64,
    /// Current bar OHLC
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,

    /// Strategy parameters
    pub initial_capital: f64,
    pub default_qty_value: f64,
    pub default_qty_type: String,
    pub commission_type: String,
    pub commission_value: f64,
    pub slippage: i32,
    pub pyramiding: i32,
    pub process_orders_on_close: bool,

    /// Current position
    pub position_size: f64,
    pub position_avg_price: f64,

    /// Pending orders (keyed by order ID)
    pub pending_orders: HashMap<String, Order>,

    /// Open trades
    pub open_trades: Vec<Trade>,

    /// Closed trades history
    pub closed_trades: Vec<ClosedTrade>,

    /// Equity and PnL tracking
    pub equity: f64,
    pub netprofit: f64,
    pub grossprofit: f64,
    pub grossloss: f64,
    pub openprofit: f64,
    pub max_drawdown: f64,
    pub max_runup: f64,
    pub peak_equity: f64,

    /// Trade statistics
    pub wintrades: i32,
    pub losstrades: i32,
    pub eventrades: i32,
}

impl Default for StrategyContext {
    fn default() -> Self {
        Self {
            bar_index: 0,
            bar_time: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,

            initial_capital: 100_000.0,
            default_qty_value: 1.0,
            default_qty_type: "fixed".to_string(),
            commission_type: "percent".to_string(),
            commission_value: 0.0,
            slippage: 0,
            pyramiding: 1,
            process_orders_on_close: false,

            position_size: 0.0,
            position_avg_price: 0.0,

            pending_orders: HashMap::new(),
            open_trades: Vec::new(),
            closed_trades: Vec::new(),

            equity: 100_000.0,
            netprofit: 0.0,
            grossprofit: 0.0,
            grossloss: 0.0,
            openprofit: 0.0,
            max_drawdown: 0.0,
            max_runup: 0.0,
            peak_equity: 100_000.0,

            wintrades: 0,
            losstrades: 0,
            eventrades: 0,
        }
    }
}

impl StrategyContext {
    /// Create a new strategy context with the given initial capital.
    pub fn new(initial_capital: f64) -> Self {
        Self {
            initial_capital,
            equity: initial_capital,
            peak_equity: initial_capital,
            ..Default::default()
        }
    }

    /// Update bar data for current execution.
    pub fn set_bar(
        &mut self,
        bar_index: usize,
        time: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) {
        self.bar_index = bar_index;
        self.bar_time = time;
        self.open = open;
        self.high = high;
        self.low = low;
        self.close = close;
        self.update_open_profit();
    }

    /// Calculate commission for a trade.
    fn calculate_commission(&self, qty: f64, price: f64) -> f64 {
        match self.commission_type.as_str() {
            "percent" => qty * price * (self.commission_value / 100.0),
            "cash_per_contract" => qty.abs() * self.commission_value,
            "cash_per_order" => self.commission_value,
            _ => 0.0,
        }
    }

    /// Calculate fill price with slippage.
    fn apply_slippage(&self, price: f64, direction: Direction, is_entry: bool) -> f64 {
        let slippage_amount = self.slippage as f64 * 0.0001 * price; // Slippage as price ticks
        match (direction, is_entry) {
            (Direction::Long, true) | (Direction::Short, false) => price + slippage_amount,
            (Direction::Long, false) | (Direction::Short, true) => price - slippage_amount,
        }
    }

    /// Update open profit based on current close price.
    fn update_open_profit(&mut self) {
        self.openprofit = 0.0;
        for trade in &self.open_trades {
            let unrealized = match trade.direction {
                Direction::Long => (self.close - trade.entry_price) * trade.qty,
                Direction::Short => (trade.entry_price - self.close) * trade.qty,
            };
            self.openprofit += unrealized;
        }
        self.equity = self.initial_capital + self.netprofit + self.openprofit;

        // Track drawdown and runup
        if self.equity > self.peak_equity {
            self.peak_equity = self.equity;
            let runup = self.equity - self.initial_capital;
            if runup > self.max_runup {
                self.max_runup = runup;
            }
        }
        let drawdown = self.peak_equity - self.equity;
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
        }
    }

    /// Enter a position.
    pub fn entry(
        &mut self,
        id: &str,
        direction: Direction,
        qty: Option<f64>,
        limit: Option<f64>,
        stop: Option<f64>,
        comment: Option<String>,
    ) {
        let qty = qty.unwrap_or(self.default_qty_value);

        // Check pyramiding limit
        let same_direction_count = self
            .open_trades
            .iter()
            .filter(|t| t.direction == direction)
            .count() as i32;
        if same_direction_count >= self.pyramiding {
            return; // Pyramiding limit reached
        }

        // For market orders, execute immediately
        if limit.is_none() && stop.is_none() {
            self.execute_entry(id, direction, qty, self.close, comment);
        } else {
            // Add pending order
            self.pending_orders.insert(
                id.to_string(),
                Order {
                    id: id.to_string(),
                    direction,
                    qty,
                    order_type: if limit.is_some() && stop.is_some() {
                        OrderType::StopLimit
                    } else if limit.is_some() {
                        OrderType::Limit
                    } else {
                        OrderType::Stop
                    },
                    limit_price: limit,
                    stop_price: stop,
                    comment,
                    bar_index: self.bar_index,
                },
            );
        }
    }

    /// Execute an entry at a specific price.
    fn execute_entry(
        &mut self,
        id: &str,
        direction: Direction,
        qty: f64,
        price: f64,
        comment: Option<String>,
    ) {
        let fill_price = self.apply_slippage(price, direction, true);
        let commission = self.calculate_commission(qty, fill_price);

        // Create trade
        let trade = Trade {
            entry_id: id.to_string(),
            direction,
            qty,
            entry_price: fill_price,
            entry_bar: self.bar_index,
            entry_time: self.bar_time,
            comment,
        };

        // Update position
        let signed_qty = match direction {
            Direction::Long => qty,
            Direction::Short => -qty,
        };
        let old_position = self.position_size;
        self.position_size += signed_qty;

        // Update average price (weighted average for pyramiding)
        if old_position.signum() == signed_qty.signum() || old_position == 0.0 {
            let total_cost = self.position_avg_price * old_position.abs() + fill_price * qty;
            self.position_avg_price = total_cost / self.position_size.abs();
        } else {
            self.position_avg_price = fill_price;
        }

        // Deduct commission from equity
        self.netprofit -= commission;
        self.open_trades.push(trade);
        self.update_open_profit();
    }

    /// Exit a position.
    pub fn exit(
        &mut self,
        from_entry: Option<&str>,
        qty: Option<f64>,
        qty_percent: Option<f64>,
        limit: Option<f64>,
        stop: Option<f64>,
        comment: Option<String>,
    ) {
        if self.open_trades.is_empty() {
            return;
        }

        // Find trades to exit
        let trades_to_exit: Vec<Trade> = if let Some(entry_id) = from_entry {
            self.open_trades
                .iter()
                .filter(|t| t.entry_id == entry_id)
                .cloned()
                .collect()
        } else {
            self.open_trades.clone()
        };

        if trades_to_exit.is_empty() {
            return;
        }

        // For market orders, execute immediately
        if limit.is_none() && stop.is_none() {
            for trade in trades_to_exit {
                let exit_qty = if let Some(q) = qty {
                    q.min(trade.qty)
                } else if let Some(pct) = qty_percent {
                    trade.qty * (pct / 100.0)
                } else {
                    trade.qty
                };
                self.execute_exit(&trade, exit_qty, self.close, comment.clone());
            }
        }
        // TODO: Add pending exit orders for limit/stop
    }

    /// Execute an exit at a specific price.
    fn execute_exit(&mut self, trade: &Trade, qty: f64, price: f64, comment: Option<String>) {
        let fill_price = self.apply_slippage(price, trade.direction, false);
        let commission = self.calculate_commission(qty, fill_price);

        // Calculate profit
        let profit = match trade.direction {
            Direction::Long => (fill_price - trade.entry_price) * qty,
            Direction::Short => (trade.entry_price - fill_price) * qty,
        } - commission;

        let profit_percent = (profit / (trade.entry_price * qty)) * 100.0;

        // Create closed trade record
        let closed = ClosedTrade {
            entry_id: trade.entry_id.clone(),
            exit_id: comment.clone(),
            direction: trade.direction,
            qty,
            entry_price: trade.entry_price,
            exit_price: fill_price,
            entry_bar: trade.entry_bar,
            exit_bar: self.bar_index,
            entry_time: trade.entry_time,
            exit_time: self.bar_time,
            profit,
            profit_percent,
        };

        // Update PnL
        self.netprofit += profit;
        if profit > 0.0 {
            self.grossprofit += profit;
            self.wintrades += 1;
        } else if profit < 0.0 {
            self.grossloss += profit.abs();
            self.losstrades += 1;
        } else {
            self.eventrades += 1;
        }

        self.closed_trades.push(closed);

        // Update position
        let signed_qty = match trade.direction {
            Direction::Long => -qty,
            Direction::Short => qty,
        };
        self.position_size += signed_qty;
        if self.position_size.abs() < f64::EPSILON {
            self.position_size = 0.0;
            self.position_avg_price = 0.0;
        }

        // Remove or reduce open trade
        if let Some(idx) = self
            .open_trades
            .iter()
            .position(|t| t.entry_id == trade.entry_id)
        {
            if (self.open_trades[idx].qty - qty).abs() < f64::EPSILON {
                self.open_trades.remove(idx);
            } else {
                self.open_trades[idx].qty -= qty;
            }
        }

        self.update_open_profit();
    }

    /// Close all positions.
    pub fn close_all(&mut self, comment: Option<String>) {
        let trades = self.open_trades.clone();
        for trade in trades {
            self.execute_exit(&trade, trade.qty, self.close, comment.clone());
        }
    }

    /// Cancel a pending order.
    pub fn cancel(&mut self, id: &str) {
        self.pending_orders.remove(id);
    }

    /// Cancel all pending orders.
    pub fn cancel_all(&mut self) {
        self.pending_orders.clear();
    }

    /// Process pending orders against current bar.
    pub fn process_pending_orders(&mut self) {
        let orders: Vec<Order> = self.pending_orders.values().cloned().collect();
        let mut filled_ids = Vec::new();

        for order in orders {
            let filled = match order.order_type {
                OrderType::Limit => {
                    if let Some(limit) = order.limit_price {
                        match order.direction {
                            Direction::Long if self.low <= limit => Some(limit),
                            Direction::Short if self.high >= limit => Some(limit),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                OrderType::Stop => {
                    if let Some(stop) = order.stop_price {
                        match order.direction {
                            Direction::Long if self.high >= stop => Some(stop),
                            Direction::Short if self.low <= stop => Some(stop),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                OrderType::StopLimit => {
                    // Stop must be triggered, then limit must be reached
                    if let (Some(stop), Some(limit)) = (order.stop_price, order.limit_price) {
                        match order.direction {
                            Direction::Long if self.high >= stop && self.low <= limit => {
                                Some(limit)
                            }
                            Direction::Short if self.low <= stop && self.high >= limit => {
                                Some(limit)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                OrderType::Market => Some(self.open), // Market orders fill at open
            };

            if let Some(fill_price) = filled {
                self.execute_entry(
                    &order.id,
                    order.direction,
                    order.qty,
                    fill_price,
                    order.comment.clone(),
                );
                filled_ids.push(order.id);
            }
        }

        // Remove filled orders
        for id in filled_ids {
            self.pending_orders.remove(&id);
        }
    }
}

/// Dispatch strategy.* function calls.
pub fn call(name: &str, _args: &[RayValue], ctx: Option<&mut StrategyContext>) -> Option<RayValue> {
    match name {
        // Direction constants
        "long" => Some(RayValue::String("strategy.long".to_string())),
        "short" => Some(RayValue::String("strategy.short".to_string())),

        // Order type constants
        "order.market" | "market" => Some(RayValue::String("order.market".to_string())),
        "order.limit" | "limit" => Some(RayValue::String("order.limit".to_string())),
        "order.stop" | "stop" => Some(RayValue::String("order.stop".to_string())),
        "order.stop_limit" | "stop_limit" => Some(RayValue::String("order.stop_limit".to_string())),

        // OCA (one-cancels-all) constants
        "oca.none" | "oca_none" => Some(RayValue::String("strategy.oca.none".to_string())),
        "oca.cancel" | "oca_cancel" => Some(RayValue::String("strategy.oca.cancel".to_string())),
        "oca.reduce" | "oca_reduce" => Some(RayValue::String("strategy.oca.reduce".to_string())),

        // Commission type constants
        "commission.percent" => Some(RayValue::String("strategy.commission.percent".to_string())),
        "commission.cash_per_contract" => Some(RayValue::String(
            "strategy.commission.cash_per_contract".to_string(),
        )),
        "commission.cash_per_order" => Some(RayValue::String(
            "strategy.commission.cash_per_order".to_string(),
        )),

        // Position properties (require context)
        "position_size" => ctx
            .map(|c| RayValue::Number(c.position_size))
            .or(Some(RayValue::Number(0.0))),
        "position_avg_price" => ctx
            .map(|c| RayValue::Number(c.position_avg_price))
            .or(Some(RayValue::Number(0.0))),

        // Trade counts
        "opentrades" => ctx
            .map(|c| RayValue::Number(c.open_trades.len() as f64))
            .or(Some(RayValue::Number(0.0))),
        "closedtrades" => ctx
            .map(|c| RayValue::Number(c.closed_trades.len() as f64))
            .or(Some(RayValue::Number(0.0))),

        // PnL metrics
        "netprofit" => ctx
            .map(|c| RayValue::Number(c.netprofit))
            .or(Some(RayValue::Number(0.0))),
        "grossprofit" => ctx
            .map(|c| RayValue::Number(c.grossprofit))
            .or(Some(RayValue::Number(0.0))),
        "grossloss" => ctx
            .map(|c| RayValue::Number(c.grossloss))
            .or(Some(RayValue::Number(0.0))),
        "openprofit" => ctx
            .map(|c| RayValue::Number(c.openprofit))
            .or(Some(RayValue::Number(0.0))),

        // Equity metrics
        "equity" => ctx
            .map(|c| RayValue::Number(c.equity))
            .or(Some(RayValue::Number(0.0))),
        "initial_capital" => ctx
            .map(|c| RayValue::Number(c.initial_capital))
            .or(Some(RayValue::Number(100_000.0))),

        // Risk metrics
        "max_drawdown" => ctx
            .map(|c| RayValue::Number(c.max_drawdown))
            .or(Some(RayValue::Number(0.0))),
        "max_runup" => ctx
            .map(|c| RayValue::Number(c.max_runup))
            .or(Some(RayValue::Number(0.0))),

        // Trade statistics
        "wintrades" => ctx
            .map(|c| RayValue::Number(c.wintrades as f64))
            .or(Some(RayValue::Number(0.0))),
        "losstrades" => ctx
            .map(|c| RayValue::Number(c.losstrades as f64))
            .or(Some(RayValue::Number(0.0))),
        "eventrades" => ctx
            .map(|c| RayValue::Number(c.eventrades as f64))
            .or(Some(RayValue::Number(0.0))),

        // Computed metrics
        "percent_profitable" => ctx
            .map(|c| {
                let total = c.wintrades + c.losstrades + c.eventrades;
                if total == 0 {
                    RayValue::Number(0.0)
                } else {
                    RayValue::Number((c.wintrades as f64 / total as f64) * 100.0)
                }
            })
            .or(Some(RayValue::Number(0.0))),
        "profit_factor" => ctx
            .map(|c| {
                if c.grossloss == 0.0 {
                    RayValue::Na
                } else {
                    RayValue::Number(c.grossprofit / c.grossloss)
                }
            })
            .or(Some(RayValue::Na)),

        _ => None,
    }
}

/// Read-only call for strategy properties (no mutation).
pub fn call_readonly(
    name: &str,
    _args: &[RayValue],
    ctx: Option<&StrategyContext>,
) -> Option<RayValue> {
    match name {
        // Direction constants
        "long" => Some(RayValue::String("strategy.long".to_string())),
        "short" => Some(RayValue::String("strategy.short".to_string())),

        // Order type constants
        "order.market" | "market" => Some(RayValue::String("order.market".to_string())),
        "order.limit" | "limit" => Some(RayValue::String("order.limit".to_string())),
        "order.stop" | "stop" => Some(RayValue::String("order.stop".to_string())),
        "order.stop_limit" | "stop_limit" => Some(RayValue::String("order.stop_limit".to_string())),

        // OCA constants
        "oca.none" | "oca_none" => Some(RayValue::String("strategy.oca.none".to_string())),
        "oca.cancel" | "oca_cancel" => Some(RayValue::String("strategy.oca.cancel".to_string())),
        "oca.reduce" | "oca_reduce" => Some(RayValue::String("strategy.oca.reduce".to_string())),

        // Commission type constants
        "commission.percent" => Some(RayValue::String("strategy.commission.percent".to_string())),
        "commission.cash_per_contract" => Some(RayValue::String(
            "strategy.commission.cash_per_contract".to_string(),
        )),
        "commission.cash_per_order" => Some(RayValue::String(
            "strategy.commission.cash_per_order".to_string(),
        )),

        // Position properties
        "position_size" => ctx
            .map(|c| RayValue::Number(c.position_size))
            .or(Some(RayValue::Number(0.0))),
        "position_avg_price" => ctx
            .map(|c| RayValue::Number(c.position_avg_price))
            .or(Some(RayValue::Number(0.0))),

        // Trade counts
        "opentrades" => ctx
            .map(|c| RayValue::Number(c.open_trades.len() as f64))
            .or(Some(RayValue::Number(0.0))),
        "closedtrades" => ctx
            .map(|c| RayValue::Number(c.closed_trades.len() as f64))
            .or(Some(RayValue::Number(0.0))),

        // PnL metrics
        "netprofit" => ctx
            .map(|c| RayValue::Number(c.netprofit))
            .or(Some(RayValue::Number(0.0))),
        "grossprofit" => ctx
            .map(|c| RayValue::Number(c.grossprofit))
            .or(Some(RayValue::Number(0.0))),
        "grossloss" => ctx
            .map(|c| RayValue::Number(c.grossloss))
            .or(Some(RayValue::Number(0.0))),
        "openprofit" => ctx
            .map(|c| RayValue::Number(c.openprofit))
            .or(Some(RayValue::Number(0.0))),

        // Equity metrics
        "equity" => ctx
            .map(|c| RayValue::Number(c.equity))
            .or(Some(RayValue::Number(0.0))),
        "initial_capital" => ctx
            .map(|c| RayValue::Number(c.initial_capital))
            .or(Some(RayValue::Number(100_000.0))),

        // Risk metrics
        "max_drawdown" => ctx
            .map(|c| RayValue::Number(c.max_drawdown))
            .or(Some(RayValue::Number(0.0))),
        "max_runup" => ctx
            .map(|c| RayValue::Number(c.max_runup))
            .or(Some(RayValue::Number(0.0))),

        // Trade statistics
        "wintrades" => ctx
            .map(|c| RayValue::Number(c.wintrades as f64))
            .or(Some(RayValue::Number(0.0))),
        "losstrades" => ctx
            .map(|c| RayValue::Number(c.losstrades as f64))
            .or(Some(RayValue::Number(0.0))),
        "eventrades" => ctx
            .map(|c| RayValue::Number(c.eventrades as f64))
            .or(Some(RayValue::Number(0.0))),

        // Computed metrics
        "percent_profitable" => ctx
            .map(|c| {
                let total = c.wintrades + c.losstrades + c.eventrades;
                if total == 0 {
                    RayValue::Number(0.0)
                } else {
                    RayValue::Number((c.wintrades as f64 / total as f64) * 100.0)
                }
            })
            .or(Some(RayValue::Number(0.0))),
        "profit_factor" => ctx
            .map(|c| {
                if c.grossloss == 0.0 {
                    RayValue::Na
                } else {
                    RayValue::Number(c.grossprofit / c.grossloss)
                }
            })
            .or(Some(RayValue::Na)),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_constants() {
        assert_eq!(
            call("long", &[], None),
            Some(RayValue::String("strategy.long".to_string()))
        );
        assert_eq!(
            call("short", &[], None),
            Some(RayValue::String("strategy.short".to_string()))
        );
    }

    #[test]
    fn test_order_type_constants() {
        assert_eq!(
            call("order.market", &[], None),
            Some(RayValue::String("order.market".to_string()))
        );
        assert_eq!(
            call("order.limit", &[], None),
            Some(RayValue::String("order.limit".to_string()))
        );
    }

    #[test]
    fn test_strategy_context_default() {
        let ctx = StrategyContext::default();
        assert_eq!(ctx.initial_capital, 100_000.0);
        assert_eq!(ctx.position_size, 0.0);
        assert!(ctx.open_trades.is_empty());
        assert!(ctx.closed_trades.is_empty());
    }

    #[test]
    fn test_entry_long() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 102.0);

        ctx.entry("entry1", Direction::Long, Some(10.0), None, None, None);

        assert_eq!(ctx.position_size, 10.0);
        assert_eq!(ctx.open_trades.len(), 1);
        assert_eq!(ctx.open_trades[0].entry_id, "entry1");
        assert_eq!(ctx.open_trades[0].qty, 10.0);
    }

    #[test]
    fn test_entry_short() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 102.0);

        ctx.entry("entry1", Direction::Short, Some(5.0), None, None, None);

        assert_eq!(ctx.position_size, -5.0);
        assert_eq!(ctx.open_trades.len(), 1);
        assert_eq!(ctx.open_trades[0].direction, Direction::Short);
    }

    #[test]
    fn test_exit_trade() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 100.0);
        ctx.entry("entry1", Direction::Long, Some(10.0), None, None, None);

        ctx.set_bar(1, 2000, 102.0, 110.0, 101.0, 108.0);
        ctx.exit(Some("entry1"), None, None, None, None, None);

        assert_eq!(ctx.position_size, 0.0);
        assert!(ctx.open_trades.is_empty());
        assert_eq!(ctx.closed_trades.len(), 1);
        assert!(ctx.netprofit > 0.0); // Profitable trade
    }

    #[test]
    fn test_close_all() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 100.0);
        ctx.pyramiding = 5;

        ctx.entry("entry1", Direction::Long, Some(10.0), None, None, None);
        ctx.entry("entry2", Direction::Long, Some(5.0), None, None, None);

        assert_eq!(ctx.open_trades.len(), 2);

        ctx.set_bar(1, 2000, 102.0, 110.0, 101.0, 108.0);
        ctx.close_all(None);

        assert_eq!(ctx.position_size, 0.0);
        assert!(ctx.open_trades.is_empty());
        assert_eq!(ctx.closed_trades.len(), 2);
    }

    #[test]
    fn test_pyramiding_limit() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.pyramiding = 2;
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 100.0);

        ctx.entry("entry1", Direction::Long, Some(10.0), None, None, None);
        ctx.entry("entry2", Direction::Long, Some(10.0), None, None, None);
        ctx.entry("entry3", Direction::Long, Some(10.0), None, None, None); // Should not execute

        assert_eq!(ctx.open_trades.len(), 2); // Only 2 trades due to pyramiding limit
        assert_eq!(ctx.position_size, 20.0);
    }

    #[test]
    fn test_pending_limit_order() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 102.0);

        // Place limit order below current price
        ctx.entry(
            "limit_buy",
            Direction::Long,
            Some(10.0),
            Some(98.0),
            None,
            None,
        );
        assert_eq!(ctx.pending_orders.len(), 1);
        assert!(ctx.open_trades.is_empty());

        // Bar that doesn't reach limit
        ctx.set_bar(1, 2000, 101.0, 104.0, 99.0, 103.0);
        ctx.process_pending_orders();
        assert!(ctx.open_trades.is_empty()); // Limit not reached

        // Bar that reaches limit
        ctx.set_bar(2, 3000, 99.0, 100.0, 97.0, 99.0);
        ctx.process_pending_orders();
        assert_eq!(ctx.open_trades.len(), 1); // Limit filled
        assert!(ctx.pending_orders.is_empty());
    }

    #[test]
    fn test_cancel_order() {
        let mut ctx = StrategyContext::new(10_000.0);
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 102.0);

        ctx.entry(
            "limit_buy",
            Direction::Long,
            Some(10.0),
            Some(98.0),
            None,
            None,
        );
        assert_eq!(ctx.pending_orders.len(), 1);

        ctx.cancel("limit_buy");
        assert!(ctx.pending_orders.is_empty());
    }

    #[test]
    fn test_pnl_tracking() {
        let mut ctx = StrategyContext::new(10_000.0);

        // Winning trade
        ctx.set_bar(0, 1000, 100.0, 105.0, 95.0, 100.0);
        ctx.entry("win", Direction::Long, Some(10.0), None, None, None);
        ctx.set_bar(1, 2000, 105.0, 110.0, 104.0, 108.0);
        ctx.exit(Some("win"), None, None, None, None, None);

        assert!(ctx.grossprofit > 0.0);
        assert_eq!(ctx.wintrades, 1);

        // Losing trade
        ctx.set_bar(2, 3000, 108.0, 110.0, 107.0, 108.0);
        ctx.entry("loss", Direction::Long, Some(10.0), None, None, None);
        ctx.set_bar(3, 4000, 100.0, 102.0, 98.0, 99.0);
        ctx.exit(Some("loss"), None, None, None, None, None);

        assert!(ctx.grossloss > 0.0);
        assert_eq!(ctx.losstrades, 1);
        assert_eq!(ctx.closed_trades.len(), 2);
    }
}

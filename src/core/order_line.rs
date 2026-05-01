//! OrderLine — platform-style order management lines.
//!
//! Order lines display pending orders on the chart with:
//! - Order type labels (Limit, Stop, Take Profit, Stop Loss)
//! - Side indication (Buy/Sell) with appropriate colors
//! - Quantity display
//! - Draggable price modification
//! - Cancel/Modify button areas
//! - Status indication (Pending, PartiallyFilled, etc.)

use crate::core::series::LineStyle;

/// Unique identifier for an order line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderLineId(pub String);

impl OrderLineId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Order type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderType {
    #[default]
    Limit,
    Market,
    Stop,
    StopLimit,
    TakeProfit,
    StopLoss,
    TrailingStop,
}

impl OrderType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "limit" => Self::Limit,
            "market" => Self::Market,
            "stop" => Self::Stop,
            "stop_limit" | "stoplimit" | "stop-limit" => Self::StopLimit,
            "take_profit" | "takeprofit" | "tp" => Self::TakeProfit,
            "stop_loss" | "stoploss" | "sl" => Self::StopLoss,
            "trailing_stop" | "trailingstop" | "trailing" => Self::TrailingStop,
            _ => Self::Limit,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Limit => "Limit",
            Self::Market => "Market",
            Self::Stop => "Stop",
            Self::StopLimit => "Stop Limit",
            Self::TakeProfit => "Take Profit",
            Self::StopLoss => "Stop Loss",
            Self::TrailingStop => "Trailing Stop",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Limit => "LMT",
            Self::Market => "MKT",
            Self::Stop => "STP",
            Self::StopLimit => "STP LMT",
            Self::TakeProfit => "TP",
            Self::StopLoss => "SL",
            Self::TrailingStop => "TSL",
        }
    }
}

/// Order side (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderSide {
    #[default]
    Buy,
    Sell,
}

impl OrderSide {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sell" | "short" | "ask" => Self::Sell,
            _ => Self::Buy,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "Buy",
            Self::Sell => "Sell",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Buy => "B",
            Self::Sell => "S",
        }
    }
}

/// Order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderStatus {
    #[default]
    Pending,
    Working,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

impl OrderStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "working" | "active" => Self::Working,
            "partial" | "partially_filled" | "partiallyfilled" => Self::PartiallyFilled,
            "filled" | "complete" | "executed" => Self::Filled,
            "cancelled" | "canceled" => Self::Cancelled,
            "rejected" => Self::Rejected,
            "expired" => Self::Expired,
            _ => Self::Pending,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Working => "Working",
            Self::PartiallyFilled => "Partial",
            Self::Filled => "Filled",
            Self::Cancelled => "Cancelled",
            Self::Rejected => "Rejected",
            Self::Expired => "Expired",
        }
    }

    /// Whether this order is still active/modifiable.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Working | Self::PartiallyFilled)
    }
}

/// Default colors for order lines.
pub mod colors {
    /// Buy order color (blue) [R, G, B, A] in 0.0-1.0 range.
    pub const BUY_COLOR: [f32; 4] = [0.20, 0.36, 1.0, 1.0]; // #335CFF

    /// Sell order color (red) [R, G, B, A] in 0.0-1.0 range.
    pub const SELL_COLOR: [f32; 4] = [0.9843137, 0.21568628, 0.28235295, 1.0]; // #FB3748

    /// Take profit color (blue-green) [R, G, B, A] in 0.0-1.0 range.
    pub const TAKE_PROFIT_COLOR: [f32; 4] = BUY_COLOR;

    /// Stop loss color (orange-red) [R, G, B, A] in 0.0-1.0 range.
    pub const STOP_LOSS_COLOR: [f32; 4] = SELL_COLOR;

    /// Label text color (white).
    pub const LABEL_TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.95];

    /// Cancel button color (muted red).
    pub const CANCEL_BUTTON_COLOR: [f32; 4] = [0.6, 0.2, 0.2, 1.0];

    /// Modify button color (muted blue).
    pub const MODIFY_BUTTON_COLOR: [f32; 4] = [0.2, 0.4, 0.6, 1.0];
}

/// Configuration options for an order line.
#[derive(Debug, Clone)]
pub struct OrderLineOptions {
    /// The price level where the order is placed.
    pub price: f64,
    /// Trigger price for stop-limit orders (None for other types).
    pub trigger_price: Option<f64>,
    /// Order type.
    pub order_type: OrderType,
    /// Order side (buy/sell).
    pub side: OrderSide,
    /// Order status.
    pub status: OrderStatus,
    /// Order quantity.
    pub quantity: f64,
    /// Filled quantity (for partial fills).
    pub filled_quantity: f64,
    /// Line color override. If None, uses default based on side/type.
    pub color: Option<[f32; 4]>,
    /// Line width in CSS pixels.
    pub line_width: f64,
    /// Line dash style.
    pub line_style: LineStyle,
    /// Whether the line is visible.
    pub visible: bool,
    /// Whether the order can be cancelled via UI.
    pub cancellable: bool,
    /// Whether the price can be modified by dragging.
    pub modifiable: bool,
    /// Whether to show extended info (quantity, type) on the label.
    pub extended_label: bool,
    /// Custom label text (overrides auto-generated label).
    pub custom_label: Option<String>,
    /// Tooltip text shown on hover.
    pub tooltip: Option<String>,
    /// Label text color.
    pub label_text_color: [f32; 4],
    /// Body background color (on the chart, not axis label).
    pub body_bg_color: Option<[f32; 4]>,
    /// Whether to show the quantity in the label.
    pub show_quantity: bool,
    /// Whether to show the order type in the label.
    pub show_order_type: bool,
    /// Associated position/trade ID (for linking TP/SL to positions).
    pub linked_position_id: Option<String>,
    /// Live PNL (Profit and Loss) to display on the line.
    pub pnl: Option<f64>,
    /// Whether to show the SL button.
    pub show_sl_button: bool,
    /// Whether to show the TP button.
    pub show_tp_button: bool,
}

impl Default for OrderLineOptions {
    fn default() -> Self {
        Self {
            price: 0.0,
            trigger_price: None,
            order_type: OrderType::Limit,
            side: OrderSide::Buy,
            status: OrderStatus::Pending,
            quantity: 0.0,
            filled_quantity: 0.0,
            color: None,
            line_width: 1.0,
            line_style: LineStyle::Solid,
            visible: true,
            cancellable: true,
            modifiable: true,
            extended_label: true,
            custom_label: None,
            tooltip: None,
            label_text_color: colors::LABEL_TEXT_COLOR,
            body_bg_color: None,
            show_quantity: true,
            show_order_type: true,
            linked_position_id: None,
            pnl: None,
            show_sl_button: false,
            show_tp_button: false,
        }
    }
}

impl OrderLineOptions {
    pub fn is_position_line(&self) -> bool {
        self.linked_position_id.is_none() && matches!(self.order_type, OrderType::Market)
    }

    pub fn quantity_label(&self) -> Option<String> {
        if self.quantity <= 0.0 {
            return None;
        }

        Some(if self.quantity == self.quantity.floor() {
            format!("{:.0}", self.quantity)
        } else {
            format!("{:.4}", self.quantity)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        })
    }

    pub fn pnl_label(&self) -> Option<String> {
        self.pnl.map(|pnl_val| format!("{:.2}", pnl_val.abs()))
    }

    pub fn pnl_is_profit(&self) -> Option<bool> {
        self.pnl.map(|pnl_val| pnl_val >= 0.0)
    }

    pub fn supports_bracket_buttons(&self) -> bool {
        self.linked_position_id.is_none()
            && !matches!(self.order_type, OrderType::TakeProfit | OrderType::StopLoss)
    }

    pub fn shows_sl_button(&self) -> bool {
        self.show_sl_button && self.supports_bracket_buttons()
    }

    pub fn shows_tp_button(&self) -> bool {
        self.show_tp_button && self.supports_bracket_buttons()
    }

    /// Get the effective line color based on side and order type.
    pub fn effective_color(&self) -> [f32; 4] {
        if let Some(color) = self.color {
            return color;
        }

        match self.order_type {
            OrderType::TakeProfit => colors::TAKE_PROFIT_COLOR,
            OrderType::StopLoss => colors::STOP_LOSS_COLOR,
            _ => match self.side {
                OrderSide::Buy => colors::BUY_COLOR,
                OrderSide::Sell => colors::SELL_COLOR,
            },
        }
    }

    /// Generate the label text for the order line.
    /// Clean, compact format—price is shown on Y-axis, no need to repeat.
    pub fn generate_label(&self, _price_precision: u32) -> String {
        if let Some(ref custom) = self.custom_label {
            return custom.clone();
        }

        // Compact format: "Sell Limit 1.0" or "Buy Stop 2"
        let mut parts = Vec::new();

        // Side
        parts.push(self.side.as_str().to_string());

        // Order type
        if self.show_order_type {
            parts.push(self.order_type.short_label().to_string());
        }

        // Quantity (compact)
        if self.show_quantity && self.quantity > 0.0 {
            if let Some(qty_str) = self.quantity_label() {
                parts.push(qty_str);
            }
        }

        parts.join(" ")
    }
}

/// A single order line instance.
#[derive(Debug, Clone)]
pub struct OrderLine {
    id: OrderLineId,
    /// Current options (includes mutable price for dragging).
    pub options: OrderLineOptions,
    /// Whether this line is currently being dragged.
    pub dragging: bool,
    /// Whether this line is hovered.
    pub hovered: bool,
    /// Whether the cancel button is hovered.
    pub cancel_hovered: bool,
    /// Whether the TP button is hovered.
    pub tp_hovered: bool,
    /// Whether the SL button is hovered.
    pub sl_hovered: bool,
    /// Original price before drag started (for cancel/revert).
    pub drag_start_price: Option<f64>,
}

impl OrderLine {
    pub fn new(id: OrderLineId, options: OrderLineOptions) -> Self {
        Self {
            id,
            options,
            dragging: false,
            hovered: false,
            cancel_hovered: false,
            tp_hovered: false,
            sl_hovered: false,
            drag_start_price: None,
        }
    }

    #[inline]
    pub fn id(&self) -> &OrderLineId {
        &self.id
    }

    #[inline]
    pub fn id_str(&self) -> &str {
        &self.id.0
    }

    #[inline]
    pub fn price(&self) -> f64 {
        self.options.price
    }

    #[inline]
    pub fn set_price(&mut self, price: f64) {
        self.options.price = price;
    }

    #[inline]
    pub fn is_visible(&self) -> bool {
        self.options.visible
    }

    #[inline]
    pub fn is_modifiable(&self) -> bool {
        self.options.modifiable
            && self.options.status.is_active()
            && !matches!(self.options.order_type, OrderType::Market)
    }

    #[inline]
    pub fn is_cancellable(&self) -> bool {
        self.options.cancellable && self.options.status.is_active()
    }

    /// Start dragging this order line.
    pub fn start_drag(&mut self) {
        if self.is_modifiable() {
            self.dragging = true;
            self.drag_start_price = Some(self.options.price);
        }
    }

    /// Cancel the drag and revert to original price.
    pub fn cancel_drag(&mut self) {
        if let Some(original_price) = self.drag_start_price.take() {
            self.options.price = original_price;
        }
        self.dragging = false;
    }

    /// End dragging and keep the new price.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_start_price = None;
    }
}

/// Hit-test result for order lines.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderLineHit {
    /// No hit.
    None,
    /// Hit the line body (for selection/hover).
    Line(OrderLineId),
    /// Hit the cancel button.
    CancelButton(OrderLineId),
    /// Hit the SL button.
    SlButton(OrderLineId),
    /// Hit the TP button.
    TpButton(OrderLineId),
    /// Hit the label/body area on the price axis.
    Label(OrderLineId),
}

/// Manages all order lines on a chart.
pub struct OrderLineManager {
    lines: Vec<OrderLine>,
    /// Hit threshold in CSS pixels.
    hit_threshold: f64,
    /// Cancel button width in CSS pixels.
    cancel_button_width: f64,
    /// Whether to show cancel buttons.
    show_cancel_buttons: bool,
    /// Default price precision for labels.
    price_precision: u32,
}

impl Default for OrderLineManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderLineManager {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            hit_threshold: 15.0,
            cancel_button_width: 24.0,
            show_cancel_buttons: true,
            price_precision: 2,
        }
    }

    /// Set the default price precision for labels.
    pub fn set_price_precision(&mut self, precision: u32) {
        self.price_precision = precision;
    }

    /// Get the configured price precision for order-line price labels.
    pub fn price_precision(&self) -> u32 {
        self.price_precision
    }

    /// Set whether to show cancel buttons.
    pub fn set_show_cancel_buttons(&mut self, show: bool) {
        self.show_cancel_buttons = show;
    }

    /// Create a new order line. Returns the assigned ID.
    pub fn create(&mut self, id: impl Into<String>, options: OrderLineOptions) -> OrderLineId {
        let order_id = OrderLineId::new(id);
        // Remove existing line with same ID if present
        self.lines.retain(|l| l.id != order_id);
        self.lines.push(OrderLine::new(order_id.clone(), options));
        order_id
    }

    /// Update an existing order line's options.
    pub fn update(&mut self, id: &OrderLineId, options: OrderLineOptions) -> bool {
        if let Some(line) = self.get_mut(id) {
            line.options = options;
            true
        } else {
            false
        }
    }

    /// Update just the price of an order line.
    pub fn update_price(&mut self, id: &OrderLineId, price: f64) -> bool {
        if let Some(line) = self.get_mut(id) {
            line.options.price = price;
            true
        } else {
            false
        }
    }

    /// Update the status of an order line.
    pub fn update_status(&mut self, id: &OrderLineId, status: OrderStatus) -> bool {
        if let Some(line) = self.get_mut(id) {
            line.options.status = status;
            true
        } else {
            false
        }
    }

    /// Update filled quantity for partial fills.
    pub fn update_filled_quantity(&mut self, id: &OrderLineId, filled: f64) -> bool {
        if let Some(line) = self.get_mut(id) {
            line.options.filled_quantity = filled;
            if filled >= line.options.quantity {
                line.options.status = OrderStatus::Filled;
            } else if filled > 0.0 {
                line.options.status = OrderStatus::PartiallyFilled;
            }
            true
        } else {
            false
        }
    }

    /// Remove an order line by ID. Returns true if found and removed.
    pub fn remove(&mut self, id: &OrderLineId) -> bool {
        if let Some(pos) = self.lines.iter().position(|l| &l.id == id) {
            self.lines.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove an order line by string ID.
    pub fn remove_by_str(&mut self, id: &str) -> bool {
        self.remove(&OrderLineId::new(id))
    }

    /// Remove all order lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Remove all order lines with a specific status.
    pub fn remove_by_status(&mut self, status: OrderStatus) {
        self.lines.retain(|l| l.options.status != status);
    }

    /// Get a mutable reference to an order line by ID.
    pub fn get_mut(&mut self, id: &OrderLineId) -> Option<&mut OrderLine> {
        self.lines.iter_mut().find(|l| &l.id == id)
    }

    /// Get a mutable reference to an order line by string ID.
    pub fn get_mut_by_str(&mut self, id: &str) -> Option<&mut OrderLine> {
        let order_id = OrderLineId::new(id);
        self.get_mut(&order_id)
    }

    /// Get an immutable reference to an order line by ID.
    pub fn get(&self, id: &OrderLineId) -> Option<&OrderLine> {
        self.lines.iter().find(|l| &l.id == id)
    }

    /// Get an immutable reference by string ID.
    pub fn get_by_str(&self, id: &str) -> Option<&OrderLine> {
        let order_id = OrderLineId::new(id);
        self.get(&order_id)
    }

    /// Iterate over all order lines.
    pub fn iter(&self) -> impl Iterator<Item = &OrderLine> {
        self.lines.iter()
    }

    /// Iterate over all active (pending/working) order lines.
    pub fn iter_active(&self) -> impl Iterator<Item = &OrderLine> {
        self.lines.iter().filter(|l| l.options.status.is_active())
    }

    /// Number of order lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Number of active order lines.
    pub fn active_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.options.status.is_active())
            .count()
    }

    fn has_active_linked_order_of_type(
        &self,
        parent_id: &OrderLineId,
        order_type: OrderType,
    ) -> bool {
        self.lines.iter().any(|line| {
            line.options.status.is_active()
                && line.options.order_type == order_type
                && line.options.linked_position_id.as_deref() == Some(parent_id.0.as_str())
        })
    }

    pub fn line_shows_sl_button(&self, line: &OrderLine) -> bool {
        line.options.shows_sl_button()
            && !self.has_active_linked_order_of_type(line.id(), OrderType::StopLoss)
    }

    pub fn line_shows_tp_button(&self, line: &OrderLine) -> bool {
        line.options.shows_tp_button()
            && !self.has_active_linked_order_of_type(line.id(), OrderType::TakeProfit)
    }

    pub fn estimate_label_width_css(&self, line: &OrderLine) -> f64 {
        let opts = &line.options;
        if opts.is_position_line() {
            let qty_len = opts.quantity_label().map(|s| s.len()).unwrap_or(1) as f64;
            let pnl_len = opts.pnl_label().map(|s| s.len()).unwrap_or(5) as f64;
            return 16.0
                + (qty_len.max(2.0) * 7.0)
                + 10.0
                + (pnl_len.max(5.0) * 7.0)
                + 10.0
                + self.cancel_button_width;
        }

        let label = opts.generate_label(self.price_precision);
        8.0 + (label.len() as f64 * 7.0)
    }

    /// Hit-test a point against all order lines.
    pub fn hit_test(
        &self,
        x_css: f64,
        y_css: f64,
        viewport: &crate::core::viewport::Viewport,
        pane_css_h: f64,
        pane_css_w: f64,
        font_size_css: f64,
    ) -> OrderLineHit {
        let axis_inset_tb_css = 2.5 / 12.0 * font_size_css;
        let mut pill_h = (font_size_css + axis_inset_tb_css * 2.0).round();
        if pill_h < 1.0 {
            pill_h = 1.0;
        }
        let px = 10.0;
        let right_margin = 8.0;
        let sec_btn_w = 28.0;
        let btn_gap = 6.0;
        // Text width constants were tuned for 11px. Scale to current order-line
        // font size so button/body hitboxes stay aligned with rendered pills.
        let text_scale = (font_size_css / 11.0).max(0.5);

        for line in &self.lines {
            if !line.is_visible() {
                continue;
            }

            let opts = &line.options;
            let y_phys = viewport.price_to_css_y(opts.price, pane_css_h);
            let dist = (y_css - y_phys).abs();

            if dist <= self.hit_threshold || dist <= pill_h / 2.0 {
                let show_tp_button = self.line_shows_tp_button(line) && opts.status.is_active();
                let show_sl_button = self.line_shows_sl_button(line) && opts.status.is_active();
                let has_close = opts.cancellable && opts.status.is_active();

                let mut main_text = String::new();
                if opts.is_position_line() {
                    if let Some(qty) = opts.quantity_label() {
                        main_text.push_str(&qty);
                    }
                    if let Some(pnl) = opts.pnl_label() {
                        if !main_text.is_empty() {
                            main_text.push_str("   ");
                        }
                        main_text.push_str(&pnl);
                    }
                } else {
                    main_text.push_str(&opts.generate_label(self.price_precision));
                }

                // Precise text width estimation to align hitboxes exactly with render
                let mut text_w = 0.0;
                for c in main_text.chars() {
                    match c {
                        'A'..='Z' => text_w += 7.5 * text_scale,
                        '0'..='9' => text_w += 6.5 * text_scale,
                        ' ' => text_w += 3.0 * text_scale,
                        '.' | ',' | ':' => text_w += 3.0 * text_scale,
                        '+' | '-' => text_w += 6.5 * text_scale,
                        _ => text_w += 6.0 * text_scale,
                    }
                }
                text_w += 2.0 * text_scale; // slight baseline buffer

                let close_btn_size = if has_close { 10.0 } else { 0.0 };
                let close_margin = if has_close { 8.0 } else { 0.0 };
                let main_pill_w = px + text_w + close_margin + close_btn_size + px;

                let right_edge = pane_css_w - right_margin;
                let main_pill_x = right_edge - main_pill_w;

                // 1. Check Cancel Button
                if has_close {
                    let cancel_hit_area_w = close_btn_size + close_margin + px + 6.0; // generous grab area
                    if x_css >= right_edge - cancel_hit_area_w && x_css <= right_edge {
                        return OrderLineHit::CancelButton(line.id.clone());
                    }
                }

                // 2. Check SL/TP Buttons
                let mut btn_cursor = main_pill_x - btn_gap;
                let hit_pad = 4.0; // generous padding so hover is extremely responsive

                if show_sl_button {
                    let sl_x_start = btn_cursor - sec_btn_w;
                    if x_css >= sl_x_start - hit_pad && x_css <= btn_cursor + hit_pad {
                        return OrderLineHit::SlButton(line.id.clone());
                    }
                    btn_cursor = sl_x_start - btn_gap;
                }

                if show_tp_button {
                    let tp_x_start = btn_cursor - sec_btn_w;
                    if x_css >= tp_x_start - hit_pad && x_css <= btn_cursor + hit_pad {
                        return OrderLineHit::TpButton(line.id.clone());
                    }
                }

                let approx_label_width = main_pill_w
                    + if show_sl_button {
                        sec_btn_w + btn_gap
                    } else {
                        0.0
                    }
                    + if show_tp_button {
                        sec_btn_w + btn_gap
                    } else {
                        0.0
                    };
                let label_x_start = right_edge - approx_label_width;

                if x_css >= label_x_start && x_css <= right_edge {
                    return OrderLineHit::Line(line.id.clone());
                }

                // 3. Line/Body Hit
                if x_css <= right_edge {
                    return OrderLineHit::Line(line.id.clone());
                }
            } // end if (dist <= ...)
        } // end for

        OrderLineHit::None
    }

    /// Clear hover state for all lines.
    pub fn clear_hover(&mut self) {
        for line in &mut self.lines {
            line.hovered = false;
            line.cancel_hovered = false;
            line.tp_hovered = false;
            line.sl_hovered = false;
        }
    }

    /// Set hover state for a specific line.
    pub fn set_hover(&mut self, id: &OrderLineId, hovered: bool) {
        if let Some(line) = self.get_mut(id) {
            line.hovered = hovered;
        }
    }

    /// Set cancel button hover state.
    pub fn set_cancel_hover(&mut self, id: &OrderLineId, hovered: bool) {
        if let Some(line) = self.get_mut(id) {
            line.cancel_hovered = hovered;
        }
    }

    /// Set TP button hover state.
    pub fn set_tp_hover(&mut self, id: &OrderLineId, hovered: bool) {
        if let Some(line) = self.get_mut(id) {
            line.tp_hovered = hovered;
        }
    }

    /// Set SL button hover state.
    pub fn set_sl_hover(&mut self, id: &OrderLineId, hovered: bool) {
        if let Some(line) = self.get_mut(id) {
            line.sl_hovered = hovered;
        }
    }

    /// Start dragging an order line.
    pub fn start_drag(&mut self, id: &OrderLineId) -> bool {
        if let Some(line) = self.get_mut(id) {
            if line.is_modifiable() {
                line.start_drag();
                return true;
            }
        }
        false
    }

    /// Update price during drag.
    pub fn drag_to(
        &mut self,
        id: &OrderLineId,
        y_css: f64,
        viewport: &crate::core::viewport::Viewport,
        pane_css_h: f64,
    ) {
        if let Some(line) = self.get_mut(id) {
            if line.dragging {
                // Convert Y coordinate to price
                let candle_h = pane_css_h * viewport.candle_height_frac();
                let frac = 1.0 - (y_css / candle_h).clamp(0.0, 1.0);
                let internal =
                    viewport.price_min + frac * (viewport.price_max - viewport.price_min);
                let price = viewport.internal_to_price(internal);
                line.set_price(price);
            }
        }
    }

    /// End dragging and confirm the new price.
    pub fn end_drag(&mut self, id: &OrderLineId) -> Option<f64> {
        if let Some(line) = self.get_mut(id) {
            let new_price = line.price();
            line.end_drag();
            return Some(new_price);
        }
        None
    }

    /// Cancel dragging and revert to original price.
    pub fn cancel_drag(&mut self, id: &OrderLineId) {
        if let Some(line) = self.get_mut(id) {
            line.cancel_drag();
        }
    }

    /// End all drags.
    pub fn end_all_drags(&mut self) {
        for line in &mut self.lines {
            line.end_drag();
        }
    }

    /// Find order line currently being dragged.
    pub fn get_dragging(&self) -> Option<&OrderLine> {
        self.lines.iter().find(|l| l.dragging)
    }

    /// Get the ID of the order line currently being dragged.
    pub fn get_dragging_id(&self) -> Option<OrderLineId> {
        self.get_dragging().map(|l| l.id.clone())
    }

    /// Serialize all order lines to JSON.
    pub fn to_json(&self) -> String {
        let marks: Vec<serde_json::Value> = self
            .lines
            .iter()
            .map(|line| {
                let mut obj = serde_json::json!({
                    "id": line.id.0,
                    "price": line.options.price,
                    "order_type": line.options.order_type.as_str(),
                    "side": line.options.side.as_str(),
                    "status": line.options.status.as_str(),
                    "quantity": line.options.quantity,
                    "filled_quantity": line.options.filled_quantity,
                    "visible": line.options.visible,
                    "cancellable": line.options.cancellable,
                    "modifiable": line.options.modifiable,
                });

                if let Some(trigger) = line.options.trigger_price {
                    obj["trigger_price"] = serde_json::json!(trigger);
                }
                if let Some(ref color) = line.options.color {
                    obj["color"] = serde_json::json!(color);
                }
                if let Some(ref label) = line.options.custom_label {
                    obj["custom_label"] = serde_json::json!(label);
                }
                if let Some(ref tooltip) = line.options.tooltip {
                    obj["tooltip"] = serde_json::json!(tooltip);
                }
                if let Some(ref pos_id) = line.options.linked_position_id {
                    obj["linked_position_id"] = serde_json::json!(pos_id);
                }
                if let Some(pnl_val) = line.options.pnl {
                    obj["pnl"] = serde_json::json!(pnl_val);
                }
                obj["show_sl_button"] = serde_json::json!(line.options.show_sl_button);
                obj["show_tp_button"] = serde_json::json!(line.options.show_tp_button);

                obj
            })
            .collect();

        serde_json::json!({
            "version": 1,
            "orders": marks
        })
        .to_string()
    }

    /// Deserialize order lines from JSON.
    pub fn from_json(&mut self, json: &str) -> Result<(), String> {
        let data: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {}", e))?;

        let orders = data
            .get("orders")
            .and_then(|v| v.as_array())
            .ok_or("Missing 'orders' array")?;

        self.lines.clear();

        for order in orders {
            let id = order
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' field")?;

            let price = order
                .get("price")
                .and_then(|v| v.as_f64())
                .ok_or("Missing 'price' field")?;

            let order_type = order
                .get("order_type")
                .and_then(|v| v.as_str())
                .map(OrderType::from_str)
                .unwrap_or_default();

            let side = order
                .get("side")
                .and_then(|v| v.as_str())
                .map(OrderSide::from_str)
                .unwrap_or_default();

            let status = order
                .get("status")
                .and_then(|v| v.as_str())
                .map(OrderStatus::from_str)
                .unwrap_or_default();

            let quantity = order
                .get("quantity")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let filled_quantity = order
                .get("filled_quantity")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let trigger_price = order.get("trigger_price").and_then(|v| v.as_f64());

            let color = order.get("color").and_then(|v| {
                v.as_array().map(|arr| {
                    let mut c = [0.0f32; 4];
                    for (i, val) in arr.iter().enumerate().take(4) {
                        c[i] = val.as_f64().unwrap_or(0.0) as f32;
                    }
                    c
                })
            });

            let custom_label = order
                .get("custom_label")
                .and_then(|v| v.as_str())
                .map(String::from);

            let tooltip = order
                .get("tooltip")
                .and_then(|v| v.as_str())
                .map(String::from);

            let linked_position_id = order
                .get("linked_position_id")
                .and_then(|v| v.as_str())
                .map(String::from);

            let pnl = order.get("pnl").and_then(|v| v.as_f64());

            let show_sl_button = order
                .get("show_sl_button")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let show_tp_button = order
                .get("show_tp_button")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let visible = order
                .get("visible")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let cancellable = order
                .get("cancellable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let modifiable = order
                .get("modifiable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let options = OrderLineOptions {
                price,
                trigger_price,
                order_type,
                side,
                status,
                quantity,
                filled_quantity,
                color,
                visible,
                cancellable,
                modifiable,
                custom_label,
                tooltip,
                linked_position_id,
                pnl,
                show_sl_button,
                show_tp_button,
                ..Default::default()
            };

            self.lines
                .push(OrderLine::new(OrderLineId::new(id), options));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_type_parsing() {
        assert_eq!(OrderType::from_str("limit"), OrderType::Limit);
        assert_eq!(OrderType::from_str("STOP_LOSS"), OrderType::StopLoss);
        assert_eq!(OrderType::from_str("tp"), OrderType::TakeProfit);
        assert_eq!(OrderType::from_str("trailing"), OrderType::TrailingStop);
    }

    #[test]
    fn test_order_side_parsing() {
        assert_eq!(OrderSide::from_str("buy"), OrderSide::Buy);
        assert_eq!(OrderSide::from_str("sell"), OrderSide::Sell);
        assert_eq!(OrderSide::from_str("short"), OrderSide::Sell);
    }

    #[test]
    fn test_order_status_is_active() {
        assert!(OrderStatus::Pending.is_active());
        assert!(OrderStatus::Working.is_active());
        assert!(OrderStatus::PartiallyFilled.is_active());
        assert!(!OrderStatus::Filled.is_active());
        assert!(!OrderStatus::Cancelled.is_active());
    }

    #[test]
    fn test_label_generation() {
        let options = OrderLineOptions {
            price: 50000.0,
            order_type: OrderType::Limit,
            side: OrderSide::Buy,
            quantity: 0.5,
            ..Default::default()
        };
        let label = options.generate_label(2);
        assert!(label.contains("Buy"));
        assert!(label.contains("LMT"));
        assert!(label.contains("0.5"));
    }

    #[test]
    fn test_manager_crud() {
        let mut manager = OrderLineManager::new();

        let id = manager.create(
            "order-1",
            OrderLineOptions {
                price: 50000.0,
                ..Default::default()
            },
        );

        assert_eq!(manager.len(), 1);
        assert!(manager.get(&id).is_some());

        manager.update_price(&id, 51000.0);
        assert_eq!(manager.get(&id).unwrap().price(), 51000.0);

        manager.remove(&id);
        assert_eq!(manager.len(), 0);
    }
}

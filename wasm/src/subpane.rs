//! SubPane — indicator sub-pane below the main chart.
//!
//! Architecture matches the main chart widget structure:
//!   - Chart area: base canvas (data/grid) + top canvas (crosshair overlay)
//!   - Price axis: base + top canvases via PriceAxisRenderer (same widget as main)
//!   - CSS Grid integration with draggable separator
//!
//! Each sub-pane shares the time axis with the main chart and has its own
//! independent price scale (Viewport).
//!
//! ## Height Coordination
//!
//! Uses `PaneHeightCoordinator` to bridge with aion_charts's `PaneManager` for:
//!   - Stretch-factor based proportional height allocation
//!   - Coordinated separator dragging (resizing one pane affects neighbors)
//!   - Minimum height enforcement
//!   - Time axis always visible at bottom
#![allow(dead_code)]

use crate::RenderInvalidation;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlDivElement, MouseEvent};

use aion_charts::core::drawings::types::DrawingGeometry;
use aion_charts::core::renderer::canvas_dash::{clear_canvas_line_dash, set_canvas_line_dash};
use aion_charts::core::renderer::geometry_generator;
use aion_charts::core::renderer::tick_marks::compute_y_ticks;
use aion_charts::core::renderer::traits::{ChartStyle, CrosshairMode, CrosshairState, TickMark};
use aion_charts::core::renderer::value_projection::TimeScaleIndex;
use aion_charts::core::series::LineDataArray;
use aion_charts::core::viewport::Viewport;
use aion_charts::{
    DrawingManager, PaneId, PaneManager, PaneOptions, PriceAxisRenderer, ScrollState,
};
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

const MIN_PANE_HEIGHT: f64 = 30.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ExactPixelSizes {
    chart_pw: u32,
    chart_ph: u32,
    axis_pw: u32,
    axis_ph: u32,
}

#[derive(Debug, Clone)]
pub struct SubPaneSeparatorStyle {
    pub line_thickness_css: f64,
    pub hit_area_css: f64,
    pub color: [f32; 4],
    pub hover_color: [f32; 4],
}

impl SubPaneSeparatorStyle {
    pub fn from_chart_style(style: &ChartStyle) -> Self {
        let mut s = Self {
            line_thickness_css: 1.0,
            hit_area_css: 9.0,
            color: style.axis_border_color,
            hover_color: style.crosshair_vert_line.color,
        };
        s.normalize();
        s
    }

    pub fn normalize(&mut self) {
        self.line_thickness_css = self.line_thickness_css.clamp(1.0, 16.0);
        self.hit_area_css = self.hit_area_css.clamp(self.line_thickness_css, 32.0);
    }
}

// ── PaneHeightCoordinator ──────────────────────────────────────────────────
//
// Bridges aion_charts's PaneManager with the SubPane DOM-based system.
// Coordinates heights across all panes using stretch factors.

/// Maps a SubPane ID to its PaneManager PaneId.
#[derive(Debug, Clone, Copy)]
pub struct PaneMapping {
    pub subpane_id: u32,
    pub pane_id: PaneId,
}

/// Coordinates pane heights using aion_charts's PaneManager.
///
/// This wrapper:
/// - Maintains a PaneManager instance for stretch-factor-based heights
/// - Maps SubPane IDs to PaneManager PaneIds
/// - Distributes computed heights to SubPane shared_height cells
pub struct PaneHeightCoordinator {
    manager: PaneManager,
    mappings: Vec<PaneMapping>,
    main_pane_height: Rc<Cell<f64>>,
}

impl PaneHeightCoordinator {
    /// Create a new coordinator with given total height.
    pub fn new(main_pane_height_css: f64) -> Self {
        let mut manager = PaneManager::new();
        // Initialize main pane (id=0) with stretch factor 3.0 (dominant)
        manager.init_main(100, 100); // Physical size doesn't matter for height calc

        let main_pane_height = Rc::new(Cell::new(main_pane_height_css));

        Self {
            manager,
            mappings: Vec::new(),
            main_pane_height,
        }
    }

    /// Get the main pane height cell (for syncing with render loop).
    pub fn main_pane_height(&self) -> Rc<Cell<f64>> {
        self.main_pane_height.clone()
    }

    /// Set the total available height and recompute all pane heights.
    pub fn set_total_height(&mut self, total_height: f64) {
        self.manager.set_total_height(total_height);
        self.sync_heights();
    }

    /// Register a SubPane and return its shared height cell.
    /// The subpane will be added to PaneManager with indicator stretch factor.
    pub fn register_subpane(&mut self, subpane_id: u32) -> Rc<Cell<f64>> {
        // Create indicator pane options (stretch factor 1.0)
        let options = PaneOptions::indicator();
        let pane_id = self.manager.add_pane(options, 100, 100);

        self.mappings.push(PaneMapping {
            subpane_id,
            pane_id,
        });

        // Get the computed height from PaneManager
        let height = self
            .manager
            .get(pane_id)
            .map(|p| p.height_css)
            .unwrap_or(100.0);

        let height_cell = Rc::new(Cell::new(height));

        // Sync all heights after adding
        self.sync_heights();

        height_cell
    }

    /// Unregister a SubPane.
    pub fn unregister_subpane(&mut self, subpane_id: u32) {
        if let Some(pos) = self
            .mappings
            .iter()
            .position(|m| m.subpane_id == subpane_id)
        {
            let mapping = self.mappings.remove(pos);
            self.manager.remove_pane(mapping.pane_id);
            self.sync_heights();
        }
    }

    /// Handle separator drag. Returns updated heights for all subpanes.
    /// `separator_idx` is 0 for the separator between main and first subpane.
    pub fn drag_separator(&mut self, separator_idx: usize, delta_y: f64) {
        self.manager.drag_separator(separator_idx, delta_y);
        self.sync_heights();
    }

    /// Get the separator index for a subpane (its position in the list).
    pub fn separator_index(&self, subpane_id: u32) -> Option<usize> {
        self.mappings
            .iter()
            .position(|m| m.subpane_id == subpane_id)
    }

    /// Get computed height for a subpane.
    pub fn get_height(&self, subpane_id: u32) -> f64 {
        self.mappings
            .iter()
            .find(|m| m.subpane_id == subpane_id)
            .and_then(|m| self.manager.get(m.pane_id))
            .map(|p| p.height_css)
            .unwrap_or(100.0)
    }

    /// Get computed heights for all subpanes as (subpane_id, height) pairs.
    pub fn all_heights(&self) -> Vec<(u32, f64)> {
        self.mappings
            .iter()
            .filter_map(|m| {
                self.manager
                    .get(m.pane_id)
                    .map(|p| (m.subpane_id, p.height_css))
            })
            .collect()
    }

    /// Sync PaneManager computed heights to the main pane height cell.
    fn sync_heights(&self) {
        if let Some(main) = self.manager.main() {
            self.main_pane_height.set(main.height_css);
        }
    }

    /// Number of registered subpanes.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }
}

// ── IndicatorConfig ────────────────────────────────────────────────────────

/// Data-driven configuration for an indicator sub-pane.
/// Replaces all hardcoded colors, viewport ranges, and reference levels.
#[derive(Debug, Clone)]
pub struct IndicatorConfig {
    /// Default line colors for each output of the indicator.
    pub colors: Vec<[f32; 4]>,
    /// Initial price range minimum.
    pub price_min: f64,
    /// Initial price range maximum.
    pub price_max: f64,
    /// Whether to auto-scale based on visible data.
    pub auto_scale: bool,
    /// Reference levels rendered as dashed horizontal lines (e.g. 30/70 for RSI).
    pub reference_levels: Vec<f64>,
}

impl IndicatorConfig {
    /// Create a configuration for a known indicator type.
    /// Falls back to sensible defaults for unknown types.
    /// Colors are drawn from the theme's indicator palette.
    pub fn for_type(indicator_type: &str) -> Self {
        let theme = aion_charts::ThemeConfig::default();
        let p = &theme.indicator_palette;
        // Palette indices: 0=Blue, 1=Amber, 2=Purple, 3=Red, 4=Green, 5=Grey
        let blue = theme.indicator_color(0);
        let amber = theme.indicator_color(1);
        let purple = theme.indicator_color(2);
        let red = theme.indicator_color(3);
        let grey = theme.indicator_color(5);

        match indicator_type {
            "rsi" => Self {
                colors: vec![purple],
                price_min: 0.0,
                price_max: 100.0,
                auto_scale: false,
                reference_levels: vec![30.0, 70.0],
            },
            "stochastic" => Self {
                colors: vec![blue, amber],
                price_min: 0.0,
                price_max: 100.0,
                auto_scale: false,
                reference_levels: vec![20.0, 80.0],
            },
            "atr" => Self {
                colors: vec![red],
                price_min: 0.0,
                price_max: 1000.0,
                auto_scale: true,
                reference_levels: vec![],
            },
            "macd" => Self {
                colors: vec![blue, amber, grey],
                price_min: -100.0,
                price_max: 100.0,
                auto_scale: true,
                reference_levels: vec![0.0],
            },
            "bollinger" => Self {
                colors: vec![blue, grey, grey],
                price_min: 0.0,
                price_max: 1000.0,
                auto_scale: true,
                reference_levels: vec![],
            },
            _ => Self {
                colors: vec![p.fallback],
                price_min: 0.0,
                price_max: 1000.0,
                auto_scale: true,
                reference_levels: vec![],
            },
        }
    }
}

// ── SubPane ────────────────────────────────────────────────────────────────

/// A sub-pane for rendering oscillator/indicator data.
///
/// Uses the same dual-canvas + PriceAxisRenderer architecture as the main chart.
pub struct SubPane {
    pub id: u32,
    pub study_id: u32,
    pub indicator_type: String,

    // ── Configuration (replaces all hardcoded values) ──
    pub config: IndicatorConfig,

    // ── DOM containers ──
    pub separator: HtmlDivElement,
    separator_handle: HtmlDivElement,
    pub chart_container: HtmlDivElement,
    pub axis_container: HtmlDivElement,
    drag_overlay: HtmlDivElement,
    pub grid_row: u32,
    separator_bg_css: Rc<RefCell<String>>,
    separator_hover_css: Rc<RefCell<String>>,
    font_family: String,

    // ── Chart canvases (base = data/grid, top = crosshair overlay) ──
    chart_base: HtmlCanvasElement,
    chart_base_ctx: CanvasRenderingContext2d,
    chart_top: HtmlCanvasElement,
    chart_top_ctx: CanvasRenderingContext2d,

    // ── Price axis (same widget as main chart) ──
    pub price_axis: PriceAxisRenderer,

    // ── Shared state (closures <-> render loop) ──
    pub shared_height: Rc<Cell<f64>>,
    pub crosshair_y: Rc<Cell<f64>>,
    pub crosshair_active: Rc<Cell<bool>>,

    // ── State ──
    pub viewport: Viewport,
    pub data: Vec<LineDataArray>,
    pub colors: Vec<[f32; 4]>,
    pub dpr: f64,
    /// Scroll state for kinetic/momentum scrolling.
    pub scroll_state: Rc<RefCell<ScrollState>>,
    /// Last double-tap time for double-tap detection.
    pub last_tap_time: Rc<Cell<f64>>,
    /// Auto-scale mode: when true, price range adjusts to fit visible data.
    pub auto_scale: bool,
    /// Drawing manager for this subpane (independent from main chart).
    pub drawings: DrawingManager,

    // ── Event closures (prevent GC) ──
    _closures: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    pub _interaction_closures: Vec<Closure<dyn FnMut(web_sys::Event)>>,
    pub _wheel_closures: Vec<Closure<dyn FnMut(web_sys::WheelEvent)>>,
    pub _touch_closures: Vec<Closure<dyn FnMut(web_sys::TouchEvent)>>,
    _resize_closure: Option<Closure<dyn Fn(js_sys::Array)>>,
    _resize_observer: Option<web_sys::ResizeObserver>,
    exact_sizes: Rc<Cell<ExactPixelSizes>>,
}

impl SubPane {
    /// Create a new sub-pane. Inserts DOM elements into the grid_wrapper.
    pub fn new(
        doc: &Document,
        grid_wrapper: &HtmlDivElement,
        id: u32,
        study_id: u32,
        indicator_type: &str,
        grid_row: u32,
        initial_height: f64,
        dpr: f64,
        _style: &ChartStyle,
        separator_style: &SubPaneSeparatorStyle,
        separator_drag_cb: Rc<dyn Fn(f64)>,
        dirty: Rc<RenderInvalidation>,
    ) -> Result<Self, JsValue> {
        let config = IndicatorConfig::for_type(indicator_type);
        let pane_row = grid_row + 1;
        let id_str = format!("aion_charts-subpane-{}", id);

        let mut separator_style = separator_style.clone();
        separator_style.normalize();
        let sep_bg = rgba(&separator_style.color);
        let hover_color = rgba(&separator_style.hover_color);
        let separator_bg_css = Rc::new(RefCell::new(sep_bg.clone()));
        let separator_hover_css = Rc::new(RefCell::new(hover_color));
        let line_h = separator_style.line_thickness_css;
        let hit_h = separator_style.hit_area_css;
        let handle_top = -((hit_h - line_h) * 0.5);

        // ── Separator ──────────────────────────────────────────────────
        let separator = doc.create_element("div")?.dyn_into::<HtmlDivElement>()?;
        separator.set_id(&format!("{}-sep", id_str));
        separator.style().set_css_text(&format!(
            "grid-column:1/3;grid-row:{row};\
             height:{line_h:.3}px;background:{bg};\
             position:relative;z-index:10;cursor:ns-resize;",
            row = grid_row,
            line_h = line_h,
            bg = sep_bg,
        ));
        grid_wrapper.append_child(&separator)?;

        let handle = doc.create_element("div")?.dyn_into::<HtmlDivElement>()?;
        handle.style().set_css_text(&format!(
            "position:absolute;top:{top:.3}px;left:0;right:0;height:{height:.3}px;\
             cursor:ns-resize;background:transparent;z-index:51;",
            top = handle_top,
            height = hit_h,
        ));
        separator.append_child(&handle)?;

        let drag_overlay = doc.create_element("div")?.dyn_into::<HtmlDivElement>()?;
        drag_overlay.style().set_css_text(
            "position:fixed;display:none;z-index:49;\
             top:0;left:0;width:100%;height:100%;\
             cursor:ns-resize;",
        );
        doc.body()
            .ok_or_else(|| JsValue::from_str("no body"))?
            .append_child(&drag_overlay)?;

        // ── Chart container (grid col 1) ───────────────────────────────
        let chart_container = doc.create_element("div")?.dyn_into::<HtmlDivElement>()?;
        chart_container.set_id(&format!("{}-chart", id_str));
        chart_container.style().set_css_text(&format!(
            "grid-column:1;grid-row:{row};\
             position:relative;overflow:hidden;\
             min-width:0;min-height:0;\
             cursor:crosshair;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
            row = pane_row,
        ));
        grid_wrapper.append_child(&chart_container)?;

        // Base canvas (data lines, grid) - z-index 0
        let chart_base = create_canvas(doc, &format!("{}-chart-base", id_str), 0)?;
        chart_container.append_child(&chart_base)?;
        let chart_base_ctx = get_2d_ctx(&chart_base)?;

        // Top canvas (crosshair overlay) - z-index 1
        let chart_top = create_canvas(doc, &format!("{}-chart-top", id_str), 1)?;
        chart_container.append_child(&chart_top)?;
        let chart_top_ctx = get_2d_ctx(&chart_top)?;

        // ── Price axis container (grid col 2) ──────────────────────────
        let axis_container = doc.create_element("div")?.dyn_into::<HtmlDivElement>()?;
        axis_container.set_id(&format!("{}-axis", id_str));
        axis_container.style().set_css_text(&format!(
            "grid-column:2;grid-row:{row};\
             position:relative;overflow:hidden;\
             min-width:0;min-height:0;\
             cursor:ns-resize;\
             touch-action:none;\
             -webkit-user-select:none;user-select:none;",
            row = pane_row,
        ));
        grid_wrapper.append_child(&axis_container)?;

        // Price axis: base + top canvases (same as main chart)
        let axis_base = create_canvas(doc, &format!("{}-axis-base", id_str), 0)?;
        axis_container.append_child(&axis_base)?;
        let axis_top = create_canvas(doc, &format!("{}-axis-top", id_str), 1)?;
        axis_container.append_child(&axis_top)?;

        // Create PriceAxisRenderer (same widget as main chart)
        let price_axis =
            PriceAxisRenderer::new(axis_base, axis_top, dpr).map_err(|e| JsValue::from_str(&e))?;

        // ── Viewport -- use config-driven ranges ───────────────────────
        let mut viewport = Viewport::new(100, 100);
        viewport.volume_height_ratio = 0.0;
        viewport.price_min = config.price_min;
        viewport.price_max = config.price_max;

        // ── Shared state ───────────────────────────────────────────────
        let shared_height = Rc::new(Cell::new(initial_height));
        let crosshair_y: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let crosshair_active: Rc<Cell<bool>> = Rc::new(Cell::new(false));

        // ── Separator drag ─────────────────────────────────────────────
        let drag_state = Rc::new(RefCell::new(DragState {
            active: false,
            last_y: 0.0,
        }));
        let mut closures: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();

        // Hover highlight
        {
            let sep = separator.clone();
            let hover_bg = separator_hover_css.clone();
            let c =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    let bg = hover_bg.borrow();
                    let _ = sep.style().set_property("background", bg.as_str());
                }));
            handle.add_event_listener_with_callback("mouseenter", c.as_ref().unchecked_ref())?;
            closures.push(c);
        }
        {
            let sep = separator.clone();
            let ds = drag_state.clone();
            let base_bg = separator_bg_css.clone();
            let c =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    if !ds.borrow().active {
                        let bg = base_bg.borrow();
                        let _ = sep.style().set_property("background", bg.as_str());
                    }
                }));
            handle.add_event_listener_with_callback("mouseleave", c.as_ref().unchecked_ref())?;
            closures.push(c);
        }

        // mousedown -> start drag
        {
            let ds = drag_state.clone();
            let ov = drag_overlay.clone();
            let dirty = Rc::clone(&dirty);
            let c =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let e: MouseEvent = e.unchecked_into();
                    let mut state = ds.borrow_mut();
                    state.active = true;
                    state.last_y = e.page_y() as f64;
                    let _ = ov.style().set_property("display", "block");
                    dirty.set(true);
                    e.prevent_default();
                }));
            handle.add_event_listener_with_callback("mousedown", c.as_ref().unchecked_ref())?;
            closures.push(c);
        }

        // mousemove -> update shared height
        {
            let ds = drag_state.clone();
            let ov = drag_overlay.clone();
            let sep = separator.clone();
            let base_bg = separator_bg_css.clone();
            let separator_drag_cb = Rc::clone(&separator_drag_cb);
            let dirty = Rc::clone(&dirty);
            let c =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |e: web_sys::Event| {
                    let e: MouseEvent = e.unchecked_into();
                    let mut state = ds.borrow_mut();
                    if !state.active {
                        return;
                    }
                    if e.buttons() == 0 {
                        state.active = false;
                        let _ = ov.style().set_property("display", "none");
                        let bg = base_bg.borrow();
                        let _ = sep.style().set_property("background", bg.as_str());
                        dirty.set(true);
                        return;
                    }
                    let next_y = e.page_y() as f64;
                    let delta = next_y - state.last_y;
                    state.last_y = next_y;
                    drop(state);
                    if delta.abs() > f64::EPSILON {
                        separator_drag_cb(delta);
                    }
                    dirty.set(true);
                    e.prevent_default();
                }));
            drag_overlay
                .add_event_listener_with_callback("mousemove", c.as_ref().unchecked_ref())?;
            closures.push(c);
        }

        // mouseup -> end drag
        {
            let ds = drag_state.clone();
            let ov = drag_overlay.clone();
            let sep = separator.clone();
            let base_bg = separator_bg_css.clone();
            let dirty = Rc::clone(&dirty);
            let c =
                Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_e: web_sys::Event| {
                    ds.borrow_mut().active = false;
                    let _ = ov.style().set_property("display", "none");
                    let bg = base_bg.borrow();
                    let _ = sep.style().set_property("background", bg.as_str());
                    dirty.set(true);
                }));
            drag_overlay.add_event_listener_with_callback("mouseup", c.as_ref().unchecked_ref())?;
            drag_overlay
                .add_event_listener_with_callback("pointerup", c.as_ref().unchecked_ref())?;
            drag_overlay
                .add_event_listener_with_callback("pointercancel", c.as_ref().unchecked_ref())?;
            closures.push(c);
        }

        // Use config colors as initial colors
        let colors = config.colors.clone();
        let auto_scale = config.auto_scale;

        // Initialize scroll state for kinetic scrolling
        let scroll_state = Rc::new(RefCell::new(ScrollState::new()));
        let last_tap_time = Rc::new(Cell::new(0.0));
        let exact_sizes = Rc::new(Cell::new(ExactPixelSizes::default()));

        let chart_container_for_ro: web_sys::Element = chart_container.clone().unchecked_into();
        let axis_container_for_ro: web_sys::Element = axis_container.clone().unchecked_into();
        let (resize_closure, resize_observer) = {
            let exact_sizes = Rc::clone(&exact_sizes);
            let chart_ref = chart_container_for_ro.clone();
            let axis_ref = axis_container_for_ro.clone();

            let cb =
                Closure::<dyn Fn(js_sys::Array)>::wrap(Box::new(move |entries: js_sys::Array| {
                    let mut next = exact_sizes.get();
                    for idx in 0..entries.length() {
                        let entry: web_sys::ResizeObserverEntry = entries.get(idx).unchecked_into();
                        let target = entry.target();
                        if target == chart_ref {
                            if let Some((pw, ph)) = extract_device_pixel_content_box_size(&entry) {
                                next.chart_pw = pw;
                                next.chart_ph = ph;
                            }
                        } else if target == axis_ref {
                            if let Some((pw, ph)) = extract_device_pixel_content_box_size(&entry) {
                                next.axis_pw = pw;
                                next.axis_ph = ph;
                            }
                        }
                    }
                    exact_sizes.set(next);
                }));
            let observer = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref())?;

            crate::observe_resize_with_device_pixel_box(&observer, &chart_container_for_ro);
            crate::observe_resize_with_device_pixel_box(&observer, &axis_container_for_ro);

            (Some(cb), Some(observer))
        };

        Ok(Self {
            id,
            study_id,
            indicator_type: indicator_type.to_string(),
            config,
            separator,
            separator_handle: handle,
            chart_container,
            axis_container,
            drag_overlay,
            grid_row,
            separator_bg_css,
            separator_hover_css,
            font_family: _style.font_family.clone(),
            chart_base,
            chart_base_ctx,
            chart_top,
            chart_top_ctx,
            price_axis,
            shared_height,
            crosshair_y,
            crosshair_active,
            viewport,
            data: Vec::new(),
            colors,
            dpr,
            scroll_state,
            last_tap_time,
            auto_scale,
            drawings: DrawingManager::new(),
            _closures: closures,
            _interaction_closures: Vec::new(),
            _wheel_closures: Vec::new(),
            _touch_closures: Vec::new(),
            _resize_closure: resize_closure,
            _resize_observer: resize_observer,
            exact_sizes,
        })
    }

    // ── Accessors ──────────────────────────────────────────────────────

    pub fn get_height(&self) -> f64 {
        self.shared_height.get()
    }

    /// Set the height directly.
    pub fn set_height(&self, height: f64) {
        self.shared_height.set(height.max(MIN_PANE_HEIGHT));
    }

    pub fn apply_separator_style(&self, style: &SubPaneSeparatorStyle) {
        let mut style = style.clone();
        style.normalize();
        let base = rgba(&style.color);
        let hover = rgba(&style.hover_color);
        *self.separator_bg_css.borrow_mut() = base.clone();
        *self.separator_hover_css.borrow_mut() = hover;

        let line_h = style.line_thickness_css;
        let hit_h = style.hit_area_css;
        let handle_top = -((hit_h - line_h) * 0.5);

        let _ = self
            .separator
            .style()
            .set_property("height", &format!("{line_h:.3}px"));
        let _ = self.separator.style().set_property("background", &base);
        let _ = self.separator_handle.style().set_css_text(&format!(
            "position:absolute;top:{top:.3}px;left:0;right:0;height:{height:.3}px;\
             cursor:ns-resize;background:transparent;z-index:51;",
            top = handle_top,
            height = hit_h,
        ));
    }

    pub fn set_font_family(&mut self, font_family: String) {
        self.font_family = font_family;
    }

    /// Get a clone of the shared height cell (for coordinator integration).
    pub fn shared_height_cell(&self) -> Rc<Cell<f64>> {
        self.shared_height.clone()
    }

    /// Get a clone of the scroll state (for event handlers).
    pub fn scroll_state_cell(&self) -> Rc<RefCell<ScrollState>> {
        self.scroll_state.clone()
    }

    /// Get a clone of the last tap time cell (for double-tap detection).
    pub fn last_tap_time_cell(&self) -> Rc<Cell<f64>> {
        self.last_tap_time.clone()
    }

    /// Update kinetic scrolling. Returns the pixel delta if still animating.
    pub fn update_kinetic(&self, now_ms: f64) -> Option<f64> {
        let mut scroll = self.scroll_state.borrow_mut();
        scroll.update_kinetic(now_ms)
    }

    /// Handle double-tap to reset viewport to default.
    /// Returns true if this was a double-tap.
    pub fn check_double_tap(&self, now_ms: f64) -> bool {
        const DOUBLE_TAP_THRESHOLD_MS: f64 = 300.0;

        let last = self.last_tap_time.get();
        self.last_tap_time.set(now_ms);

        if now_ms - last < DOUBLE_TAP_THRESHOLD_MS {
            // This is a double-tap
            true
        } else {
            false
        }
    }

    /// Reset the price viewport to default values from config.
    pub fn reset_price_viewport(&mut self) {
        self.viewport.price_min = self.config.price_min;
        self.viewport.price_max = self.config.price_max;
        // auto_scale will be applied on next render() with visible range
    }

    /// Toggle auto-scale mode and unlock price axis.
    pub fn toggle_auto_scale(&mut self) {
        self.auto_scale = !self.auto_scale;
        // Also unlock price axis when enabling auto-scale (same as main chart)
        if self.auto_scale {
            self.viewport.price_locked = false;
            // auto_scale will be applied on next render() with visible range
        }
    }

    // ── Canvas sizing ──────────────────────────────────────────────────

    pub fn resize(&mut self, dpr: f64) {
        self.dpr = dpr;
        // Read size from CONTAINER (not canvas) - the container is properly sized by CSS grid
        let chart_rect = self.chart_container.get_bounding_client_rect();
        let cw = chart_rect.width();
        let ch = chart_rect.height();
        if cw > 0.0 && ch > 0.0 {
            let exact = self.exact_sizes.get();
            let pw = if exact.chart_pw > 0 {
                exact.chart_pw
            } else {
                (cw * dpr).round() as u32
            };
            let ph = if exact.chart_ph > 0 {
                exact.chart_ph
            } else {
                (ch * dpr).round() as u32
            };
            resize_canvas_with_size(&self.chart_base, pw, ph, cw, ch);
            resize_canvas_with_size(&self.chart_top, pw, ph, cw, ch);
        }
        // Price axis canvases — must set CSS size for correct display
        let axis_rect = self.axis_container.get_bounding_client_rect();
        let aw = axis_rect.width();
        let ah = axis_rect.height();
        if aw > 0.0 && ah > 0.0 {
            let exact = self.exact_sizes.get();
            let apw = if exact.axis_pw > 0 {
                exact.axis_pw
            } else {
                (aw * dpr).round() as u32
            };
            let aph = if exact.axis_ph > 0 {
                exact.axis_ph
            } else {
                (ah * dpr).round() as u32
            };
            self.price_axis
                .resize_with_css(apw.max(1), aph.max(1), dpr, aw, ah);
        }
    }

    // ── Data ───────────────────────────────────────────────────────────

    pub fn set_data(&mut self, data: Vec<LineDataArray>, colors: Vec<[f32; 4]>) {
        self.data = data;
        if !colors.is_empty() {
            self.colors = colors;
        }
        // Note: auto_scale is done in render() with the visible range from main_viewport,
        // not here, so the y-axis tightens to the visible bars like the main chart does.
    }

    /// Auto-scale the price axis to fit visible data only.
    /// `start_bar` and `end_bar` define the visible range from the main viewport.
    pub fn auto_scale_price_visible(&mut self, start_bar: f64, end_bar: f64) {
        let lo_idx = (start_bar.floor() as isize).max(0) as usize;
        let hi_idx = (end_bar.ceil() as isize).max(0) as usize;

        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for line in &self.data {
            let end = hi_idx.min(line.values.len());
            if lo_idx >= end {
                continue;
            }
            for &v in &line.values[lo_idx..end] {
                if v.is_finite() {
                    lo = lo.min(v as f64);
                    hi = hi.max(v as f64);
                }
            }
        }
        if lo < hi {
            let m = (hi - lo) * 0.1;
            self.viewport.price_min = lo - m;
            self.viewport.price_max = hi + m;
        } else if lo == hi && lo.is_finite() {
            // Single value — give some visual padding
            self.viewport.price_min = lo - 1.0;
            self.viewport.price_max = hi + 1.0;
        }
    }

    // ── High-level Rendering (called from lib.rs render loop) ──────────

    /// Measure the maximum tick label width for this subpane's price axis.
    /// Used by the main render loop to ensure the shared price axis column
    /// is wide enough for all panes.
    pub fn measure_axis_label_width(&mut self, style: &ChartStyle) -> f64 {
        let ph = self.chart_base.height() as f64;
        if ph <= 0.0 {
            return 0.0;
        }
        // Subpanes have no volume area, so pane_h == candle_h
        let y_ticks = compute_y_ticks(&self.viewport, ph, ph, self.dpr, style);
        self.price_axis.measure_tick_label_width(style, &y_ticks)
    }

    /// Render chart data + grid + reference lines on the BASE canvas,
    /// then render the price axis base layer.
    /// This is the main render entry point called each frame.
    ///
    /// `x_ticks` are passed from the main chart for consistent vertical grid lines.
    pub fn render(
        &mut self,
        main_viewport: &Viewport,
        time_scale: &TimeScaleIndex,
        style: &ChartStyle,
        x_ticks: &[TickMark],
    ) -> Vec<DrawingGeometry> {
        let dpr = self.dpr;
        let pw = self.chart_base.width() as f64;
        let ph = self.chart_base.height() as f64;
        if pw <= 0.0 || ph <= 0.0 {
            return Vec::new();
        }

        // Auto-scale to visible bar range (same behavior as main chart)
        if self.config.auto_scale {
            self.auto_scale_price_visible(main_viewport.start_bar, main_viewport.end_bar);
        }

        // Subpanes have no volume area, so pane_h == candle_h.
        // Drawings now stay on the overlay bucket by default.
        let y_ticks = compute_y_ticks(&self.viewport, ph, ph, dpr, style);
        let (bottom_drawings, top_drawings) = self.generate_drawing_geometry(main_viewport, style);

        self.render_chart(
            main_viewport,
            time_scale,
            style,
            &y_ticks,
            x_ticks,
            &bottom_drawings,
        );

        self.price_axis.render_base(style, &y_ticks);
        let horizontal_line_labels = self.drawings.horizontal_line_axis_labels();
        self.price_axis.render_horizontal_line_labels(
            &horizontal_line_labels,
            &self.viewport,
            style,
            ph,
        );

        // Render last-value indicator labels on the price axis (colored pills)
        let last_values: Vec<(f64, [f32; 4])> = self
            .data
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                // Find the last finite value in the visible range
                let end_idx = (main_viewport.end_bar.ceil() as usize).min(line.values.len());
                let start_idx = (main_viewport.start_bar.floor() as usize)
                    .max(0)
                    .min(end_idx);
                let last_val = line.values[start_idx..end_idx]
                    .iter()
                    .rev()
                    .find(|v| v.is_finite())
                    .copied()?;
                let color = self.colors.get(i).copied().unwrap_or(
                    aion_charts::ThemeConfig::default()
                        .indicator_palette
                        .fallback,
                );
                Some((last_val as f64, color))
            })
            .collect();
        if !last_values.is_empty() {
            self.price_axis
                .render_indicator_last_values(&last_values, &self.viewport, style, ph);
        }

        top_drawings
    }

    /// Clear the crosshair overlay canvas. Call before drawing crosshair lines.
    pub fn clear_crosshair_overlay(&self) {
        let pw = self.chart_top.width() as f64;
        let ph = self.chart_top.height() as f64;
        if pw > 0.0 && ph > 0.0 {
            self.chart_top_ctx.clear_rect(0.0, 0.0, pw, ph);
        }
    }

    /// Render vertical crosshair line (synced from main chart X position).
    /// Called for every sub-pane when crosshair is active.
    pub fn render_crosshair_vert(&self, x_css: f64, style: &ChartStyle) {
        let dpr = self.dpr;
        let pw = self.chart_top.width() as f64;
        let ph = self.chart_top.height() as f64;
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        if !style.crosshair_vert_line.visible {
            return;
        }

        let line_w = (style.crosshair_vert_line.width * dpr).floor().max(1.0);
        let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };

        // Set crosshair line style
        self.chart_top_ctx
            .set_stroke_style_str(&rgba(&style.crosshair_vert_line.color));
        self.chart_top_ctx.set_line_width(line_w);
        self.chart_top_ctx.set_line_cap("butt");
        set_canvas_line_dash(&self.chart_top_ctx, style.crosshair_vert_line.style, line_w);

        // Vertical crosshair line
        let x = (x_css * dpr).round() + correction;
        if x >= 0.0 && x <= pw {
            let span = line_w + 1.0;
            self.chart_top_ctx.begin_path();
            self.chart_top_ctx.move_to(x, -span);
            self.chart_top_ctx.line_to(x, ph + span);
            self.chart_top_ctx.stroke();
        }

        clear_canvas_line_dash(&self.chart_top_ctx);
    }

    /// Render horizontal crosshair line + price axis label when cursor is in this sub-pane.
    /// Also clears the crosshair overlay when cursor leaves.
    pub fn render_crosshair_horiz(&mut self, style: &ChartStyle) {
        let dpr = self.dpr;
        let pw = self.chart_top.width() as f64;
        let ph = self.chart_top.height() as f64;
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }

        // Only draw horizontal line if mouse is in this sub-pane
        if !self.crosshair_active.get() {
            // Clear price axis top layer when not active
            self.price_axis
                .render_top(&CrosshairState::default(), &self.viewport, style, ph, dpr);
            return;
        }

        let y_css = self.crosshair_y.get();
        if style.crosshair_horz_line.visible {
            let line_w = (style.crosshair_horz_line.width * dpr).floor().max(1.0);
            let correction = if (line_w as i32) % 2 == 1 { 0.5 } else { 0.0 };
            let y = (y_css * dpr).round() + correction;

            // Draw horizontal crosshair on chart top canvas
            self.chart_top_ctx
                .set_stroke_style_str(&rgba(&style.crosshair_horz_line.color));
            self.chart_top_ctx.set_line_width(line_w);
            self.chart_top_ctx.set_line_cap("butt");
            set_canvas_line_dash(&self.chart_top_ctx, style.crosshair_horz_line.style, line_w);

            if y >= 0.0 && y <= ph {
                let span = line_w + 1.0;
                self.chart_top_ctx.begin_path();
                self.chart_top_ctx.move_to(-span, y);
                self.chart_top_ctx.line_to(pw + span, y);
                self.chart_top_ctx.stroke();
            }

            clear_canvas_line_dash(&self.chart_top_ctx);
        }

        // Compute price for the crosshair label
        let css_h = ph / dpr;
        let price = self.viewport.pixel_to_price(y_css, css_h);

        // Render price axis top layer with crosshair label
        let crosshair_state = CrosshairState {
            active: true,
            x: 0.0,
            y: y_css,
            bar_index: None,
            price,
            mode: CrosshairMode::Normal,
        };
        self.price_axis
            .render_top(&crosshair_state, &self.viewport, style, ph, dpr);
    }

    // ── Low-level chart rendering ──────────────────────────────────────

    /// Render chart data on the BASE canvas (grid, reference lines, data lines).
    /// Uses CENTRALIZED generate_grid_rects() for consistent grid rendering.
    fn render_chart(
        &self,
        main_viewport: &Viewport,
        time_scale: &TimeScaleIndex,
        style: &ChartStyle,
        y_ticks: &[TickMark],
        x_ticks: &[TickMark],
        bottom_drawings: &[DrawingGeometry],
    ) {
        let dpr = self.dpr;
        let pw = self.chart_base.width() as f64;
        let ph = self.chart_base.height() as f64;
        if pw <= 0.0 || ph <= 0.0 {
            return;
        }
        let css_w = pw / dpr;
        let css_h = ph / dpr;

        // Background
        self.chart_base_ctx
            .set_fill_style_str(&rgba(&style.bg_color));
        self.chart_base_ctx.fill_rect(0.0, 0.0, pw, ph);

        // Grid lines - using CENTRALIZED function (both horizontal and vertical)
        let grid_rects = geometry_generator::generate_grid_rects(style, y_ticks, x_ticks, pw, ph);
        for rect in &grid_rects {
            let color = format!(
                "rgba({},{},{},{})",
                (rect.r * 255.0) as u8,
                (rect.g * 255.0) as u8,
                (rect.b * 255.0) as u8,
                rect.a
            );
            self.chart_base_ctx.set_fill_style_str(&color);
            self.chart_base_ctx.fill_rect(
                rect.x as f64,
                rect.y as f64,
                rect.w as f64,
                rect.h as f64,
            );
        }

        // Base-bucket drawings remain on the chart canvas when present.
        self.render_drawings_on_ctx(&self.chart_base_ctx, bottom_drawings);

        // Reference lines from config (e.g. RSI 30/70, Stochastic 20/80, MACD 0)
        if !self.config.reference_levels.is_empty() {
            let ref_color = [
                style.grid_color[0],
                style.grid_color[1],
                style.grid_color[2],
                0.6,
            ];
            self.chart_base_ctx.set_stroke_style_str(&rgba(&ref_color));
            self.chart_base_ctx.set_line_width(1.0);
            let _ = self.chart_base_ctx.set_line_dash(&js_sys::Array::of2(
                &JsValue::from(4.0 * dpr),
                &JsValue::from(4.0 * dpr),
            ));
            for &level in &self.config.reference_levels {
                let y = (self.viewport.price_to_css_y(level, css_h) * dpr).round() + 0.5;
                self.chart_base_ctx.begin_path();
                self.chart_base_ctx.move_to(0.0, y);
                self.chart_base_ctx.line_to(pw, y);
                self.chart_base_ctx.stroke();
            }
            let _ = self.chart_base_ctx.set_line_dash(&js_sys::Array::new());
        }

        // Data lines
        for (i, line) in self.data.iter().enumerate() {
            let color = self.colors.get(i).copied().unwrap_or(
                aion_charts::ThemeConfig::default()
                    .indicator_palette
                    .fallback,
            );
            self.draw_data_line(line, &color, main_viewport, time_scale, css_w, css_h, dpr);
        }
    }

    fn draw_data_line(
        &self,
        line: &LineDataArray,
        color: &[f32; 4],
        main_viewport: &Viewport,
        time_scale: &TimeScaleIndex,
        css_w: f64,
        css_h: f64,
        dpr: f64,
    ) {
        if line.values.is_empty() {
            return;
        }
        self.chart_base_ctx.set_stroke_style_str(&rgba(color));
        self.chart_base_ctx.set_line_width(2.0 * dpr);
        self.chart_base_ctx.set_line_join("round");
        self.chart_base_ctx.set_line_cap("round");
        self.chart_base_ctx.begin_path();
        let mut started = false;
        for (i, &value) in line.values.iter().enumerate() {
            if !value.is_finite() {
                started = false;
                continue;
            }
            let Some(logical_slot) = time_scale.logical_index_for_main_bar(i) else {
                started = false;
                continue;
            };
            let x = main_viewport.bar_center_css(logical_slot as usize, css_w) * dpr;
            let y = self.viewport.price_to_css_y(value as f64, css_h) * dpr;
            if !started {
                self.chart_base_ctx.move_to(x, y);
                started = true;
            } else {
                self.chart_base_ctx.line_to(x, y);
            }
        }
        self.chart_base_ctx.stroke();
    }

    /// Generate drawing geometry for this subpane.
    /// Uses main viewport's time range with subpane's price range and returns
    /// `(base, overlay)` buckets.
    pub fn generate_drawing_geometry(
        &self,
        main_viewport: &Viewport,
        style: &ChartStyle,
    ) -> (Vec<DrawingGeometry>, Vec<DrawingGeometry>) {
        let dpr = self.dpr;
        let pw = self.chart_base.width() as f64;
        let ph = self.chart_base.height() as f64;
        let chart_rect = self.chart_container.get_bounding_client_rect();
        let css_w = chart_rect.width();
        let css_h = chart_rect.height();
        let h_pixel_ratio = if css_w > 0.0 { pw / css_w } else { dpr };
        let v_pixel_ratio = if css_h > 0.0 { ph / css_h } else { dpr };

        // Create a hybrid viewport:
        // - Time axis (bar range) from main viewport (shared across all panes)
        // - Price axis (price range) from this subpane's viewport
        // Use subpane's viewport as base and update time range
        let mut hybrid_viewport = Viewport::new(pw as u32, ph as u32);
        hybrid_viewport.start_bar = main_viewport.start_bar;
        hybrid_viewport.end_bar = main_viewport.end_bar;
        hybrid_viewport.price_min = self.viewport.price_min;
        hybrid_viewport.price_max = self.viewport.price_max;
        hybrid_viewport.volume_height_ratio = 0.0; // Subpanes don't have volume

        self.drawings.generate_all_geometry_with_text_font_family(
            &hybrid_viewport,
            css_w,
            css_h,
            dpr,
            h_pixel_ratio,
            v_pixel_ratio,
            style.bg_color,
            &style.font_family,
        )
    }

    /// Render overlay-bucket drawings on the chart top canvas.
    /// Call before crosshair rendering so crosshair remains visible above drawings.
    pub fn render_top_drawings(&self, drawings: &[DrawingGeometry]) {
        self.render_drawings_on_ctx(&self.chart_top_ctx, drawings);
    }

    fn render_drawings_on_ctx(&self, ctx: &CanvasRenderingContext2d, drawings: &[DrawingGeometry]) {
        for geom in drawings {
            self.draw_geometry_on_ctx(ctx, geom);
        }
    }

    /// Draw a single DrawingGeometry to a target canvas.
    fn draw_geometry_on_ctx(&self, ctx: &CanvasRenderingContext2d, geom: &DrawingGeometry) {
        // Filled rects
        for r in &geom.rects {
            if r.w <= 0.0 || r.h <= 0.0 {
                continue;
            }
            ctx.set_fill_style_str(&rgba(&[r.r, r.g, r.b, r.a]));
            ctx.fill_rect(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        }

        // Lines
        for l in geom.lines.iter().filter(|line| line.dash >= 0.0) {
            ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            ctx.set_line_width(l.width as f64);
            if l.dash < 0.0 {
                ctx.set_line_cap("butt");
            } else {
                ctx.set_line_cap("round");
            }
            ctx.set_line_join("round");

            if l.dash > 0.0 && l.gap > 0.0 {
                let _ = ctx.set_line_dash(&js_sys::Array::of2(
                    &JsValue::from(l.dash as f64),
                    &JsValue::from(l.gap as f64),
                ));
            } else {
                let _ = ctx.set_line_dash(&js_sys::Array::new());
            }

            // reference implementation strokeInPixel: add 0.5px offset for odd-width lines
            let correction = if l.dash < 0.0 {
                0.0
            } else if (l.width as i32) % 2 == 1 {
                0.5
            } else {
                0.0
            };

            ctx.begin_path();
            ctx.move_to(l.x0 as f64 + correction, l.y0 as f64 + correction);
            ctx.line_to(l.x1 as f64 + correction, l.y1 as f64 + correction);
            ctx.stroke();
        }
        let _ = ctx.set_line_dash(&js_sys::Array::new());

        // Text labels (in physical pixel coords)
        for t in &geom.texts {
            let font = if t.italic {
                format!(
                    "italic {} {}px {}",
                    t.font_weight, t.font_size, self.font_family
                )
            } else {
                format!("{} {}px {}", t.font_weight, t.font_size, self.font_family)
            };
            ctx.save();
            ctx.set_font(&font);
            ctx.set_fill_style_str(&rgba(&[t.r, t.g, t.b, t.a]));
            ctx.set_text_align(t.align.as_canvas_str());
            ctx.set_text_baseline(t.vertical_align.as_canvas_str());
            if t.rotation_rad.abs() > f32::EPSILON {
                let _ = ctx.translate(t.x as f64, t.y as f64);
                let _ = ctx.rotate(t.rotation_rad as f64);
                let _ = ctx.fill_text(&t.text, 0.0, 0.0);
            } else {
                let _ = ctx.fill_text(&t.text, t.x as f64, t.y as f64);
            }
            ctx.restore();
        }

        for l in geom.lines.iter().filter(|line| line.dash < 0.0) {
            ctx.set_stroke_style_str(&rgba(&[l.r, l.g, l.b, l.a]));
            ctx.set_line_width(l.width as f64);
            ctx.set_line_cap("butt");
            ctx.set_line_join("round");
            let _ = ctx.set_line_dash(&js_sys::Array::new());
            let mut x0 = l.x0 as f64;
            let mut y0 = l.y0 as f64;
            let mut x1 = l.x1 as f64;
            let mut y1 = l.y1 as f64;
            if (x1 - x0).abs() <= f64::EPSILON {
                let x = x0.round() + 0.5;
                x0 = x;
                x1 = x;
            } else if (y1 - y0).abs() <= f64::EPSILON {
                let y = y0.round() + 0.5;
                y0 = y;
                y1 = y;
            }
            ctx.begin_path();
            ctx.move_to(x0, y0);
            ctx.line_to(x1, y1);
            ctx.stroke();
        }

        // Anchor circles
        for a in &geom.anchors {
            // Fill
            ctx.set_fill_style_str(&rgba(&a.fill));
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.fill();
            // Border
            ctx.set_stroke_style_str(&rgba(&a.border));
            ctx.set_line_width(a.border_width);
            ctx.begin_path();
            let _ = ctx.arc(a.cx, a.cy, a.radius, 0.0, std::f64::consts::TAU);
            ctx.stroke();
        }
    }

    /// Build a CrosshairState for this sub-pane's price axis.
    pub fn crosshair_state(&self) -> CrosshairState {
        CrosshairState {
            active: self.crosshair_active.get(),
            x: 0.0,
            y: self.crosshair_y.get(),
            bar_index: None,
            price: 0.0,
            mode: CrosshairMode::Normal,
        }
    }

    /// Remove all DOM elements.
    pub fn remove(&self) {
        let _ = self.separator.remove();
        let _ = self.chart_container.remove();
        let _ = self.axis_container.remove();
        let _ = self.drag_overlay.remove();
    }

    /// Dispose: clear all event closures and remove DOM elements.
    ///
    /// Note: The closures hold Rc references to shared state. Clearing them
    /// allows the DOM to release its references to the JS functions,
    /// preventing memory leaks when the subpane is destroyed.
    pub fn dispose(&mut self) {
        if let Some(observer) = &self._resize_observer {
            observer.disconnect();
        }
        self._resize_observer = None;
        self._resize_closure = None;

        // Clear closure vectors - this drops the closures
        // DOM listeners are not explicitly removed here since we're removing
        // the DOM elements entirely, which effectively removes all listeners
        self._closures.clear();
        self._interaction_closures.clear();
        self._wheel_closures.clear();
        self._touch_closures.clear();

        // Remove DOM elements
        self.remove();
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

struct DragState {
    active: bool,
    last_y: f64,
}

fn create_canvas(doc: &Document, id: &str, z_index: u32) -> Result<HtmlCanvasElement, JsValue> {
    crate::utils::create_canvas(doc, id, z_index)
}

fn get_2d_ctx(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, JsValue> {
    Ok(canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d ctx"))?
        .dyn_into::<CanvasRenderingContext2d>()?)
}

fn extract_device_pixel_content_box_size(
    entry: &web_sys::ResizeObserverEntry,
) -> Option<(u32, u32)> {
    let raw = js_sys::Reflect::get(entry, &JsValue::from_str("devicePixelContentBoxSize")).ok()?;
    if raw.is_undefined() || raw.is_null() {
        return None;
    }
    let arr: &js_sys::Array = raw.unchecked_ref();
    if arr.length() == 0 {
        return None;
    }
    let item = arr.get(0);
    let inline_size = js_sys::Reflect::get(&item, &JsValue::from_str("inlineSize"))
        .ok()
        .and_then(|value| value.as_f64())?;
    let block_size = js_sys::Reflect::get(&item, &JsValue::from_str("blockSize"))
        .ok()
        .and_then(|value| value.as_f64())?;
    if inline_size <= 0.0 || block_size <= 0.0 {
        return None;
    }
    Some((inline_size as u32, block_size as u32))
}

fn resize_canvas(canvas: &HtmlCanvasElement, dpr: f64) {
    let w = canvas.client_width() as f64;
    let h = canvas.client_height() as f64;
    if w > 0.0 && h > 0.0 {
        let pw = (w * dpr).round() as u32;
        let ph = (h * dpr).round() as u32;
        crate::utils::set_canvas_size_with_css(canvas, pw.max(1), ph.max(1), w, h);
    }
}

/// Resize canvas with explicit dimensions (for when size is read from container, not canvas).
fn resize_canvas_with_size(canvas: &HtmlCanvasElement, pw: u32, ph: u32, css_w: f64, css_h: f64) {
    crate::utils::set_canvas_size_with_css(canvas, pw.max(1), ph.max(1), css_w, css_h);
}

fn rgba(c: &[f32; 4]) -> String {
    crate::utils::rgba_css(c)
}

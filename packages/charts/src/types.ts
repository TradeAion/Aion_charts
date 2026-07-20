/**
 * Public data, option, and handle types for `@aion/charts` (snake_case; semantics mirror
 * lightweight-charts v5). Extracted from `index.ts`.
 */

// ---------------------------------------------------------------------------------------------
// Data & option types
// ---------------------------------------------------------------------------------------------

/** A series kind. Maps to the engine's numeric kind at the boundary. */
export type series_kind = "candlestick" | "bar" | "line" | "area" | "histogram" | "baseline";

/** OHLC bar for candlestick/bar series. `time` is UTC seconds. */
export interface ohlc_data {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
}

/** Single-value point for line/area/histogram series. `time` is UTC seconds. */
export interface single_value_data {
  time: number;
  value: number;
}

export type series_data = ohlc_data | single_value_data;

/** Inclusive logical (bar-index) range. */
export interface logical_range {
  from: number;
  to: number;
}

/** Inclusive time range (UTC seconds). */
export interface time_range {
  from: number;
  to: number;
}

export interface bars_info extends Partial<time_range> {
  bars_before: number;
  bars_after: number;
}

/** LWC mismatch-direction values used by `data_by_index`. */
export type mismatch_direction = -1 | 0 | 1;
export type data_changed_scope = "full" | "update";
export type data_changed_handler = (scope: data_changed_scope) => void;

/** Parameters delivered to crosshair-move and click subscribers (mirrors LWC `MouseEventParams`). */
export interface mouse_event_params {
  /** UTC seconds of the bar under the cursor, or `null` off the data. */
  time: number | null;
  /** Float logical (bar) index under the cursor, or `null` when there is no data. */
  logical: number | null;
  /** Cursor position in CSS px relative to the pane, or `null` when the cursor left the chart. */
  point: { x: number; y: number } | null;
  /** Per-series value at the hovered bar, keyed by the series handle. */
  series_data: Map<series_api, ohlc_data | single_value_data>;
}

export type mouse_event_handler = (params: mouse_event_params) => void;
export type dbl_click_handler = (params: mouse_event_params) => void;
export type visible_logical_range_handler = (range: logical_range | null) => void;
export type visible_time_range_handler = (range: time_range | null) => void;

export interface time_scale_options {
  /** Distance between adjacent bars in CSS pixels. */
  bar_spacing: number;
  /** Empty logical bars between the final data point and the right edge. */
  right_offset: number;
}

export interface price_scale_options {
  /** 0 normal, 1 logarithmic, 2 percentage, 3 indexed-to-100 (LWC values). */
  mode: 0 | 1 | 2 | 3;
  auto_scale: boolean;
  invert_scale: boolean;
  scale_margins: { top: number; bottom: number };
}

/** The visible raw-value range of a price scale. */
export interface price_range {
  from: number;
  to: number;
}

/** Deeply-partial chart options; forwarded to the engine and deep-merged there (LWC semantics). */
export type deep_partial<T> = { [K in keyof T]?: deep_partial<T[K]> };

export interface grid_line_options {
  color: string;
  style: number;
  visible: boolean;
}
export interface chart_options {
  layout: { background: { type: string; color: string }; textColor: string; fontSize: number; fontFamily: string; attributionLogo: boolean };
  grid: { vertLines: grid_line_options; horzLines: grid_line_options };
  crosshair: { vertLine: Partial<grid_line_options>; horzLine: Partial<grid_line_options>; mode: number };
  leftPriceScale: { visible: boolean; borderVisible: boolean; borderColor: string };
  rightPriceScale: { visible: boolean; borderVisible: boolean; borderColor: string };
  /** Time-axis strip cosmetics (LWC `timeScale.borderVisible`/`borderColor`). */
  timeScale: { borderVisible: boolean; borderColor: string };
  /** Install a ResizeObserver so the chart tracks its container's size. Default `false` (LWC parity). */
  autoSize: boolean;
  hoveredSeriesOnTop: boolean;
  /** Backend override for capability testing; defaults to automatic WebGPU → Canvas2D fallback. */
  backend: "auto" | "canvas2d";
  /**
   * Default style preset from `theme.ts` (the package's style settings file), applied at
   * creation *under* any explicit options. Default `"light"`. Package-level only — never
   * forwarded to the engine.
   */
  theme: "light" | "dark";
}

/** Options accepted when adding a series. */
export interface series_options {
  /** Overrides the kind default color (line/area/histogram). */
  color: string;
  /** Candlestick/bar up (close ≥ open) body color. Any CSS color the engine parses. */
  up_color: string;
  /** Candlestick/bar down (close < open) body color. */
  down_color: string;
  /** Candlestick up-bar wick color. Until set, follows `up_color` (LWC parity). */
  wick_up_color: string;
  /** Candlestick down-bar wick color. Until set, follows `down_color`. */
  wick_down_color: string;
  /** Candlestick up-bar border color. Until set, follows `up_color` (LWC parity). */
  border_up_color: string;
  /** Candlestick down-bar border color. Until set, follows `down_color`. */
  border_down_color: string;
  /** Candlestick wick visibility (default true; ignored by bar series). */
  wick_visible: boolean;
  /** Candlestick body-border visibility (default true; ignored by bar series). */
  border_visible: boolean;
  /** Line/area stroke width in css px (default 3). */
  line_width: number;
  /** Area fill color at the line (top of the gradient). */
  area_top_color: string;
  /** Area fill color at the base (bottom of the gradient; usually fully transparent). */
  area_bottom_color: string;
  /**
   * Histogram only: color each bar by the main price series' up/down direction at that time
   * (translucent green/red), matching TradingView-style volume. Default false (solid `color`).
   */
  histogram_updown: boolean;
  /**
   * Place the series on the bottom-band overlay price scale (volume-style): its magnitude is
   * excluded from the main price axis autoscale. Mirrors LWC's `priceScaleId: ''` + scaleMargins.
   */
  overlay: boolean;
  /** LWC price-scale id: visible left/right pane axis, or empty string for an overlay scale. */
  price_scale_id: "left" | "right" | "";
  /** Camel-case LWC alias of `price_scale_id`. */
  priceScaleId: "left" | "right" | "";
  /** Overlay band as fractions of pane height (default `{ top: 0.8, bottom: 0 }` ⇒ bottom fifth). */
  scale_margins: { top: number; bottom: number };
  /** Stacked pane index (0 = top/price pane). A new pane is created on demand (roadmap Phase B1). */
  pane: number;
  /** Relative height of a newly-created pane (default 1; the price pane is 3). */
  pane_stretch: number;
  /** Line/area join type (roadmap Phase B3). */
  line_type: "simple" | "stepped" | "curved";
  /** Draw a disc at each data point (shown when bars are spaced enough), roadmap Phase B3. */
  point_markers: boolean;
  /** Baseline price for a baseline series (omit for auto = visible-range midpoint). */
  baseline_value: number;
  /** Pulse an expanding ring at the last value (drives an rAF loop), roadmap Phase B3. */
  last_price_animation: boolean;
  /** Keep the series in the engine while toggling its visibility. */
  visible: boolean;
}

export const LINE_TYPE_TO_U8: Record<NonNullable<series_options["line_type"]>, number> = {
  simple: 0,
  stepped: 1,
  curved: 2,
};

/** Style of a price line / crosshair line. */
export type line_style = "solid" | "dotted" | "dashed" | "large_dashed" | "sparse_dotted";
export const LINE_STYLE_TO_U8: Record<line_style, number> = {
  solid: 0,
  dotted: 1,
  dashed: 2,
  large_dashed: 3,
  sparse_dotted: 4,
};

/** Options for {@link series_api.create_price_line}. */
export interface price_line_options {
  price: number;
  color?: string;
  line_width?: number;
  line_style?: line_style;
  /** Axis label text; defaults to the formatted price. */
  title?: string;
}

/** A handle to a created price line. */
export interface price_line_api {
  remove(): void;
  readonly id: number;
}

/** A per-bar marker on a series (roadmap Phase B4). Mirrors lightweight-charts `SeriesMarker`. */
export interface series_marker {
  /** Bar time (must match a data point's time). */
  time: number;
  /** Placement relative to the bar. LWC names are canonical; short aliases remain compatible. */
  position?: "aboveBar" | "belowBar" | "inBar" | "above" | "below";
  /** Marker shape. Default `"circle"`. */
  shape?: "circle" | "square" | "arrowUp" | "arrowDown";
  /** Fill color (any CSS color the engine parses). Default series color. */
  color?: string;
  /** Optional label rendered beside the marker. */
  text?: string;
}

export interface series_marker_options {
  /** Expand price-scale pixel margins so marker shapes remain visible. Default `true` (LWC). */
  auto_scale: boolean;
}

export const KIND_TO_U8: Record<series_kind, number> = {
  candlestick: 0,
  bar: 1,
  line: 2,
  area: 3,
  histogram: 4,
  baseline: 5,
};

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

/** A single data series on the chart. */
export interface series_api {
  /** Replace the series' data. Accepts OHLC or single-value points; packed to typed arrays here. */
  set_data(data: readonly series_data[]): void;
  /** Append a new point or replace the last one (streaming). */
  update(point: series_data): void;
  /** Apply series options (currently: `color`). */
  apply_options(options: Partial<series_options>): void;
  /** Change how the primary series is drawn (candlestick/bar/line/area/histogram). */
  set_type(kind: series_kind): void;
  /** Move this series into stacked pane `pane_index` (0 = price pane), creating it if needed. */
  move_to_pane(pane_index: number, stretch?: number): void;
  /** Add a horizontal price line on this series; returns a handle with `.remove()`. */
  create_price_line(options: price_line_options): price_line_api;
  /** Replace this series' per-bar markers (pass `[]` to clear). Roadmap Phase B4. */
  set_markers(markers: readonly series_marker[], options?: Partial<series_marker_options>): void;
  /** Price-scale handle currently used by this series. */
  price_scale(): price_scale_api;
  price_to_coordinate(price: number): number | null;
  coordinate_to_price(coordinate: number): number | null;
  bars_in_logical_range(range: logical_range): bars_info | null;
  data_by_index(logical_index: number, mismatch_direction?: mismatch_direction): series_data | null;
  data(): readonly series_data[];
  series_type(): series_kind;
  subscribe_data_changed(handler: data_changed_handler): void;
  unsubscribe_data_changed(handler: data_changed_handler): void;
  /** The engine-side series id. */
  readonly id: number;
}

/** The horizontal (time) scale. */
export interface time_scale_api {
  /** Distance in logical bars between the latest point and the right edge. */
  scroll_position(): number;
  /** Scroll to a logical right-edge position. Animation is currently applied immediately. */
  scroll_to_position(position: number, animated: boolean): void;
  /** Return the latest point to the real-time edge. */
  scroll_to_real_time(): void;
  /** Restore configured default spacing and right offset. */
  reset_time_scale(): void;
  fit_content(): void;
  apply_options(options: Partial<time_scale_options>): void;
  options(): time_scale_options;
  get_visible_logical_range(): logical_range | null;
  set_visible_logical_range(range: logical_range): void;
  get_visible_range(): time_range | null;
  set_visible_range(range: time_range): void;
  /** Fire after the visible logical range changes. */
  subscribe_visible_logical_range_change(handler: visible_logical_range_handler): void;
  unsubscribe_visible_logical_range_change(handler: visible_logical_range_handler): void;
  /** Fire after the visible time range changes. */
  subscribe_visible_time_range_change(handler: visible_time_range_handler): void;
  unsubscribe_visible_time_range_change(handler: visible_time_range_handler): void;
  time_to_coordinate(time: number): number | null;
  coordinate_to_time(x: number): number | null;
  logical_to_coordinate(logical: number): number | null;
  coordinate_to_logical(x: number): number | null;
  /** Exact timestamp lookup, or LWC-compatible lower-bound lookup when `find_nearest` is true. */
  time_to_index(time: number, find_nearest?: boolean): number | null;
  /** Current media-coordinate width of the horizontal scale. */
  width(): number;
  /** Current media-coordinate height of the horizontal axis, or zero when hidden. */
  height(): number;
}

/** A pane price scale. Left/right are visible axes; the empty id is an independent overlay. */
export interface price_scale_api {
  apply_options(options: deep_partial<price_scale_options>): void;
  options(): price_scale_options;
  width(): number;
  set_visible_range(range: price_range): void;
  get_visible_range(): price_range | null;
  set_auto_scale(on: boolean): void;
}

/** The chart. Create with {@link create_chart}. */
/** A stacked pane (roadmap Phase B1). Mirrors lightweight-charts `IPaneApi`. */
export interface pane_api {
  /** This pane's index (0 = top/price pane). */
  pane_index(): number;
  /** Current CSS height in px (from the last layout pass). */
  get_height(): number;
  /** Resize this pane to `height` CSS px, absorbing the delta from its neighbour. */
  set_height(height: number): void;
  /** This pane's relative stretch factor (height weight). */
  get_stretch_factor(): number;
  /** Set this pane's relative stretch factor. */
  set_stretch_factor(factor: number): void;
}

export interface chart_api {
  /** Active pane backend: `webgpu` when available, otherwise the shared `canvas2d` fallback. */
  backend(): "webgpu" | "canvas2d";
  add_series(kind: series_kind, options?: Partial<series_options>): series_api;
  /** Add a Rust-native simple moving-average line derived from an existing series. */
  add_sma(source: series_api, period: number, options?: Partial<series_options>): series_api;
  /** Add a Rust-native exponential moving-average line derived from an existing series. */
  add_ema(source: series_api, period: number, options?: Partial<series_options>): series_api;
  /** Add upper, middle, and lower Rust-native Bollinger-band lines. */
  add_bollinger(source: series_api, period: number, deviation?: number, options?: Partial<series_options>): [series_api, series_api, series_api];
  apply_options(options: deep_partial<chart_options>): void;
  options(): unknown;
  time_scale(): time_scale_api;
  price_scale(price_scale_id?: "left" | "right" | "", pane_index?: number): price_scale_api;
  /** The stacked panes, top to bottom (roadmap Phase B1). At least one always exists. */
  panes(): pane_api[];
  price_to_coordinate(price: number): number | null;
  coordinate_to_price(y: number): number | null;
  /** Fire on every crosshair move (and once with `point: null` when the cursor leaves). */
  subscribe_crosshair_move(handler: mouse_event_handler): void;
  unsubscribe_crosshair_move(handler: mouse_event_handler): void;
  /** Fire on a click/tap inside the pane. */
  subscribe_click(handler: mouse_event_handler): void;
  unsubscribe_click(handler: mouse_event_handler): void;
  /** Fire on a double-click inside the pane (the default fit-content action still runs). */
  subscribe_dbl_click(handler: dbl_click_handler): void;
  unsubscribe_dbl_click(handler: dbl_click_handler): void;
  /** Fire after the visible logical range changes. */
  subscribe_visible_logical_range_change(handler: visible_logical_range_handler): void;
  unsubscribe_visible_logical_range_change(handler: visible_logical_range_handler): void;
  /** Fire after the visible time range changes. */
  subscribe_visible_time_range_change(handler: visible_time_range_handler): void;
  unsubscribe_visible_time_range_change(handler: visible_time_range_handler): void;
  /** Manually set the CSS size (and optional devicePixelRatio). Ignored while `autoSize` is on. */
  resize(width: number, height: number, dpr?: number): void;
  /** Force a repaint. Normally unnecessary — mutating calls repaint themselves. */
  render(): void;
  /** Snapshot the composed pane and axis layers at their current device-pixel resolution. */
  take_screenshot(): HTMLCanvasElement;
  /** Tear down: remove canvases and listeners. */
  remove(): void;
}

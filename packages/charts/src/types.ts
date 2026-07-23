/**
 * Public data, option, and handle types for `@tradeaion/charts` (snake_case; semantics mirror
 * lightweight-charts v5). Extracted from `index.ts`.
 */

import type { pane_primitive, pane_primitive_handle, series_primitive, series_primitive_handle } from "./primitives.js";
import type { custom_series_pane_view } from "./custom_series.js";

// ---------------------------------------------------------------------------------------------
// Data & option types
// ---------------------------------------------------------------------------------------------

/**
 * A series kind. Maps to the engine's numeric kind at the boundary. `"custom"` is only ever
 * REPORTED (by {@link series_api.series_type} for a custom series); it is not accepted by
 * {@link chart_api.add_series} — custom series are created with {@link chart_api.add_custom_series}.
 */
export type series_kind = "candlestick" | "bar" | "line" | "area" | "histogram" | "baseline" | "custom";

/** Calendar day (LWC `BusinessDay`), interpreted at UTC midnight. `month`/`day` are 1-based. */
export interface business_day {
  year: number;
  month: number;
  day: number;
}

/**
 * A point in time (LWC `Time`). Accepted forms at the input boundary:
 * - `number` — UTC timestamp in seconds since the epoch;
 * - `business_day` — `{ year, month, day }`, taken at UTC midnight;
 * - `string` — `"YYYY-MM-DD"`, taken at UTC midnight.
 *
 * The engine stores UTC seconds; business-day/string inputs are converted at the boundary. Values
 * the engine returns (e.g. `data()`, crosshair params) are always the numeric UTC-seconds form.
 */
export type time = number | business_day | string;

/** OHLC bar for candlestick/bar series. */
export interface ohlc_data {
  time: time;
  open: number;
  high: number;
  low: number;
  close: number;
  /**
   * Optional body color for this bar (LWC `BarData.color`/`CandlestickData.color`); when missed,
   * the color from the series options is used. snake_case per the package API convention.
   */
  color?: string;
  /**
   * Optional wick color for this bar (LWC `CandlestickData.wickColor`); when missed, the color
   * from the series options is used. snake_case per the package API convention.
   */
  wick_color?: string;
  /**
   * Optional border color for this bar (LWC `CandlestickData.borderColor`); when missed, the
   * color from the series options is used. snake_case per the package API convention.
   */
  border_color?: string;
}

/** Single-value point for line/area/histogram series. */
export interface single_value_data {
  time: time;
  value: number;
  /**
   * Optional color for this point (LWC `LineData.color`/`HistogramData.color`); when missed, the
   * color from the series options is used.
   */
  color?: string;
}

/**
 * A whitespace point (LWC `WhitespaceData`): reserves a time slot without carrying a value.
 * The engine keeps the row as an explicit empty slot instead of dropping it, so it can later be
 * replaced by a real bar via {@link series_api.update}.
 */
export interface whitespace_data {
  time: time;
}

export type series_data = ohlc_data | single_value_data | whitespace_data;

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

/**
 * The last value of a series as reported by the engine (cf. LWC `LastValueDataResult`, which
 * instead returns `{ noData, price, color, ... }`). `formatted` is the value rendered with the
 * series' price format; `time` is the UTC-second timestamp of the bar the value came from.
 */
export interface last_value_data {
  value: number;
  formatted: string;
  time: number;
}

/** Parameters delivered to crosshair-move and click subscribers (mirrors LWC `MouseEventParams`). */
export interface mouse_event_params {
  /** UTC seconds of the bar under the cursor, or `null` off the data. */
  time: number | null;
  /** Float logical (bar) index under the cursor, or `null` when there is no data. */
  logical: number | null;
  /** Cursor position in CSS px relative to the pane, or `null` when the cursor left the chart. */
  point: { x: number; y: number } | null;
  /** Index of the pane under the cursor, or `null` over an axis strip or outside the panes. */
  pane_index: number | null;
  /** Per-series value at the hovered bar, keyed by the series handle. */
  series_data: Map<series_api, ohlc_data | single_value_data>;
  /**
   * The series under the cursor (LWC `MouseEventParams.hoveredSeries`), from the engine's
   * per-kind hit tests (candle/bar high-low range, histogram column, line stroke) — or a
   * series primitive's hit, whose owning series reports here. `null` when nothing is hit.
   */
  hovered_series: series_api | null;
  /**
   * The `external_id` a primitive's `hit_test` reported for the hovered object (LWC
   * `MouseEventParams.hoveredObjectId`), or `null` when no primitive is hit.
   */
  hovered_object_id: string | null;
}

export type mouse_event_handler = (params: mouse_event_params) => void;
export type dbl_click_handler = (params: mouse_event_params) => void;
export type visible_logical_range_handler = (range: logical_range | null) => void;
export type visible_time_range_handler = (range: time_range | null) => void;
/** Receives the time scale's new media size in px (LWC `SizeChangeEventHandler`). */
export type size_change_handler = (width: number, height: number) => void;

export interface time_scale_options {
  /** Distance between adjacent bars in CSS pixels. */
  bar_spacing: number;
  /** Empty logical bars between the final data point and the right edge. */
  right_offset: number;
  /** Minimum bar spacing in CSS pixels (LWC `minBarSpacing`, default 0.5). */
  min_bar_spacing?: number;
  /** Maximum bar spacing in CSS pixels (LWC `maxBarSpacing`); omit for unlimited. */
  max_bar_spacing?: number;
  /** Right margin after the last bar in CSS pixels (LWC `rightOffsetPixels`). */
  right_offset_pixels?: number;
  /** Show the time of day (not just the date) in axis/crosshair labels (LWC `timeVisible`). */
  time_visible?: boolean;
  /** Include seconds when the time is shown (LWC `secondsVisible`). */
  seconds_visible?: boolean;
  /** Prevent scrolling past the first data point on the left (LWC `fixLeftEdge`). */
  fix_left_edge?: boolean;
  /** Prevent scrolling past the last data point on the right (LWC `fixRightEdge`). */
  fix_right_edge?: boolean;
  /** Keep the visible range constant across chart resizes (LWC `lockVisibleTimeRangeOnResize`). */
  lock_visible_time_range_on_resize?: boolean;
  /** Keep the right-most bar pinned to the right edge while scrolling (LWC `rightBarStaysOnScroll`). */
  right_bar_stays_on_scroll?: boolean;
  /**
   * Shift the visible range to the right (into the future) by the number of new bars when new
   * data is added. Only applies when the last bar is visible (LWC `shiftVisibleRangeOnNewBar`,
   * default `true`).
   */
  shift_visible_range_on_new_bar?: boolean;
  /**
   * Allow the visible range to shift right when a new bar replaces an existing whitespace time
   * point. Only applies when the last bar is visible and `shift_visible_range_on_new_bar` is
   * enabled (LWC `allowShiftVisibleRangeOnWhitespaceReplacement`, default `false`).
   */
  allow_shift_visible_range_on_whitespace_replacement?: boolean;
  /**
   * Show the whole time-scale strip (LWC `timeScale.visible`, default `true`). Distinct from
   * `time_visible`, which only controls whether the labels show the time of day.
   */
  visible?: boolean;
  /** Draw small vertical lines on the time axis labels (LWC `ticksVisible`, default `false`). */
  ticks_visible?: boolean;
  /**
   * Minimum height of the time scale in CSS px (LWC `minimumHeight`, default 0 = auto, i.e.
   * ~28 px). Exceeded when the scale needs more space; useful to align horizontally stacked
   * charts' scale heights.
   */
  minimum_height?: number;
  /**
   * Maximum tick-mark label length in characters, overriding the built-in cap
   * (LWC `tickMarkMaxCharacterLength`, default 8).
   */
  tick_mark_max_character_length?: number;
  /** Custom time-axis tick formatter (LWC `tickMarkFormatter`). Receives `(timeSeconds, tickMarkType)`
   *  where tickMarkType is 0 Year, 1 Month, 2 DayOfMonth, 3 Time, 4 TimeWithSeconds. */
  tick_mark_formatter?: (time: number, tick_mark_type: number) => string;
}

export interface price_scale_options {
  /** 0 normal, 1 logarithmic, 2 percentage, 3 indexed-to-100 (LWC values). */
  mode: 0 | 1 | 2 | 3;
  auto_scale: boolean;
  invert_scale: boolean;
  scale_margins: { top: number; bottom: number };
  /** Align price scale labels to prevent them from overlapping (LWC `alignLabels`, default `true`). */
  align_labels?: boolean;
  /** Draw small horizontal lines on the price axis labels (LWC `ticksVisible`, default `false`). */
  ticks_visible?: boolean;
  /**
   * Show the top and bottom corner labels only when their text is fully visible
   * (LWC `entireTextOnly`, default `false`).
   */
  entire_text_only?: boolean;
  /**
   * Minimum width of the price scale in CSS px (LWC `minimumWidth`, default 0 = auto). Exceeded
   * when the scale needs more space; useful to align vertically stacked charts' scale widths.
   */
  minimum_width?: number;
  /**
   * Price scale text color (LWC `textColor`); when unset, the scale follows `layout.textColor`.
   */
  text_color?: string;
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

/**
 * One crosshair line (LWC `CrosshairLineOptions`). `labelVisible`/`labelBackgroundColor` keep
 * LWC's camelCase names, matching the engine's serde keys.
 */
export interface crosshair_line_options {
  color?: string;
  /** Stroke width in CSS px. */
  width?: number;
  /** Line style (`line_style` value; default LargeDashed). */
  style?: number;
  visible?: boolean;
  /** Display the crosshair label on the relevant scale (LWC `labelVisible`, default `true`). */
  labelVisible?: boolean;
  /** Crosshair label background color (LWC `labelBackgroundColor`). */
  labelBackgroundColor?: string;
}
/** Custom label formatters (LWC `localization`). Each receives numbers and returns a string. */
export interface localization_options {
  /**
   * Current locale used to format dates. Uses the browser's language settings by default
   * (LWC `localization.locale`, default `navigator.language`).
   */
  locale?: string;
  /**
   * Date formatting string. Can contain `yyyy`, `yy`, `MMMM`, `MMM`, `MM` and `dd` literals
   * which will be replaced with the corresponding date's value. Ignored when `time_formatter`
   * is specified (LWC `localization.dateFormat`, default `'dd MMM \'yy'`).
   */
  date_format?: string;
  /** Format any non-percentage price label (axis ticks, last-value badge, crosshair, price lines). */
  price_formatter?: (price: number) => string;
  /** Format the crosshair time label. Receives the UTC-second timestamp. */
  time_formatter?: (time: number) => string;
}

/** Pan/scroll gesture toggles (LWC `handleScroll`). `false` disables all scrolling. */
export interface handle_scroll_options {
  /** Drag inside the pane to pan the time scale. */
  pressed_mouse_move?: boolean;
  /** Horizontal wheel/trackpad scroll pans the time scale (LWC `handleScroll.mouseWheel`). */
  mouse_wheel?: boolean;
  /** Horizontal one-finger touch drag pans the time scale. */
  horz_touch_drag?: boolean;
  /** Vertical one-finger touch drag participates in panning. */
  vert_touch_drag?: boolean;
}

/** Zoom/scale gesture toggles (LWC `handleScale`). `false` disables all zooming. */
export interface handle_scale_options {
  /** Mouse-wheel zoom on the pane. */
  mouse_wheel?: boolean;
  /** Two-finger touch pinch zoom. */
  pinch?: boolean;
  /**
   * Double-clicking a price/time axis resets it (LWC `handleScale.axisDoubleClickReset`).
   * `true`/`false` toggles both axes; the object form toggles each independently.
   */
  axis_double_click_reset?: boolean | { time?: boolean; price?: boolean };
  /**
   * Press-and-drag on an axis strip scales it (LWC `axisPressedMouseMove`): vertical drag on a
   * price axis scales that price scale (disabling autoscale), horizontal drag on the time axis
   * scales bar spacing. `true`/`false` toggles both; the object form toggles each independently.
   */
  axis_pressed_mouse_move?: boolean | { time?: boolean; price?: boolean };
}

/** Momentum ("kinetic") scroll after a pan flick (LWC `kineticScroll`). */
export interface kinetic_scroll_options {
  /** Coast after a one-finger touch flick. Default `true`. */
  touch?: boolean;
  /** Coast after a mouse-drag flick. Default `false`. */
  mouse?: boolean;
}

/**
 * Chart-level cosmetics of one visible price axis (LWC `leftPriceScale`/`rightPriceScale`).
 * Keys keep LWC's camelCase names, matching the engine's serde keys; the engine routes them to
 * the corresponding scale.
 */
export interface chart_price_scale_options {
  visible: boolean;
  borderVisible: boolean;
  borderColor: string;
  /** Align price scale labels to prevent them from overlapping (LWC `alignLabels`, default `true`). */
  alignLabels?: boolean;
  /** Draw small horizontal lines on the price axis labels (LWC `ticksVisible`, default `false`). */
  ticksVisible?: boolean;
  /**
   * Show the top and bottom corner labels only when their text is fully visible
   * (LWC `entireTextOnly`, default `false`).
   */
  entireTextOnly?: boolean;
  /**
   * Minimum width of the price scale in CSS px (LWC `minimumWidth`, default 0 = auto). Exceeded
   * when the scale needs more space; useful to align vertically stacked charts' scale widths.
   */
  minimumWidth?: number;
  /** Price scale text color (LWC `textColor`); when unset, the scale follows `layout.textColor`. */
  textColor?: string;
}

/** Crosshair "tracking mode" behavior on touch (LWC `trackingMode`). Package-level. */
export interface tracking_mode_options {
  /**
   * How tracking mode exits (LWC `trackingMode.exitMode`). `"on_touch_end"` (LWC
   * `TrackingModeExitMode.OnTouchEnd`) clears the crosshair when the finger lifts;
   * `"on_next_tap"` (default, LWC `TrackingModeExitMode.OnNextTap`) keeps it until the next
   * tap ends.
   */
  exit_mode?: "on_next_tap" | "on_touch_end";
}

export interface chart_options {
  layout: {
    background: { type: string; color: string };
    textColor: string;
    fontSize: number;
    fontFamily: string;
    attributionLogo: boolean;
    panes: {
      separatorColor: string;
      /**
       * Allow dragging pane separators to resize panes (LWC `layout.panes.enableResize`,
       * default `true`). Package-level: drives the separator drag and its hover cursor; it is
       * stripped before options reach the engine.
       */
      enableResize: boolean;
    };
  };
  grid: { vertLines: grid_line_options; horzLines: grid_line_options };
  crosshair: { vertLine: crosshair_line_options; horzLine: crosshair_line_options; mode: number };
  leftPriceScale: chart_price_scale_options;
  rightPriceScale: chart_price_scale_options;
  /** Time-axis strip cosmetics (LWC `timeScale.borderVisible`/`borderColor`). */
  timeScale: { borderVisible: boolean; borderColor: string };
  /**
   * Large text label painted inside the pane (LWC v4 `watermark`). `color` is any CSS color
   * (include alpha for a faint mark; the default is fully transparent). Aion draws it on the shared
   * overlay above the series — a deliberate divergence needed to stay pixel-identical across the
   * WebGPU and Canvas2D backends.
   */
  watermark: {
    visible: boolean;
    text: string;
    color: string;
    fontSize: number;
    fontFamily: string;
    fontStyle: string;
    horzAlign: "left" | "center" | "right";
    vertAlign: "top" | "center" | "bottom";
  };
  /** Install a ResizeObserver so the chart tracks its container's size. Default `false` (LWC parity). */
  autoSize: boolean;
  hoveredSeriesOnTop: boolean;
  /** Custom label formatters (LWC `localization`). Package-level; carries JS callbacks. */
  localization: localization_options;
  /** Enable/disable panning gestures (LWC `handleScroll`). Default `true`. Package-level. */
  handle_scroll: boolean | handle_scroll_options;
  /** Enable/disable zoom gestures (LWC `handleScale`). Default `true`. Package-level. */
  handle_scale: boolean | handle_scale_options;
  /** Momentum scroll after a pan flick (LWC `kineticScroll`). Default touch-only. Package-level. */
  kinetic_scroll: boolean | kinetic_scroll_options;
  /** Touch crosshair tracking-mode behavior (LWC `trackingMode`). Package-level. */
  tracking_mode: tracking_mode_options;
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
  /** Candlestick up-bar wick color. Until set, follows `up_color` (LWC parity). Pass `""` to
   *  clear a previously-pinned color and go back to following the body color. */
  wick_up_color: string;
  /** Candlestick down-bar wick color. Until set, follows `down_color`. `""` clears the override. */
  wick_down_color: string;
  /** Candlestick up-bar border color. Until set, follows `up_color` (LWC parity). `""` clears it. */
  border_up_color: string;
  /** Candlestick down-bar border color. Until set, follows `down_color`. `""` clears the override. */
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
  /** Show the last-value badge on the price scale (LWC `lastValueVisible`, default `true`). */
  last_value_visible?: boolean;
  /** Show the series price line at the last value (LWC `priceLineVisible`, default `true`). */
  price_line_visible?: boolean;
  /** Value the price line tracks (LWC `PriceLineSource`): 0 LastBar (default), 1 LastVisible. */
  price_line_source?: 0 | 1;
  /** Price line width in CSS px (LWC `priceLineWidth`, default 1). */
  price_line_width?: number;
  /** Price line color (LWC `priceLineColor`); default `""` follows the series color. */
  price_line_color?: string;
  /** Price line style, a `LINE_STYLE_TO_U8` value (LWC `priceLineStyle`, default 2 Dashed). */
  price_line_style?: number;
  /** Line stroke style 0-4, a `LINE_STYLE_TO_U8` value (LWC `lineStyle`, default 0 Solid). */
  line_style?: number;
  /** Draw the line itself on line/area/baseline series (LWC `lineVisible`, default `true`). */
  line_visible?: boolean;
  /** Point-marker disc radius in CSS px (LWC `pointMarkersRadius`); unset = auto. */
  point_markers_radius?: number;
  /** Show the crosshair marker on this series (LWC `crosshairMarkerVisible`, default `true`). */
  crosshair_marker_visible?: boolean;
  /** Crosshair marker radius in CSS px (LWC `crosshairMarkerRadius`, default 4). */
  crosshair_marker_radius?: number;
  /** Crosshair marker border color (LWC `crosshairMarkerBorderColor`, default `""` = none). */
  crosshair_marker_border_color?: string;
  /** Crosshair marker background color (LWC `crosshairMarkerBackgroundColor`, default `""` = none). */
  crosshair_marker_background_color?: string;
  /** Crosshair marker border width in CSS px (LWC `crosshairMarkerBorderWidth`, default 2). */
  crosshair_marker_border_width?: number;
  /** Baseline: first gradient fill color above the baseline (LWC `topFillColor1`). */
  top_fill_color1?: string;
  /** Baseline: second gradient fill color above the baseline (LWC `topFillColor2`). */
  top_fill_color2?: string;
  /** Baseline: line color above the baseline (LWC `topLineColor`). */
  top_line_color?: string;
  /** Baseline: line width above the baseline in CSS px (LWC `topLineWidth`). */
  top_line_width?: number;
  /** Baseline: line style above the baseline, a `LINE_STYLE_TO_U8` value (LWC `topLineStyle`). */
  top_line_style?: number;
  /** Baseline: first gradient fill color below the baseline (LWC `bottomFillColor1`). */
  bottom_fill_color1?: string;
  /** Baseline: second gradient fill color below the baseline (LWC `bottomFillColor2`). */
  bottom_fill_color2?: string;
  /** Baseline: line color below the baseline (LWC `bottomLineColor`). */
  bottom_line_color?: string;
  /** Baseline: line width below the baseline in CSS px (LWC `bottomLineWidth`). */
  bottom_line_width?: number;
  /** Baseline: line style below the baseline, a `LINE_STYLE_TO_U8` value (LWC `bottomLineStyle`). */
  bottom_line_style?: number;
  /** Histogram base value the bars grow from (LWC `base`, default 0). */
  base?: number;
  /** Area: invert the filled area (fill above the line) (LWC `invertFilledArea`, default `false`). */
  invert_filled_area?: boolean;
  /** Bar: draw the open tick on each bar (LWC `openVisible`, default `true`). */
  open_visible?: boolean;
  /** Bar: draw thin bars when the bar spacing is small (LWC `thinBars`, default `true`). */
  thin_bars?: boolean;
  /**
   * Per-series price formatting (LWC `priceFormat`). Built-in types (`"price"`/`"volume"`/
   * `"percent"`, LWC `PriceFormatBuiltIn`) take `precision` and `min_move` (LWC
   * `precision`/`minMove`, snake_case per the package API convention); `"custom"` (LWC
   * `PriceFormatCustom`) installs a JS formatter callback with an optional `min_move`.
   */
  price_format?:
    | { type: "price" | "volume" | "percent"; precision?: number; min_move?: number }
    | { type: "custom"; formatter: (price: number) => string; min_move?: number };
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
  /** Draw the line itself (LWC `lineVisible`, default `true`). */
  line_visible?: boolean;
  /** Show the price label on the axis (LWC `axisLabelVisible`, default `true`). */
  axis_label_visible?: boolean;
  /** Axis label background color; defaults to the line color (LWC `axisLabelColor`). */
  axis_label_color?: string;
  /** Axis label text color (LWC `axisLabelTextColor`). */
  axis_label_text_color?: string;
}

/** A handle to a created price line. */
export interface price_line_api {
  remove(): void;
  /** Deep-merge a patch onto this line's options (LWC `IPriceLine.applyOptions`). */
  apply_options(options: Partial<price_line_options>): void;
  /** The current (deep-merged) options of this price line (LWC `IPriceLine.options`). */
  options(): price_line_options;
  readonly id: number;
}

/** A per-bar marker on a series (roadmap Phase B4). Mirrors lightweight-charts `SeriesMarker`. */
export interface series_marker {
  /** Bar time (must match a data point's time). Accepts the same forms as data `time`. */
  time: time;
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
  custom: 6,
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
  /**
   * Remove `count` data items from the end of the series (LWC `ISeriesApi.pop`, default
   * `count: 1`). Divergence: LWC returns the removed items; here the engine drops them and the
   * method returns nothing.
   */
  pop(count?: number): void;
  /**
   * The last value data of the series (LWC `ISeriesApi.lastValueData`). `global_last: false`
   * reads the last value in the current visible range, `true` the absolute last value. Returns
   * `null` when the series has no value.
   */
  last_value_data(global_last?: boolean): last_value_data | null;
  /**
   * The current price formatter of this series (LWC `ISeriesApi.priceFormatter`). Divergence:
   * LWC returns an `IPriceFormatter` object with a `format` method; this snake_case API returns
   * the bare format function `(price) => string`.
   */
  price_formatter(): (price: number) => string;
  /** Apply series options (currently: `color`). */
  apply_options(options: Partial<series_options>): void;
  /** The current (deep-merged) options of this series (LWC `ISeriesApi.options`). */
  options(): series_options;
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
  /**
   * Attach a series primitive (LWC `ISeriesApi.attachPrimitive`, plugin platform Phase C-b)
   * and repaint. The primitive records backend-neutral draw commands (no raw canvas), so its
   * output is identical on the WebGPU and Canvas2D backends; its price converter and price-axis
   * labels resolve on this series' price scale, and its `autoscale_info` hook can expand that
   * scale's range. Divergence: LWC returns `void`; here the returned handle detaches. Removing
   * the series auto-detaches its primitives (the `detached` hook fires).
   */
  attach_primitive(primitive: series_primitive): series_primitive_handle;
  /** The engine-side series id. */
  readonly id: number;
}

/** The horizontal (time) scale. */
export interface time_scale_api {
  /** Distance in logical bars between the latest point and the right edge. */
  scroll_position(): number;
  /**
   * Scroll to a logical right-edge position. `animated: true` eases over ~300 ms with a cubic
   * ease-out (suppressed under prefers-reduced-motion); falsy applies immediately.
   */
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
  /** Fire after the time scale's media size changes (LWC `subscribeSizeChange`). */
  subscribe_size_change(handler: size_change_handler): void;
  unsubscribe_size_change(handler: size_change_handler): void;
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
  /**
   * Move this pane to the `target` index (LWC `IPaneApi.moveTo`). Returns `false` without
   * changing anything when the engine rejects the move (e.g. a stale index after a removal).
   * Divergence: LWC returns `void`.
   */
  move_to(target: number): boolean;
  /** Whether this pane is kept while it has no series (LWC `IPaneApi.preserveEmptyPane`). */
  preserve_empty_pane(): boolean;
  /** Set whether to keep this pane while it has no series (LWC `IPaneApi.setPreserveEmptyPane`). */
  set_preserve_empty_pane(flag: boolean): void;
  /** The series attached to this pane, as live handles (LWC `IPaneApi.getSeries`). */
  get_series(): series_api[];
  /**
   * Attach a pane primitive (LWC `IPaneApi.attachPrimitive`, plugin platform Phase C-a) and
   * repaint. The primitive records backend-neutral draw commands (no raw canvas), so its
   * output is identical on the WebGPU and Canvas2D backends. Divergence: LWC returns `void`;
   * here the returned handle detaches. The pane binding is by index — it does not follow
   * later pane moves/removals (a removed pane's primitives draw nowhere until detached).
   */
  attach_primitive(primitive: pane_primitive): pane_primitive_handle;
  /**
   * This pane's price scale by id (LWC `IPaneApi.priceScale`): the visible `"left"`/`"right"`
   * axis, or `""` for the overlay scale. Divergence: LWC throws on an unknown id; here the id is
   * one of the three literals, so a scale always resolves.
   */
  price_scale(id: "left" | "right" | ""): price_scale_api;
}

export interface chart_api {
  /** Active pane backend: `webgpu` when available, otherwise the shared `canvas2d` fallback. */
  backend(): "webgpu" | "canvas2d";
  add_series(kind: series_kind, options?: Partial<series_options>): series_api;
  /**
   * Add a custom series (plugin platform Phase C-c; LWC `IChartApi.addCustomSeries`): a
   * user-defined series type rendered by the pane view's `render(ctx)` through backend-neutral
   * draw commands, so its output is pixel-identical on the WebGPU and Canvas2D backends. The
   * engine owns the time mapping and autoscale (via the view's `price_value_builder`); the
   * returned handle's `set_data`/`update`/`data` work on the raw plugin items. The view's
   * `default_options` merge under the caller's `options` (LWC `createCustomSeriesDefinition`);
   * the series options that make sense for a plugin-drawn series apply (`visible`,
   * `price_scale_id`/overlay, `pane`/`move_to_pane`, `last_value_visible`, the `price_line_*`
   * family, `price_format`) and unsupported style keys are ignored. Removing the series fires
   * the view's `destroy` hook.
   */
  add_custom_series(pane_view: custom_series_pane_view, options?: Partial<series_options>): series_api;
  /**
   * Remove a series (and any indicators derived from it). No-op for an already-removed or
   * foreign handle. The primary series (the first one created, engine id 0) may also be removed;
   * the engine tombstones it safely.
   */
  remove_series(series: series_api): void;
  /**
   * The chart's series in their engine (z-)order, as live handles. A series whose handle the
   * package no longer tracks is omitted. Cf. LWC's per-series `ISeriesApi.seriesOrder`.
   */
  series_order(): series_api[];
  /**
   * Reorder the chart's series to match `ordered` (cf. LWC's per-series
   * `ISeriesApi.setSeriesOrder`, elevated here to a whole-chart call). Returns `false` without
   * changing anything when the engine rejects the order.
   */
  set_series_order(ordered: series_api[]): boolean;
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
  /**
   * Add a stacked pane (LWC `IChartApi.addPane`) and return the handle for the new (last) index.
   * `preserve_empty` keeps the pane alive while it has no series (LWC `preserveEmptyPane`,
   * default `false`).
   */
  add_pane(preserve_empty?: boolean): pane_api;
  /**
   * Remove the pane at `index` (LWC `IChartApi.removePane`). Returns `false` without changing
   * anything when the engine refuses (e.g. an out-of-range index or the last pane). Divergence:
   * LWC returns `void`. Pane handles are index-based — a removal shifts the indices of the panes
   * below it, so re-fetch handles with {@link chart_api.panes} afterwards.
   */
  remove_pane(index: number): boolean;
  /**
   * Swap the panes at `first` and `second` (LWC `IChartApi.swapPanes`). Returns `false` without
   * changing anything when the engine rejects the swap. Divergence: LWC returns `void`. Pane
   * handles are index-based — re-fetch them with {@link chart_api.panes} afterwards.
   */
  swap_panes(first: number, second: number): boolean;
  price_to_coordinate(price: number): number | null;
  coordinate_to_price(y: number): number | null;
  /**
   * Set the crosshair position within the chart (LWC `IChartApi.setCrosshairPosition`). The
   * crosshair normally follows the user's cursor; setting it explicitly is useful to synchronise
   * the crosshairs of two separate charts. `time` accepts the same forms as data times.
   * Divergence: LWC throws on an unknown series; here the call is a silent no-op when the
   * position cannot be applied.
   */
  set_crosshair_position(price: number, time: time, series: series_api): void;
  /** Clear the crosshair position within the chart (LWC `IChartApi.clearCrosshairPosition`). */
  clear_crosshair_position(): void;
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
  /**
   * Snapshot the composed pane and axis layers at their current device-pixel resolution.
   * `add_top_layer: false` composites the pane only (no axis/input overlay);
   * `include_crosshair: false` hides the crosshair for the capture (both default `true`).
   */
  take_screenshot(add_top_layer?: boolean, include_crosshair?: boolean): HTMLCanvasElement;
  /** Whether the `autoSize` option is enabled and active (LWC `autoSizeActive`). */
  auto_size_active(): boolean;
  /** The container element passed to {@link create_chart} (LWC `chartElement`). */
  chart_element(): HTMLElement;
  /** Tear down: remove canvases and listeners. */
  remove(): void;
}

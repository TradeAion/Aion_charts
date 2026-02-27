/**
 * RayCore — Production-grade WASM charting library
 *
 * This is the hand-crafted TypeScript definitions file.
 * It supplements the auto-generated `wasm/pkg/raycore_wasm.d.ts` with:
 *   - Full typed event system (ChartEventMap, per-event payload types)
 *   - Properly typed `on<K>()`, `off<K>()`, `once<K>()` overloads
 *   - Typed CreateChartOptions / ThemeConfig interfaces
 *   - ChartGroup and ChartWorkspace classes
 *   - JSDoc on every public method
 *
 * @example
 * ```ts
 * import init, { RayCore } from './pkg/raycore_wasm.js';
 * await init();
 *
 * const chart = await RayCore.create_chart('#chart', {
 *   theme: 'dark',
 *   autoRender: true,
 *   symbol: 'BTCUSD',
 * });
 *
 * chart.on('crosshairMove', (e) => console.log(e.price));
 * chart.on('click', (e) => console.log('bar', e.barIndex));
 * ```
 */

// ─────────────────────────────────────────────────────────────────────────────
// Colour helpers
// ─────────────────────────────────────────────────────────────────────────────

/** [r, g, b, a] — each component 0.0–1.0 */
export type RgbaColor = [number, number, number, number];

// ─────────────────────────────────────────────────────────────────────────────
// Theme
// ─────────────────────────────────────────────────────────────────────────────

export interface ThemeColors {
  background?: RgbaColor;
  pane_background?: RgbaColor;
  grid?: RgbaColor;
  axis_border?: RgbaColor;
  text?: RgbaColor;
  watermark?: RgbaColor;
  bullish?: RgbaColor;
  bearish?: RgbaColor;
  volume_bullish?: RgbaColor;
  volume_bearish?: RgbaColor;
}

export interface ThemeCrosshairLine {
  color?: RgbaColor;
  width?: number;
  /** "solid" | "dashed" | "dotted" | "large_dashed" | "sparse_dotted" */
  style?: string;
  visible?: boolean;
  label_background?: RgbaColor;
  label_text?: RgbaColor;
}

export interface ThemeCrosshair {
  vert?: ThemeCrosshairLine;
  horiz?: ThemeCrosshairLine;
}

export interface ThemeTypography {
  font_family?: string;
  font_size?: number;
}

export interface ThemeLayout {
  price_scale_margins?: { top?: number; bottom?: number };
}

export interface ThemeSeriesDefaults {
  line_color?: RgbaColor;
  area_line?: RgbaColor;
  area_top?: RgbaColor;
  area_bottom?: RgbaColor;
  bar_up?: RgbaColor;
  bar_down?: RgbaColor;
  baseline_above_line?: RgbaColor;
  baseline_below_line?: RgbaColor;
  histogram?: RgbaColor;
}

/** Full custom theme config — any field omitted falls back to the Dark preset */
export interface ThemeConfig {
  colors?: ThemeColors;
  crosshair?: ThemeCrosshairLine & ThemeCrosshair;
  typography?: ThemeTypography;
  layout?: ThemeLayout;
  series?: ThemeSeriesDefaults;
}

/** Theme preset name or a full ThemeConfig object */
export type Theme = 'dark' | 'light' | ThemeConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Create / apply options
// ─────────────────────────────────────────────────────────────────────────────

export interface CrosshairOptions {
  /** "normal" | "magnet_ohlc" */
  mode?: 'normal' | 'magnet_ohlc';
}

export interface PriceScaleOptions {
  /** "normal" | "logarithmic" | "percentage" | "indexed_to_100" */
  mode?: 'normal' | 'logarithmic' | 'percentage' | 'indexed_to_100';
  margins?: { top?: number; bottom?: number };
}

/**
 * Options for `RayCore.create_chart()` and `chart.apply_options()`.
 * All fields are optional. Omitted fields use defaults on creation
 * or keep current values when passed to `apply_options()`.
 */
export interface CreateChartOptions {
  /**
   * Theme preset or custom theme.
   * @default 'dark'
   */
  theme?: Theme;
  /**
   * Renderer backend.
   * - `'auto'`     — WebGPU when available, else Canvas2D
   * - `'webgpu'`   — Force WebGPU (fails if unavailable)
   * - `'canvas2d'` — Force Canvas2D
   * @default 'auto'
   */
  renderer?: 'auto' | 'webgpu' | 'canvas2d';
  /**
   * Enable automatic requestAnimationFrame loop.
   * Set `false` to render manually via `chart.render()`.
   * @default true
   */
  autoRender?: boolean;
  /** Symbol string shown in watermark / header (e.g. "BTCUSD"). */
  symbol?: string;
  /** Interval string (e.g. "1D", "4H", "15m"). */
  interval?: string;
  /** Watermark text centred on the chart pane. */
  watermark?: string;
  crosshair?: CrosshairOptions;
  priceScale?: PriceScaleOptions;
}

// ─────────────────────────────────────────────────────────────────────────────
// Events
// ─────────────────────────────────────────────────────────────────────────────

export interface BaseChartEvent {
  /** Event name — same string used in `on(name, ...)` */
  type: string;
}

export interface CrosshairMoveEvent extends BaseChartEvent {
  type: 'crosshairMove';
  x: number;
  y: number;
  /** Bar index under cursor, or null if outside data range */
  barIndex: number | null;
  /** Price at cursor position */
  price: number;
  /** Unix timestamp (ms) of bar, or null */
  timestamp: number | null;
}

export interface ClickEvent extends BaseChartEvent {
  type: 'click';
  x: number;
  y: number;
  barIndex: number | null;
  price: number;
}

export interface VisibleRangeChangeEvent extends BaseChartEvent {
  type: 'visibleRangeChange';
  startBar: number;
  endBar: number;
}

export interface SymbolChangeEvent extends BaseChartEvent {
  type: 'symbolChange';
  symbol: string;
}

export interface IntervalChangeEvent extends BaseChartEvent {
  type: 'intervalChange';
  interval: string;
}

export interface ChartTypeChangeEvent extends BaseChartEvent {
  type: 'chartTypeChange';
  chartType: string;
}

export interface PriceScaleChangeEvent extends BaseChartEvent {
  type: 'priceScaleChange';
  mode: string;
}

export interface ResizeEvent extends BaseChartEvent {
  type: 'resize';
  width: number;
  height: number;
}

export interface DrawingCreatedEvent extends BaseChartEvent {
  type: 'drawingCreated';
  id: number;
  tool: string;
}

export interface DrawingSelectedEvent extends BaseChartEvent {
  type: 'drawingSelected';
  /** Drawing ID, or null when selection is cleared */
  id: number | null;
}

export interface ErrorEvent extends BaseChartEvent {
  type: 'error';
  message: string;
}

/** Map of event name → typed payload, used to type `on<K>()` overloads */
export interface ChartEventMap {
  crosshairMove:       CrosshairMoveEvent;
  click:               ClickEvent;
  visibleRangeChange:  VisibleRangeChangeEvent;
  symbolChange:        SymbolChangeEvent;
  intervalChange:      IntervalChangeEvent;
  chartTypeChange:     ChartTypeChangeEvent;
  priceScaleChange:    PriceScaleChangeEvent;
  resize:              ResizeEvent;
  drawingCreated:      DrawingCreatedEvent;
  drawingSelected:     DrawingSelectedEvent;
  error:               ErrorEvent;
}

// ─────────────────────────────────────────────────────────────────────────────
// CSS variables emitted onto the container element
// ─────────────────────────────────────────────────────────────────────────────

/**
 * CSS custom properties RayCore writes to its container element.
 *
 * @example
 * ```css
 * .tooltip { background: var(--raycore-bg); color: var(--raycore-text); }
 * .signal  { color: var(--raycore-bullish); }
 * ```
 */
export interface RayCoreCssVariables {
  '--raycore-bg': string;
  '--raycore-text': string;
  '--raycore-bullish': string;
  '--raycore-bearish': string;
  '--raycore-grid': string;
  '--raycore-border': string;
  '--raycore-watermark': string;
  '--raycore-crosshair': string;
  '--raycore-crosshair-label-bg': string;
  '--raycore-crosshair-label-text': string;
  '--raycore-font-family': string;
  '--raycore-font-size': string;
}

// ─────────────────────────────────────────────────────────────────────────────
// ChartGroup — multi-pane synchronisation
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Synchronises multiple chart panes (symbol, interval, time-range, crosshair).
 *
 * @example
 * ```ts
 * const group = new ChartGroup();
 * const paneA = group.add_pane('BTCUSD', '1D');
 * const paneB = group.add_pane('ETHUSD', '1D');
 * group.link_panes(paneA, paneB);
 * group.set_sync('time', true);
 * group.set_sync('crosshair', true);
 * ```
 */
export declare class ChartGroup {
  constructor();
  /** Release WASM memory. Call after `dispose()`. */
  free(): void;

  /** Add a pane to the group. Returns the group-pane ID. */
  add_pane(symbol: string, interval: string): number;

  /** Remove a pane from the group. */
  remove_pane(pane_id: number): boolean;

  /** Total pane count. */
  pane_count(): number;

  /**
   * Link two panes so sync events flow between them.
   * Returns true if the link was created.
   */
  link_panes(a: number, b: number): boolean;

  /** Unlink two panes. */
  unlink_panes(a: number, b: number): boolean;

  /**
   * Enable/disable a sync feature for all panes.
   * Feature names: `"symbol"`, `"interval"`, `"time"`, `"crosshair"`, `"data_range"`.
   */
  set_sync(feature: string, enabled: boolean): void;

  /** Per-pane sync control. */
  set_sync_for_pane(pane_id: number, feature: string, enabled: boolean): void;

  /** Per-link sync control (between two specific panes). */
  set_sync_for_link(pane_a: number, pane_b: number, feature: string, enabled: boolean): void;

  /** Auto-link new panes to all existing panes when added. */
  set_auto_link(enabled: boolean): void;

  /**
   * Notify the group that a pane's symbol changed.
   * Returns an array of pane IDs that should be updated.
   */
  update_symbol(source: number, symbol: string): number[];

  /** Notify interval change. Returns pane IDs to update. */
  update_interval(source: number, interval: string): number[];

  /**
   * Notify visible bar-range change.
   * Returns pane IDs to update.
   */
  update_time_range(source: number, start_bar: number, end_bar: number): number[];

  /**
   * Notify crosshair change.
   * `crosshair` format: `[active, x, y, bar_index, price, magnet]`
   * where `magnet`: 0 = normal, 1 = OHLC-magnet mode.
   * Returns pane IDs to update.
   */
  update_crosshair(source: number, crosshair: Float64Array): number[];

  /** Notify data timestamp range change. Returns pane IDs to update. */
  update_data_range(source: number, from_timestamp: number, to_timestamp: number): number[];

  /** Get symbol for a pane. */
  pane_symbol(pane_id: number): string;

  /** Get interval for a pane. */
  pane_interval(pane_id: number): string;

  /** Get visible bar range `[start, end]` for a pane, or empty array. */
  pane_time_range(pane_id: number): Float64Array;

  /** Get data timestamp range `[from_ts, to_ts]` for a pane, or empty array. */
  pane_data_range(pane_id: number): Float64Array;
}

// ─────────────────────────────────────────────────────────────────────────────
// ChartWorkspace — multi-pane split layout
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Manages a resizable split-pane workspace container.
 * Each pane gets a host `<div>` that you pass to `RayCore.create_chart()`.
 *
 * @example
 * ```ts
 * const ws = new ChartWorkspace('my-container-id');
 * const rootPaneId = ws.root_pane_id();
 * const hostId     = ws.pane_host_id(rootPaneId);
 * const chart      = await RayCore.create_chart(hostId, { autoRender: true });
 *
 * // Split vertically
 * const newPaneId = ws.split_active('vertical');
 * const newHostId = ws.pane_host_id(newPaneId);
 * const chart2    = await RayCore.create_chart(newHostId, { autoRender: true });
 * ```
 */
export declare class ChartWorkspace {
  /**
   * Create a workspace inside the element with the given ID.
   * The element must exist in the DOM and have explicit dimensions.
   */
  constructor(container_id: string);

  /** Release WASM memory and remove DOM elements. */
  free(): void;

  /** Tear down event listeners and release DOM nodes without freeing WASM. */
  dispose(): void;

  /** ID of the root (first) pane — always exists. */
  root_pane_id(): number;

  /** ID of the currently active (focused) pane. */
  active_pane_id(): number;

  /**
   * Get the host element ID for a pane.
   * Pass this string to `RayCore.create_chart()`.
   */
  pane_host_id(pane_id: number): string;

  /** Array of all current pane IDs. */
  pane_ids(): number[];

  /**
   * Split the active pane.
   * @param direction — `"vertical"` (left/right) or `"horizontal"` (top/bottom)
   * @returns The new pane ID.
   */
  split_active(direction: 'vertical' | 'horizontal'): number;

  /**
   * Split a specific pane by ID.
   * @returns The new pane ID.
   */
  split_pane(pane_id: number, direction: 'vertical' | 'horizontal'): number;

  /** Set which pane is active (focused). */
  set_active_pane(pane_id: number): boolean;

  // ── Styling ──────────────────────────────────────────────────────────────

  /** Set split divider visible line thickness (CSS px). */
  set_split_divider_thickness(thickness_css: number): void;

  /** Set split divider drag hit-area thickness (CSS px). */
  set_split_divider_hit_area(hit_area_css: number): void;

  /** Set split divider colour (RGBA 0–1). */
  set_split_divider_color(r: number, g: number, b: number, a: number): void;

  /** Set split divider colour when being dragged (RGBA 0–1). */
  set_split_divider_active_color(r: number, g: number, b: number, a: number): void;

  /** Set workspace pane background colour (RGBA 0–1). */
  set_workspace_pane_background_color(r: number, g: number, b: number, a: number): void;

  /** Set border colour of the active (focused) pane (RGBA 0–1). */
  set_workspace_active_pane_border_color(r: number, g: number, b: number, a: number): void;

  /** Set border width of the active pane (CSS px). */
  set_workspace_active_pane_border_width(width_css: number): void;
}

// ─────────────────────────────────────────────────────────────────────────────
// RayCore — main chart class
// ─────────────────────────────────────────────────────────────────────────────

export declare class RayCore {
  // ── Lifecycle ──────────────────────────────────────────────────────────────

  /**
   * Create and mount a new chart.
   *
   * @param container — `HTMLElement` or a string element ID (without `#`).
   * @param options   — Optional creation options.
   *
   * @example
   * ```ts
   * const chart = await RayCore.create_chart(
   *   document.getElementById('chart')!,
   *   { theme: 'dark', autoRender: true, symbol: 'BTCUSD' }
   * );
   * ```
   */
  static create_chart(
    container: HTMLElement | string,
    options?: CreateChartOptions,
  ): Promise<RayCore>;

  /**
   * @deprecated Use `RayCore.create_chart(container, options)` instead.
   */
  static create(container_id: string): Promise<RayCore>;

  /**
   * @deprecated Use `RayCore.create_chart(container, { renderer })` instead.
   */
  static create_with(container_id: string, renderer: string): Promise<RayCore>;

  /**
   * Apply a partial options update at runtime.
   * Only provided fields are changed; everything else stays unchanged.
   *
   * @example
   * ```ts
   * chart.apply_options({ theme: 'light' });
   * chart.apply_options({ crosshair: { mode: 'magnet_ohlc' }, watermark: 'ETH/USD' });
   * ```
   */
  apply_options(options: CreateChartOptions): void;

  /**
   * Destroy the chart: stop RAF, remove DOM nodes, release all WASM memory.
   * Always call this on unmount to prevent memory leaks.
   */
  dispose(): void;

  /** Release raw WASM memory. Call after `dispose()`. */
  free(): void;

  // ── Events ─────────────────────────────────────────────────────────────────

  /**
   * Subscribe to a typed chart event.
   *
   * @example
   * ```ts
   * chart.on('crosshairMove', ({ price, barIndex }) => {
   *   priceLabel.textContent = price.toFixed(2);
   * });
   * chart.on('error', ({ message }) => console.error(message));
   * ```
   */
  on<K extends keyof ChartEventMap>(
    event: K,
    callback: (event: ChartEventMap[K]) => void,
  ): void;

  /**
   * Unsubscribe a callback. Pass the **same function reference** used in `on()`.
   */
  off<K extends keyof ChartEventMap>(
    event: K,
    callback: (event: ChartEventMap[K]) => void,
  ): void;

  /**
   * Subscribe for exactly one invocation — auto-removed after firing.
   */
  once<K extends keyof ChartEventMap>(
    event: K,
    callback: (event: ChartEventMap[K]) => void,
  ): void;

  // ── Render control ─────────────────────────────────────────────────────────

  /**
   * Render one frame.
   * Only needed when `autoRender` is `false`; the RAF loop calls this automatically otherwise.
   */
  render(): void;

  /** Enable the automatic RAF render loop. */
  start_auto_render(): void;

  /** Disable the automatic RAF loop. You must call `render()` manually. */
  stop_auto_render(): void;

  /** Whether the automatic render loop is currently active. */
  is_auto_render(): boolean;

  // ── Theme ──────────────────────────────────────────────────────────────────

  /**
   * Current theme preset name.
   * @returns `"dark"`, `"light"`, or `"custom"`.
   */
  theme(): string;

  /**
   * CSS custom properties currently set on the container.
   * Use to synchronise tooltips and UI elements with the chart's palette.
   *
   * @example
   * ```ts
   * const vars = chart.get_css_variables();
   * myTooltip.style.background = vars['--raycore-bg'];
   * ```
   */
  get_css_variables(): RayCoreCssVariables;

  // ── Data loading ───────────────────────────────────────────────────────────

  /**
   * Load OHLCV data from columnar typed arrays.
   * All arrays must have the same length.
   * `timestamps` are Unix milliseconds as `BigUint64Array`.
   *
   * @example
   * ```ts
   * chart.set_data_arrays(opens, highs, lows, closes, volumes, timestamps);
   * ```
   */
  set_data_arrays(
    open:       Float32Array,
    high:       Float32Array,
    low:        Float32Array,
    close:      Float32Array,
    volume:     Float32Array,
    timestamps: BigUint64Array,
  ): void;

  /**
   * LWC-style upsert: appends a new bar if the timestamp is newer than the last,
   * or updates the last bar in-place if timestamps match.
   * Ideal for live-tick streaming.
   */
  upsert_bar(
    timestamp: bigint,
    open:      number,
    high:      number,
    low:       number,
    close:     number,
    volume:    number,
  ): void;

  /**
   * Append a single bar. The timestamp must be strictly greater than all existing bars.
   */
  append_bar(
    timestamp: bigint,
    open:      number,
    high:      number,
    low:       number,
    close:     number,
    volume:    number,
  ): void;

  /**
   * Overwrite the last (most-recent) bar in-place.
   * Used for streaming where the latest bar is still forming.
   */
  update_last_bar(
    timestamp: bigint,
    open:      number,
    high:      number,
    low:       number,
    close:     number,
    volume:    number,
  ): void;

  // ── Symbol / interval ──────────────────────────────────────────────────────

  /** Current symbol string. */
  symbol(): string;

  /** Update the symbol. Fires the `symbolChange` event. */
  set_symbol(symbol: string): void;

  /** Current interval string. */
  interval(): string;

  /** Update the interval string. Fires the `intervalChange` event. */
  set_interval(interval: string): void;

  // ── Chart type ─────────────────────────────────────────────────────────────

  /**
   * Switch the main chart type.
   *
   * Accepted values: `"candlestick"`, `"candles"`, `"ohlc"`, `"bars"`,
   * `"line"`, `"area"`, `"heikin_ashi"`, `"ha"`, `"baseline"`.
   *
   * Fires the `chartTypeChange` event.
   */
  set_chart_type(chart_type: string): void;

  /** Current chart type string. */
  get_chart_type(): string;

  /** Comma-separated string of all available chart type names. */
  static get_available_chart_types(): string;

  // ── Viewport ───────────────────────────────────────────────────────────────

  /**
   * Current visible bar range `[start_bar, end_bar]`.
   * Returns an empty `Float64Array` if no data is loaded.
   */
  visible_range(): Float64Array;

  /**
   * Set the visible bar range. Fires `visibleRangeChange`.
   */
  set_visible_range(start: number, end: number): void;

  /**
   * Zoom to a specific timestamp range.
   * Both arguments are Unix milliseconds as `bigint`.
   */
  zoom_to_range(start: bigint, end: bigint): void;

  /**
   * Data timestamp range `[from_ts, to_ts]` in milliseconds.
   * Returns empty array if no bars are loaded.
   */
  data_range(): Float64Array;

  // ── Price scale ────────────────────────────────────────────────────────────

  /**
   * Switch price-scale mode. Fires `priceScaleChange`.
   *
   * Accepted: `"normal"`, `"logarithmic"` / `"log"`,
   * `"percentage"` / `"percent"`, `"indexed_to_100"` / `"indexedTo100"`.
   */
  set_price_scale_mode(mode: string): void;

  /**
   * Set price-scale top and bottom margins (fractions 0.0–1.0).
   * Default: top=0.2, bottom=0.1.
   */
  set_price_scale_margins(top: number, bottom: number): void;

  // ── Crosshair ──────────────────────────────────────────────────────────────

  /** Current crosshair mode string (`"normal"` or `"magnet_ohlc"`). */
  crosshair_mode(): string;

  /**
   * Current crosshair state as `[active, x, y, bar_index, price]`.
   * `active` is 1.0 if the crosshair is visible, else 0.0.
   */
  crosshair_state(): Float64Array;

  /**
   * Programmatically set the crosshair state — used by `ChartGroup` sync.
   */
  set_crosshair_state(
    active:    boolean,
    x:         number,
    y:         number,
    bar_index: number,
    price:     number,
    mode:      string,
  ): void;

  /** Set crosshair mode: `"normal"` or `"magnet_ohlc"`. */
  set_crosshair_mode(mode: string): void;

  /**
   * Set crosshair line style.
   * @param target — `"vert"`, `"horz"`, or `"both"`.
   * @param line_style — `"solid"` | `"dotted"` | `"dashed"` | `"large_dashed"` | `"sparse_dotted"`
   */
  set_crosshair_line_style(target: string, line_style: string): void;

  /**
   * Set crosshair line width (CSS px).
   * @param target — `"vert"`, `"horz"`, or `"both"`.
   */
  set_crosshair_line_width(target: string, width: number): void;

  /**
   * Set crosshair line visibility.
   * @param target — `"vert"`, `"horz"`, or `"both"`.
   */
  set_crosshair_line_visible(target: string, visible: boolean): void;

  /**
   * Set crosshair axis-label visibility.
   * @param target — `"vert"`, `"horz"`, or `"both"`.
   */
  set_crosshair_label_visible(target: string, visible: boolean): void;

  /**
   * Set crosshair line colour (RGBA 0–1).
   * @param target — `"vert"`, `"horz"`, or `"both"`.
   */
  set_crosshair_line_color(target: string, r: number, g: number, b: number, a: number): void;

  /**
   * Set crosshair axis-label background colour (RGBA 0–1).
   * @param target — `"vert"`, `"horz"`, or `"both"`.
   */
  set_crosshair_line_label_bg_color(target: string, r: number, g: number, b: number, a: number): void;

  /** Set crosshair label text colour (RGBA 0–1). */
  set_crosshair_label_text_color(r: number, g: number, b: number, a: number): void;

  // ── Last-price line ────────────────────────────────────────────────────────

  /** Set the animated last-price line style. */
  set_last_price_line_style(
    /** "solid" | "dotted" | "dashed" | "large_dashed" | "sparse_dotted" */
    line_style: string,
  ): void;

  /** Set the last-price line width (CSS px). */
  set_last_price_line_width(width: number): void;

  /** Show/hide the last-price line. */
  set_last_price_line_visible(visible: boolean): void;

  /** Show/hide the last-price axis label. */
  set_last_price_label_visible(visible: boolean): void;

  // ── Watermark ──────────────────────────────────────────────────────────────

  /** Set the centred watermark text. */
  set_watermark(text: string): void;

  /** Set the watermark text colour (RGBA 0–1). */
  set_watermark_color(r: number, g: number, b: number, a: number): void;

  // ── Candle / volume colors ─────────────────────────────────────────────────

  /**
   * Set bullish (up) candle body fill and wick/border colour (RGBA 0–1 each).
   * @deprecated Use `apply_options({ theme: { colors: { bullish } } })`.
   */
  set_bullish_color(
    fill_r: number, fill_g: number, fill_b: number, fill_a: number,
    wick_r: number, wick_g: number, wick_b: number, wick_a: number,
  ): void;

  /**
   * Set bearish (down) candle colours (RGBA 0–1 each).
   * @deprecated Use `apply_options({ theme: { colors: { bearish } } })`.
   */
  set_bearish_color(
    fill_r: number, fill_g: number, fill_b: number, fill_a: number,
    wick_r: number, wick_g: number, wick_b: number, wick_a: number,
  ): void;

  /** Set volume bar bullish/bearish colours (RGBA 0–1). */
  set_volume_colors(
    up_r: number,   up_g: number,   up_b: number,   up_a: number,
    down_r: number, down_g: number, down_b: number, down_a: number,
  ): void;

  /** Set bar width ratio (0.0–1.0, default 0.8). */
  set_bar_width_ratio(ratio: number): void;

  // ── Overlay series ─────────────────────────────────────────────────────────

  /**
   * Add a line series overlay.
   * @param line_style — `"solid"` | `"dotted"` | `"dashed"` | `"large_dashed"` | `"sparse_dotted"`
   * @returns Series ID.
   */
  add_line_series(
    color_r: number, color_g: number, color_b: number, color_a: number,
    line_width: number,
    line_style: string,
  ): number;

  /**
   * Add an area series overlay (line + gradient fill).
   * @returns Series ID.
   */
  add_area_series(
    line_color_r: number, line_color_g: number, line_color_b: number, line_color_a: number,
    top_color_r:  number, top_color_g:  number, top_color_b:  number, top_color_a:  number,
    bottom_color_r: number, bottom_color_g: number, bottom_color_b: number, bottom_color_a: number,
    line_width: number,
  ): number;

  /**
   * Add an OHLC bar series overlay.
   * @returns Series ID.
   */
  add_bar_series(
    up_color_r:   number, up_color_g:   number, up_color_b:   number, up_color_a:   number,
    down_color_r: number, down_color_g: number, down_color_b: number, down_color_a: number,
    open_visible: boolean,
    thin_bars:    boolean,
  ): number;

  /**
   * Add a baseline series (two-tone line + fill above/below a base value).
   * @returns Series ID.
   */
  add_baseline_series(
    base_value:    number,
    top_line_r:    number, top_line_g:    number, top_line_b:    number, top_line_a:    number,
    bottom_line_r: number, bottom_line_g: number, bottom_line_b: number, bottom_line_a: number,
    top_fill1_r:   number, top_fill1_g:   number, top_fill1_b:   number, top_fill1_a:   number,
    top_fill2_r:   number, top_fill2_g:   number, top_fill2_b:   number, top_fill2_a:   number,
    bottom_fill1_r:number, bottom_fill1_g:number, bottom_fill1_b:number, bottom_fill1_a:number,
    bottom_fill2_r:number, bottom_fill2_g:number, bottom_fill2_b:number, bottom_fill2_a:number,
    line_width:    number,
  ): number;

  /**
   * Add a histogram series overlay.
   * @param base — Y value from which bars extend (default 0).
   * @returns Series ID.
   */
  add_histogram_series(
    color_r: number, color_g: number, color_b: number, color_a: number,
    base: number,
  ): number;

  /** Remove a series by ID. */
  remove_series(id: number): boolean;

  /** Number of active overlay series. */
  series_count(): number;

  /** Show or hide a series. */
  set_series_visible(id: number, visible: boolean): void;

  /**
   * Set bulk data for a line/area/baseline series.
   * `values` and `timestamps` must be the same length.
   */
  set_series_data(
    id:         number,
    values:     Float32Array,
    timestamps: BigUint64Array,
  ): void;

  /**
   * Set bulk data for an OHLC bar series.
   * All arrays must be the same length.
   */
  set_bar_series_data(
    id:         number,
    timestamps: BigUint64Array,
    open:       Float32Array,
    high:       Float32Array,
    low:        Float32Array,
    close:      Float32Array,
  ): void;

  /**
   * Set bulk data for a histogram series.
   * `values` and `timestamps` must be the same length.
   * Per-bar colour arrays are optional — pass empty arrays to use the series default colour.
   */
  set_histogram_data(
    id:         number,
    values:     Float32Array,
    timestamps: BigUint64Array,
    colors_r:   Float32Array,
    colors_g:   Float32Array,
    colors_b:   Float32Array,
    colors_a:   Float32Array,
  ): void;

  /** Append a single point to a line/area/baseline series. */
  append_series_point(id: number, timestamp: bigint, value: number): void;

  /** LWC-style upsert for a line/area/baseline series. */
  upsert_series_point(id: number, timestamp: bigint, value: number): void;

  /** Update the last point in a line/area/baseline series. */
  update_last_series_point(id: number, timestamp: bigint, value: number): void;

  /** Append a point to an OHLC bar series. */
  append_bar_series_point(
    id: number, timestamp: bigint,
    open: number, high: number, low: number, close: number,
  ): void;

  /** LWC-style upsert for an OHLC bar series. */
  upsert_bar_series_point(
    id: number, timestamp: bigint,
    open: number, high: number, low: number, close: number,
  ): void;

  /** Update the last point in an OHLC bar series. */
  update_last_bar_series_point(
    id: number, timestamp: bigint,
    open: number, high: number, low: number, close: number,
  ): void;

  /** Append a point to a histogram series. */
  append_histogram_point(
    id: number, timestamp: bigint, value: number,
    color_r: number, color_g: number, color_b: number, color_a: number,
  ): void;

  /** LWC-style upsert for a histogram series. */
  upsert_histogram_point(
    id: number, timestamp: bigint, value: number,
    color_r: number, color_g: number, color_b: number, color_a: number,
  ): void;

  /** Update the last point in a histogram series. */
  update_last_histogram_point(
    id: number, timestamp: bigint, value: number,
    color_r: number, color_g: number, color_b: number, color_a: number,
  ): void;

  // ── Price lines ────────────────────────────────────────────────────────────

  /**
   * Add a horizontal price line.
   * @param draggable — Whether the user can drag the line to a new price.
   * @returns Price line ID.
   */
  create_price_line(
    price:      number,
    color_r:    number, color_g: number, color_b: number, color_a: number,
    line_width: number,
    line_style: string,
    draggable:  boolean,
  ): number;

  /** Remove a price line by ID. */
  remove_price_line(id: number): boolean;

  /** Update the price of an existing price line. */
  set_price_line_price(id: number, price: number): void;

  /** Set the label text for a price line (empty = show formatted price). */
  set_price_line_label(id: number, label: string): void;

  /** Show/hide a price line. */
  set_price_line_visible(id: number, visible: boolean): void;

  /** Number of active price lines. */
  price_line_count(): number;

  // ── Markers ────────────────────────────────────────────────────────────────

  /**
   * Add a marker to a series at the specified bar index.
   * @param shape    — `"arrow_up"` | `"arrow_down"` | `"circle"` | `"square"`
   * @param position — `"above_bar"` | `"below_bar"` | `"at_price"`
   * @param price    — Used only when `position === "at_price"`.
   * @returns Marker ID.
   */
  add_marker(
    series_id:  number,
    bar_index:  number,
    shape:      string,
    position:   string,
    price:      number,
    color_r:    number, color_g: number, color_b: number, color_a: number,
    size:       number,
    text:       string,
  ): number;

  /** Remove a specific marker from a series. */
  remove_marker(series_id: number, marker_id: number): boolean;

  /** Clear all markers for a series. */
  clear_markers(series_id: number): void;

  /** Clear all markers for all series. */
  clear_all_markers(): void;

  // ── Studies ────────────────────────────────────────────────────────────────

  /**
   * Create a study instance.
   * @param study_type — `"sma"`, `"ema"`, `"rsi"`, `"macd"`, `"stochastic"`,
   *   `"bollinger"`, `"atr"`, `"vwap"`
   * @returns Study ID, or 0 if the type is unknown.
   */
  create_study(study_type: string): number;

  /** Remove a study by ID. */
  remove_study(id: number): boolean;

  /** Number of active studies. */
  study_count(): number;

  /**
   * Get study output data.
   * @returns `{ timestamps: BigUint64Array, values: Float32Array }` or `null`.
   */
  get_study_output(
    id:           number,
    output_index: number,
  ): { timestamps: BigUint64Array; values: Float32Array } | null;

  /**
   * Set a study parameter (e.g. `"period"` for SMA/EMA, `"fast_period"` for MACD).
   * The study recalculates on the next render.
   */
  set_study_parameter(id: number, key: string, value: number): void;

  // ── Indicator sub-panes ────────────────────────────────────────────────────

  /**
   * Add an indicator sub-pane below the main chart.
   * The study must be created first with `create_study()`.
   * @returns Pane ID, or 0 on failure.
   */
  add_indicator_pane(
    study_id:       number,
    indicator_type: string,
    height_css:     number,
  ): number;

  /** Remove an indicator sub-pane by ID. */
  remove_indicator_pane(pane_id: number): boolean;

  /** Push updated study data to an indicator sub-pane. */
  update_indicator_pane(pane_id: number, study_id: number): void;

  /** Number of active indicator sub-panes. */
  indicator_pane_count(): number;

  /**
   * Drag a sub-pane separator to resize adjacent panes.
   * @param separator_idx — 0 = between main chart and first sub-pane.
   * @param delta_y        — Positive = down, negative = up.
   */
  drag_pane_separator(separator_idx: number, delta_y: number): void;

  // ── Sub-pane separator styling ─────────────────────────────────────────────

  /** Set sub-pane separator visible line thickness (CSS px). */
  set_subpane_separator_thickness(thickness_css: number): void;

  /** Set sub-pane separator drag hit-area thickness (CSS px). */
  set_subpane_separator_hit_area(hit_area_css: number): void;

  /** Set sub-pane separator colour (RGBA 0–1). */
  set_subpane_separator_color(r: number, g: number, b: number, a: number): void;

  /** Set sub-pane separator hover/active colour (RGBA 0–1). */
  set_subpane_separator_hover_color(r: number, g: number, b: number, a: number): void;

  // ── Drawings ───────────────────────────────────────────────────────────────

  /**
   * Activate a drawing tool.
   * @param tool — `"none"`, `"trend_line"`, `"horizontal_line"`, `"vertical_line"`,
   *   `"ray"`, `"rectangle"`, `"fibonacci"`, `"scale"`
   */
  set_drawing_tool(tool: string): void;

  /** Cancel an in-progress drawing creation. */
  cancel_drawing(): void;

  /** Delete the currently selected drawing. */
  remove_selected_drawing(): void;

  /** Remove all drawings. */
  clear_drawings(): void;

  /** Remove all scale (measurement) drawings. */
  remove_all_scale_drawings(): void;

  /** Number of drawings. */
  drawing_count(): number;

  // ── Font / axis styling ────────────────────────────────────────────────────

  /** Set the font family for axis labels (CSS font-family string). */
  set_font_family(family: string): void;

  /** Set the axis label font size (CSS px). */
  set_font_size(size: number): void;

  // ── Deprecated individual colour setters ──────────────────────────────────

  /** @deprecated Use `apply_options({ theme: { colors: { background } } })`. */
  set_background_color(r: number, g: number, b: number, a: number): void;

  /** @deprecated Use `apply_options({ theme: { colors: { grid } } })`. */
  set_grid_color(r: number, g: number, b: number, a: number): void;

  /** @deprecated Use `apply_options({ theme: { colors: { text } } })`. */
  set_axis_text_color(r: number, g: number, b: number, a: number): void;

  /** @deprecated Use `apply_options({ theme: { colors: { axis_border } } })`. */
  set_axis_border_color(r: number, g: number, b: number, a: number): void;

  /** @deprecated Use `apply_options({ theme: ... })`. */
  set_crosshair_color(r: number, g: number, b: number, a: number): void;

  /** @deprecated Use `apply_options({ theme: ... })`. */
  set_crosshair_label_bg_color(r: number, g: number, b: number, a: number): void;

  // ── Keyboard shortcuts ─────────────────────────────────────────────────────

  /**
   * Forward a keyboard event to the chart.
   *
   * Supported:
   * - `Delete` / `Backspace` — remove selected drawing
   * - `Escape` — cancel drawing, deselect all
   * - `←` / `→` — scroll one bar
   * - `↑` / `↓` — zoom price axis
   * - `Home` / `End` — scroll to first / last bar
   * - `+` / `=` — zoom in (time)
   * - `-` — zoom out
   * - `0` — reset zoom to fit all data
   *
   * @returns `true` if the key was handled (caller should `preventDefault()`).
   */
  on_key_down(key: string, ctrl: boolean, shift: boolean, alt: boolean): boolean;

  // ── Misc ───────────────────────────────────────────────────────────────────

  /** Name of the active renderer backend: `"webgpu"` or `"canvas2d"`. */
  renderer_name(): string;

  /**
   * Returns the list of available renderer backends in priority order.
   * @example `["webgpu", "canvas2d"]`
   */
  static get_supported_renderers(): string[];

  /** Activate the built-in demo mode (generates synthetic OHLCV data). */
  demo_mode(): void;
}

// ─────────────────────────────────────────────────────────────────────────────
// Module initialisation
// ─────────────────────────────────────────────────────────────────────────────

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

/**
 * Initialise the WASM module. Must be awaited once before using any API.
 *
 * @example
 * ```ts
 * import init, { RayCore } from './pkg/raycore_wasm.js';
 * await init();
 * const chart = await RayCore.create_chart('#chart', { theme: 'dark' });
 * ```
 */
export default function init(
  module_or_path?: InitInput | Promise<InitInput>,
): Promise<void>;

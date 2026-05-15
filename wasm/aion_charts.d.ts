/**
 * Aion_charts — Production-grade WASM charting library
 *
 * This is the hand-crafted TypeScript definitions file.
 * It supplements the auto-generated `wasm/pkg/aion_charts_wasm.d.ts` with:
 *   - Full typed event system (ChartEventMap, per-event payload types)
 *   - Properly typed `on<K>()`, `off<K>()`, `once<K>()` overloads
 *   - Typed CreateChartOptions / ThemeConfig interfaces
 *   - ChartGroup and ChartWorkspace classes
 *   - JSDoc on every public method
 *
 * Logical prices use JavaScript `number` / Rust `f64` end-to-end. Single-precision
 * render attributes are produced only inside the renderer projection seam.
 *
 * @example
 * ```ts
 * import init, { Aion_charts } from './pkg/aion_charts_wasm.js';
 * await init();
 *
 * const chartHost = document.getElementById('chart');
 * if (!chartHost) throw new Error("Missing chart element with id 'chart'");
 * const chart = await Aion_charts.create_chart(chartHost, {
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

export interface ChartGuardrails {
  /** Maximum indicator pane count. Use `0` or omit to disable the cap. */
  maxIndicatorPanes?: number;
  /** Maximum historical bar count accepted in a single load. Use `0` or omit to disable the cap. */
  maxBarsPerLoad?: number;
  /** Interval allowlist. Omit or pass an empty array to allow all intervals. */
  allowedIntervals?: string[];
  /** When true, blocks changing away from the current interval. */
  lockInterval?: boolean;
}

export interface WorkspaceGuardrails {
  /** Maximum split-pane count. Use `0` or omit to disable the cap. */
  maxPanes?: number;
}

export type ReplayEdgeBehavior = 'auto_pause' | 'live_continue' | 'auto_exit';

export interface ReplayOptions {
  speedBarsPerSecond?: number;
  edgeBehavior?: ReplayEdgeBehavior;
}

export type FootprintPalette = 'blue_red' | 'green_red';
export type FootprintGradientStyle = 'soft_glow' | 'strong_glow' | 'no_glow';

export interface FootprintOptionsPatch {
  display_mode?: 'bid_ask' | 'delta' | 'volume' | 'delta_profile' | 'volume_profile';
  tick_size?: number;
  palette?: FootprintPalette;
  gradient_style?: FootprintGradientStyle;
  poc_color?: RgbaColor | string;
  imbalance_ratio?: number;
  show_imbalances?: boolean;
  show_stacked_imbalances?: boolean;
  show_diagonal_imbalances?: boolean;
  show_poc?: boolean;
  show_value_area?: boolean;
  value_area_pct?: number;
  show_delta_bar?: boolean;
  show_volume_text?: boolean;
  show_unfinished_auction?: boolean;
  show_cumulative_delta?: boolean;
  font_size?: number;
  min_cell_height?: number;
  zoom_price_with_time?: boolean;
}

/**
 * Options for `Aion_charts.create_chart()` and `chart.apply_options()`.
 * All fields are optional. Omitted fields use defaults on creation
 * or keep current values when passed to `apply_options()`.
 */
export interface CreateChartOptions {
  /**
   * Renderer selection strategy.
   * `"auto"` and `"webgpu"` both prefer WebGPU and fall back to Canvas2D.
   * @default 'webgpu'
   */
  renderer?: 'auto' | 'webgpu' | 'canvas2d';
  /**
   * Theme preset or custom theme.
   * @default 'dark'
   */
  theme?: Theme;
  /**
   * Enable automatic requestAnimationFrame loop.
   * Set `false` to render manually via `chart.render()`.
   * @default true
   */
  autoRender?: boolean;
  /** Symbol string shown in the header (e.g. "BTCUSD"). */
  symbol?: string;
  /** Interval string (e.g. "1D", "4H", "15m"). */
  interval?: string;
  crosshair?: CrosshairOptions;
  priceScale?: PriceScaleOptions;
  /** Optional engine-side guardrails for panes, historical bar loads, and interval control. */
  guardrails?: ChartGuardrails;
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

export interface RendererFallbackEvent extends BaseChartEvent {
  type: 'rendererFallback';
  requested: 'auto' | 'webgpu' | 'canvas2d';
  active: 'canvas2d';
  reason: string;
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

export interface ExecutionMarkClickEvent extends BaseChartEvent {
  type: 'executionMarkClick';
  /** Unique ID of the execution mark. */
  id: string;
  /** Unix timestamp (ms) of the execution. */
  timestampMs: number;
  /** Execution price. */
  price: number;
  /** Side: "buy" or "sell". */
  side: 'buy' | 'sell';
  /** Role: "entry", "scale_in", "scale_out", or "exit". */
  role: 'entry' | 'scale_in' | 'scale_out' | 'exit';
  /** Execution quantity. */
  quantity: number;
  /** Optional group ID for related fills. */
  groupId: string | null;
}

export interface ExecutionClusterClickEvent extends BaseChartEvent {
  type: 'executionClusterClick';
  /** Leading mark ID for the rendered cluster. */
  leaderId: string;
  /** All execution mark IDs collapsed into the clicked cluster. */
  memberIds: string[];
}

export interface ExecutionMarkHoverEvent extends BaseChartEvent {
  type: 'executionMarkHover';
  /** Unique ID of the execution mark, or null when leaving. */
  id: string | null;
  /** Unix timestamp (ms) of the execution, if hovering. */
  timestampMs: number | null;
  /** Execution price, if hovering. */
  price: number | null;
  /** Side: "buy" or "sell", if hovering. */
  side: 'buy' | 'sell' | null;
  /** Role: "entry", "scale_in", "scale_out", or "exit", if hovering. */
  role: 'entry' | 'scale_in' | 'scale_out' | 'exit' | null;
  /** Execution quantity, if hovering. */
  quantity: number | null;
  /** Optional group ID, if hovering. */
  groupId: string | null;
}

export interface MarkerHoverEvent extends BaseChartEvent {
  type: 'markerHover';
  /** Series ID for the marker, or null when leaving. */
  seriesId: number | null;
  /** Marker ID within the series, or null when leaving. */
  markerId: number | null;
  /** Main bar index for the marker, or null when leaving. */
  barIndex: number | null;
  /** Marker timestamp in milliseconds, or null when unavailable/leaving. */
  timestamp: number | null;
  shape: 'arrowUp' | 'arrowDown' | 'circle' | 'square' | string | null;
  position: 'aboveBar' | 'belowBar' | 'atPrice' | string | null;
  zOrder: 'normal' | 'aboveSeries' | 'top' | string | null;
  text: string | null;
}

/** Map of event name → typed payload, used to type `on<K>()` overloads */
export interface ChartEventMap {
  crosshairMove:        CrosshairMoveEvent;
  click:                ClickEvent;
  visibleRangeChange:   VisibleRangeChangeEvent;
  symbolChange:         SymbolChangeEvent;
  intervalChange:       IntervalChangeEvent;
  chartTypeChange:      ChartTypeChangeEvent;
  priceScaleChange:     PriceScaleChangeEvent;
  resize:               ResizeEvent;
  rendererFallback:     RendererFallbackEvent;
  drawingCreated:       DrawingCreatedEvent;
  drawingSelected:      DrawingSelectedEvent;
  error:                ErrorEvent;
  executionClusterClick: ExecutionClusterClickEvent;
  executionMarkClick:   ExecutionMarkClickEvent;
  executionMarkHover:   ExecutionMarkHoverEvent;
  markerHover:          MarkerHoverEvent;
}

// ─────────────────────────────────────────────────────────────────────────────
// CSS variables emitted onto the container element
// ─────────────────────────────────────────────────────────────────────────────

/**
 * CSS custom properties Aion_charts writes to its container element.
 *
 * @example
 * ```css
 * .tooltip { background: var(--aion_charts-bg); color: var(--aion_charts-text); }
 * .signal  { color: var(--aion_charts-bullish); }
 * ```
 */
export interface Aion_chartsCssVariables {
  '--aion_charts-bg': string;
  '--aion_charts-text': string;
  '--aion_charts-bullish': string;
  '--aion_charts-bearish': string;
  '--aion_charts-grid': string;
  '--aion_charts-border': string;
  '--aion_charts-crosshair': string;
  '--aion_charts-crosshair-label-bg': string;
  '--aion_charts-crosshair-label-text': string;
  '--aion_charts-font-family': string;
  '--aion_charts-font-size': string;
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
 * Each pane gets a host `<div>` that you pass to `Aion_charts.create_chart()`.
 *
 * @example
 * ```ts
 * const ws = new ChartWorkspace('my-container-id');
 * const rootPaneId = ws.root_pane_id();
 * const hostId     = ws.pane_host_id(rootPaneId);
 * const chart      = await Aion_charts.create_chart(hostId, { autoRender: true });
 *
 * // Split vertically
 * const newPaneId = ws.split_active('vertical');
 * const newHostId = ws.pane_host_id(newPaneId);
 * const chart2    = await Aion_charts.create_chart(newHostId, { autoRender: true });
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
   * Pass this string to `Aion_charts.create_chart()`.
   */
  pane_host_id(pane_id: number): string;

  /** Array of all current pane IDs. */
  pane_ids(): number[];

  /** Whether the active pane can currently be split under the configured cap. */
  can_split_active(): boolean;

  /** Whether a specific pane can currently be split under the configured cap. */
  can_split_pane(pane_id: number): boolean;

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

  /** Register a callback fired whenever the active pane changes. */
  set_on_active_pane_change(callback: (pane_id: number) => void): void;

  /** Remove the active-pane change callback. */
  clear_on_active_pane_change(): void;

  /** Set the maximum pane count. Pass `0` to disable the cap. */
  set_max_panes(max_panes: number): void;

  /** Get the maximum pane count. Returns `0` when uncapped. */
  max_panes(): number;

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
// Aion_charts — main chart class
// ─────────────────────────────────────────────────────────────────────────────

export declare class Aion_charts {
  // ── Lifecycle ──────────────────────────────────────────────────────────────

  /**
   * Create and mount a new chart.
   *
   * @param container — `HTMLElement` or a string element ID (without `#`).
   * @param options   — Optional creation options.
   *
   * @example
   * ```ts
   * const chart = await Aion_charts.create_chart(
   *   document.getElementById('chart')!,
   *   { theme: 'dark', autoRender: true, symbol: 'BTCUSD' }
   * );
   * ```
   */
  static create_chart(
    container: HTMLElement | string,
    options?: CreateChartOptions,
  ): Promise<Aion_charts>;

  /**
   * @deprecated Use `Aion_charts.create_chart(container, { renderer })` instead.
   */
  static create_with(container_id: string, renderer: string): Promise<Aion_charts>;

  /**
   * Apply a partial options update at runtime.
   * Only provided fields are changed; everything else stays unchanged.
   * Note: `renderer` is create-time only and is ignored here.
   *
   * @example
   * ```ts
   * chart.apply_options({ theme: 'light' });
   * chart.apply_options({ crosshair: { mode: 'magnet_ohlc' } });
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

  // ── Replay ────────────────────────────────────────────────────────────────

  /** Enter or exit replay mode. */
  set_replay_mode(enabled: boolean): void;

  /** Whether replay mode is currently active. */
  replay_mode(): boolean;

  /** Start or pause replay playback. */
  set_replay_playing(playing: boolean): void;

  /** Whether replay playback is currently running. */
  replay_playing(): boolean;

  /** Step replay backward by exactly one bar. */
  replay_step_back(): void;

  /** Step replay forward by exactly one bar. */
  replay_step_forward(): void;

  /** Set replay right-edge cutoff (inclusive). */
  set_replay_cutoff_bar(index: number): void;

  /** Current replay cutoff index, or -1 when unavailable. */
  replay_cutoff_bar(): number;

  /** Set replay runtime options. */
  set_replay_options(options: ReplayOptions): void;

  /** Get replay runtime options. */
  replay_options(): ReplayOptions;

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
   * myTooltip.style.background = vars['--aion_charts-bg'];
   * ```
   */
  get_css_variables(): Aion_chartsCssVariables;

  /** Set the maximum indicator sub-pane count. Pass `0` to disable the cap. */
  set_max_indicator_panes(max_panes: number): void;

  /** Get the maximum indicator sub-pane count. Returns `0` when uncapped. */
  max_indicator_panes(): number;

  /** Whether another indicator pane can be created under the current cap. */
  can_add_indicator_pane(): boolean;

  /** Replace the interval allowlist. Pass an empty array to allow all intervals. */
  set_allowed_intervals(intervals: string[]): void;

  /** Clear the interval allowlist. */
  clear_allowed_intervals(): void;

  /** Get the interval allowlist. Returns an empty array when all intervals are allowed. */
  allowed_intervals(): string[];

  /** Whether a specific interval is permitted by the current guardrails. */
  is_interval_allowed(interval: string): boolean;

  /** Whether the chart can switch from the current interval to the requested one. */
  can_set_interval(interval: string): boolean;

  /** Lock or unlock interval changes away from the current interval. */
  set_interval_change_locked(locked: boolean): void;

  /** Whether interval changes are currently locked. */
  interval_change_locked(): boolean;

  /** Set the maximum historical bar count accepted in a single load. Pass `0` to disable the cap. */
  set_max_bars_per_load(max_bars: number): void;

  /** Get the maximum historical bar count accepted in a single load. Returns `0` when uncapped. */
  max_bars_per_load(): number;

  /** Whether a historical load of the given bar count would be accepted. */
  can_load_bar_count(bar_count: number): boolean;

  // ── Data loading ───────────────────────────────────────────────────────────

  /**
   * Load OHLCV data from columnar typed arrays.
   * All arrays must have the same length.
   * `timestamps` are Unix milliseconds as `BigUint64Array`.
   * Logical prices and volume use `Float64Array`.
   *
   * @example
   * ```ts
   * chart.set_data_arrays(opens, highs, lows, closes, volumes, timestamps);
   * ```
   */
  set_data_arrays(
    open:       Float64Array,
    high:       Float64Array,
    low:        Float64Array,
    close:      Float64Array,
    volume:     Float64Array,
    timestamps: BigUint64Array,
  ): void;

  /**
   * Canonical historical footprint initialization API.
   * Atomically loads OHLCV bars plus aligned footprint levels.
   * `level_offsets.length` must equal `bar_count + 1`.
   */
  set_data_with_footprint_arrays(
    open:          Float64Array,
    high:          Float64Array,
    low:           Float64Array,
    close:         Float64Array,
    volume:        Float64Array,
    timestamps:    BigUint64Array,
    level_offsets: Uint32Array,
    prices:        Float64Array,
    bid_volumes:   Float64Array,
    ask_volumes:   Float64Array,
  ): void;

  /**
   * Canonical historical footprint initialization API using JSON.
   * Accepts either an array of `{ timestamp, open, high, low, close, volume, levels }`
   * objects or `{ bars: [...] }`.
   */
  set_data_with_footprint_json(json: string): void;

  /**
   * Legacy compatibility method for patching a single footprint bar by bar index.
   */
  set_footprint_bar(
    bar_index: number,
    prices: Float64Array,
    bid_volumes: Float64Array,
    ask_volumes: Float64Array,
  ): void;

  /**
   * Legacy compatibility method for bulk footprint patch/update by explicit bar indices.
   */
  set_footprint_data_arrays(
    bar_indices: Uint32Array,
    level_offsets: Uint32Array,
    prices: Float64Array,
    bid_volumes: Float64Array,
    ask_volumes: Float64Array,
  ): void;

  /**
   * Legacy compatibility method for bulk footprint patch/update from JSON.
   */
  set_footprint_data_json(json: string): void;

  /**
   * Set footprint display mode.
   */
  set_footprint_display_mode(
    mode: 'bid_ask' | 'delta' | 'volume' | 'delta_profile' | 'volume_profile' | string,
  ): void;

  /**
   * Semantic footprint theming and behavior options.
   */
  set_footprint_options(json: string): void;

  /**
   * Set footprint tick size. Pass 0 for auto-detection.
   */
  set_footprint_tick_size(tick_size: number): void;

  /**
   * Enable or disable coupled X+Y zoom while footprint mode is active.
   */
  set_footprint_xy_zoom_enabled(enabled: boolean): void;

  /**
   * Return whether footprint XY zoom is enabled.
   */
  get_footprint_xy_zoom_enabled(): boolean;

  /**
   * Clear all footprint data.
   */
  clear_footprint_data(): void;

  /**
   * compatibility-style upsert: appends a new bar if the timestamp is newer than the last,
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
   * Canonical live footprint API.
   * Atomically appends/updates OHLCV plus footprint levels for the same logical bar.
   */
  upsert_bar_with_footprint(
    timestamp: bigint,
    open: number,
    high: number,
    low: number,
    close: number,
    volume: number,
    prices: Float64Array,
    bid_volumes: Float64Array,
    ask_volumes: Float64Array,
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
   * `"line"`, `"area"`, `"heikin_ashi"`, `"ha"`, `"baseline"`,
   * `"footprint"`, `"fp"`, `"order_flow"`.
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
   * Reset the main chart viewport.
   *
   * Modes:
   * - `"default"`: restore the recent-bars default view with a small right gap
   * - `"fit_all"`: show the full dataset with a small right gap
   *
   * Omitted or unknown modes fall back to `"default"`.
   * Fires `visibleRangeChange`.
   */
  reset_viewport(mode?: string): void;

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

  /**
   * Add an external price range that participates in automatic price scaling.
   */
  add_autoscale_contribution(min_price: number, max_price: number): number;

  /**
   * Remove a previously registered autoscale contribution.
   */
  remove_autoscale_contribution(id: number): boolean;

  /**
   * Remove all external autoscale contributions.
   */
  clear_autoscale_contributions(): void;

  /**
   * Enable or disable auto-scroll when new bars arrive during live streaming.
   *
   * When `true` (default) the viewport advances by 1 bar each time a new bar
   * is appended and the chart is already showing the latest data — identical
   * to the reference implementation's `shiftVisibleRangeOnNewBar` behaviour.
   *
   * When `false` the viewport is never moved by incoming data regardless of
   * scroll position, giving the user a fully static view during live updates.
   *
   * @example
   * ```ts
   * // Disable auto-scroll so the user can freely inspect history
   * // while live data accumulates off-screen to the right.
   * chart.set_auto_scroll(false);
   *
   * // Re-enable — the chart will scroll again on the next new bar.
   * chart.set_auto_scroll(true);
   * ```
   */
  set_auto_scroll(enabled: boolean): void;

  /** Return whether auto-scroll is currently enabled. */
  get_auto_scroll(): boolean;

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

  /** Show or hide volume bars in the main pane. */
  set_volume_visible(visible: boolean): void;

  /** Return whether volume bars are currently visible in the main pane. */
  get_volume_visible(): boolean;

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
    values:     Float64Array,
    timestamps: BigUint64Array,
  ): void;

  /**
   * Set bulk data for an OHLC bar series.
   * All arrays must be the same length.
   */
  set_bar_series_data(
    id:         number,
    timestamps: BigUint64Array,
    open:       Float64Array,
    high:       Float64Array,
    low:        Float64Array,
    close:      Float64Array,
  ): void;

  /**
   * Set bulk data for a histogram series.
   * `values` and `timestamps` must be the same length.
   * Per-bar colour arrays are optional — pass empty arrays to use the series default colour.
   */
  set_histogram_data(
    id:         number,
    values:     Float64Array,
    timestamps: BigUint64Array,
    colors_r:   Float32Array,
    colors_g:   Float32Array,
    colors_b:   Float32Array,
    colors_a:   Float32Array,
  ): void;

  /** Append a single point to a line/area/baseline series. */
  append_series_point(id: number, timestamp: bigint, value: number): void;

  /** compatibility-style upsert for a line/area/baseline series. */
  upsert_series_point(id: number, timestamp: bigint, value: number): void;

  /** Update the last point in a line/area/baseline series. */
  update_last_series_point(id: number, timestamp: bigint, value: number): void;

  /** Append a point to an OHLC bar series. */
  append_bar_series_point(
    id: number, timestamp: bigint,
    open: number, high: number, low: number, close: number,
  ): void;

  /** compatibility-style upsert for an OHLC bar series. */
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

  /** compatibility-style upsert for a histogram series. */
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
   * Pass the line color as RGBA components, the width and style explicitly,
   * and use `set_price_line_label()` if you want a custom label string.
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
   * @param bar_index — Bar index to attach the marker to.
   * @param shape     — `"arrow_up"` | `"arrow_down"` | `"circle"` | `"square"`
   * @param position  — `"above_bar"` | `"below_bar"` | `"at_price"`
   * @param price     — Used only when `position === "at_price"`.
   * @param size      — Marker size.
   * @param text      — Label text shown with the marker.
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

  /**
   * Add a marker anchored by timestamp instead of mutable bar index.
   * The timestamp is retained as the canonical render anchor across data reloads.
   */
  add_marker_at_time(
    series_id:  number,
    timestamp: bigint | number,
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

  /** Set global marker stacking order: `"normal"`, `"aboveSeries"`, or `"top"`. */
  set_marker_z_order(z_order: 'normal' | 'aboveSeries' | 'top' | string): void;

  /** Get the current global marker stacking order. */
  marker_z_order(): 'normal' | 'aboveSeries' | 'top' | string;

  /** Include marker visual size in automatic price scaling. Defaults to `true`. */
  set_marker_auto_scale(auto_scale: boolean): void;

  /** Whether marker visual size participates in automatic price scaling. */
  marker_auto_scale(): boolean;

  /**
   * Hit-test rendered series markers at pane CSS coordinates.
   * Returns `null` when no marker contains the point.
   */
  hit_test_marker(x_css: number, y_css: number): null | {
    seriesId: number;
    markerId: number;
    barIndex: number;
    timestamp: number | null;
    x: number;
    y: number;
    shape: 'arrowUp' | 'arrowDown' | 'circle' | 'square' | string;
    position: 'aboveBar' | 'belowBar' | 'atPrice' | string;
    zOrder: 'normal' | 'aboveSeries' | 'top' | string;
    text: string;
  };

  /**
   * Set multiple markers for a series at once.
   * The flat array stride is 9:
   * `[bar_index, shape_idx, position_idx, price, r, g, b, a, size, ...]`.
   * Throws if the series ID, bar index, stride, enum IDs, colours, price, or size are invalid.
   */
  set_markers(series_id: number, marker_data: Float64Array | number[]): void;

  /**
   * Set multiple timestamp-anchored markers for a series at once.
   * `timestamps` contains one timestamp per marker.
   * The flat marker-data stride is 8:
   * `[shape_idx, position_idx, price, r, g, b, a, size, ...]`.
   */
  set_time_markers(
    series_id: number,
    timestamps: BigUint64Array,
    marker_data: Float64Array | number[],
  ): void;

  // ── Execution Marks ──────────────────────────────────────────────────────
  //
  // First-class execution mark support for trade visualization.
  // Unlike generic markers, execution marks are timestamp-based (not bar-index-based)
  // and designed specifically for trading workflows.

  /**
   * Add a single execution mark to the chart.
   *
   * @param id           — Unique identifier for this execution
   * @param timestamp_ms — Unix timestamp in milliseconds when the execution occurred
   * @param price        — Execution price
   * @param quantity     — Execution quantity (positive)
   * @param side         — `"buy"` or `"sell"`
   * @param role         — `"entry"` | `"scale_in"` | `"scale_out"` | `"exit"`
   *
   * @example
   * ```ts
   * chart.add_execution_mark(
   *   'exec-1',
   *   1700000000000,
   *   45000.50,
   *   0.5,
   *   'buy',
   *   'entry'
   * );
   * ```
   */
  add_execution_mark(
    id:           string,
    timestamp_ms: bigint | number,
    price:        number,
    quantity:     number,
    side:         'buy' | 'sell' | string,
    role:         'entry' | 'scale_in' | 'scale_out' | 'exit' | string,
  ): void;

  /**
   * Add an execution mark with all optional fields.
   *
   * @param id           — Unique identifier
   * @param timestamp_ms — Unix timestamp in milliseconds
   * @param price        — Execution price
   * @param quantity     — Execution quantity
   * @param side         — `"buy"` or `"sell"`
   * @param role         — `"entry"` | `"scale_in"` | `"scale_out"` | `"exit"`
   * @param order_type   — e.g., `"market"`, `"limit"`, `"stop"` (empty string for none)
   * @param label        — Custom label text (empty string for default)
   * @param group_id     — Group ID for related fills (empty string for none)
   * @param color_*      — Custom color override RGBA (pass all zeros to use default)
   * @param realized_pnl — Realized P&L (pass NaN for none)
   */
  add_execution_mark_full(
    id:           string,
    timestamp_ms: bigint | number,
    price:        number,
    quantity:     number,
    side:         string,
    role:         string,
    order_type:   string,
    label:        string,
    group_id:     string,
    color_r:      number,
    color_g:      number,
    color_b:      number,
    color_a:      number,
    realized_pnl: number,
  ): void;

  /** Clear all execution marks. */
  clear_execution_marks(): void;

  /** Clear the selected execution mark. */
  clear_selected_execution_mark(): void;

  /** Number of execution marks. */
  execution_mark_count(): number;

  /**
   * Expand the currently rendered cluster for a given leader ID.
   *
   * @example
   * ```ts
   * const members = chart.expand_execution_cluster('exec-1');
   * ```
   */
  expand_execution_cluster(leader_id: string): string[];

  /** Get the chart-wide execution label mode. */
  get_execution_label_mode(): 'side' | 'role' | 'side_and_role' | string;

  /** Whether execution mark text labels are currently rendered. */
  get_execution_mark_text_visible(): boolean;

  /**
   * Serialize all execution marks to JSON.
   *
   * Returns the wrapped snapshot shape:
   * `{ "version": 1, "marks": [...] }`
   */
  get_execution_marks_json(): string;

  /** Whether realized P&L text is currently rendered for eligible execution marks. */
  get_execution_pnl_visible(): boolean;

  /** Get the currently selected execution mark ID, or `null` if none. */
  get_selected_execution_mark(): string | null;

  /**
   * Remove an execution mark by ID.
   * @returns `true` if found and removed.
   */
  remove_execution_mark(id: string): boolean;

  /** Set the CSS-pixel clustering threshold for dense execution marks. */
  set_execution_cluster_threshold_px(threshold_px: number): void;

  /**
   * Set the chart-wide execution label mode.
   *
   * Accepted values: `"side"`, `"role"`, `"side_and_role"` (case-insensitive).
   *
   * @example
   * ```ts
   * chart.set_execution_label_mode('side_and_role');
   * ```
   */
  set_execution_label_mode(mode: string): void;

  /** Show or hide execution mark text labels. */
  set_execution_mark_text_visible(visible: boolean): void;

  /**
   * Set multiple execution marks at once (replaces existing).
   *
   * @param ids       — Array of unique IDs (must match length of mark_data / 5)
   * @param mark_data — Flat array with stride 5: `[timestamp_ms, price, quantity, side_idx, role_idx, ...]`
   *                    where side_idx: 0=buy, 1=sell
   *                    and role_idx: 0=entry, 1=scale_in, 2=scale_out, 3=exit
   *
   * @example
   * ```ts
   * chart.set_execution_marks(
   *   ['exec-1', 'exec-2'],
   *   new Float64Array([
   *     1700000000000, 45000.50, 0.5, 0, 0,  // buy entry
   *     1700001000000, 46000.00, 0.5, 1, 3,  // sell exit
   *   ])
   * );
   * ```
   */
  set_execution_marks(ids: string[], mark_data: Float64Array | number[]): void;

  /**
   * Set execution marks from a JSON string.
   *
   * Accepts either the wrapped snapshot format or the legacy bare-array format.
   *
   * Preferred wrapped format:
   * ```json
   * {
   *   "version": 1,
   *   "marks": [
   *     {
   *       "id": "exec-1",
   *       "timestamp_ms": 1234567890000,
   *       "price": 100.5,
   *       "quantity": 1.0,
   *       "side": "buy",
   *       "role": "entry",
   *       "order_type": "market",
   *       "label": "Entry Long",
   *       "group_id": "trade-1",
   *       "color": [0.2, 0.8, 0.4, 1.0],
   *       "realized_pnl": 0.0
   *     }
   *   ]
   * }
   * ```
   *
   * @example
   * ```ts
   * chart.set_execution_marks_json(JSON.stringify({
   *   version: 1,
   *   marks: [
   *     { id: 'e1', timestamp_ms: Date.now(), price: 45000, quantity: 0.1, side: 'buy', role: 'entry' },
   *     { id: 'e2', timestamp_ms: Date.now() + 60000, price: 46000, quantity: 0.1, side: 'sell', role: 'exit', realized_pnl: 150.0 },
   *   ],
   * }));
   * ```
   */
  set_execution_marks_json(json: string): void;

  /** Show or hide realized P&L text for eligible execution marks. */
  set_execution_pnl_visible(visible: boolean): void;

  /** Set the selected execution mark ID (shows selected-trade locators). */
  set_selected_execution_mark(mark_id: string | null): void;

  /**
   * Convert a timestamp (milliseconds) to a bar index.
   * @returns The bar index, or -1 if the timestamp is before all bars.
   */
  timestamp_to_bar_index(timestamp_ms: bigint | number): number;

  /**
   * Convert a bar index to a timestamp (milliseconds).
   * @returns The timestamp, or 0 if the bar index is out of bounds.
   */
  bar_index_to_timestamp(bar_index: number): bigint;

  /**
   * Project a timestamp/price coordinate into the current pane CSS coordinate space.
   * Returns `visible: false` with `NaN` coordinates when the timestamp is before loaded data.
   */
  project_point(timestamp_ms: bigint | number, price: number): { x: number; y: number; visible: boolean };

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
   * @returns `{ timestamps: BigUint64Array, values: Float64Array }` or `null`.
   */
  get_study_output(
    id:           number,
    output_index: number,
  ): { timestamps: BigUint64Array; values: Float64Array } | null;

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
   *   `"ray"`, `"rectangle"`, `"fibonacci"`, `"scale"`, `"brush"`
   */
  set_drawing_tool(tool: string): void;

  /** Cancel an in-progress drawing creation. */
  cancel_drawing(): void;

  /** Complete an in-progress drawing creation for tools that require explicit completion. */
  complete_drawing(): boolean;

  /** Deselect all drawings. */
  deselect_drawings(): void;

  /** Delete the currently selected drawing. */
  remove_selected_drawing(): void;

  /** Remove all drawings. */
  clear_drawings(): void;

  /** Remove all scale (measurement) drawings. */
  remove_all_scale_drawings(): void;

  /** Number of drawings. */
  drawing_count(): number;

  // ── Chart persistence (state + drawings) ──────────────────────────────────

  /**
   * Export a full chart snapshot for persistence.
   *
   * Includes chart options/styles, viewport, indicator pane layout, and drawings.
   * Use `layout_id` to tag snapshots per workspace/layout in your storage.
   */
  export_persistence_state(layout_id?: string | null): string;

  /**
   * Restore a full chart snapshot produced by `export_persistence_state()`.
   *
   * Reapplies options/styles, viewport, pane layout, then drawings.
   */
  import_persistence_state(json: string): void;

  /**
   * Export all drawings (main pane + indicator sub-panes) as a JSON snapshot.
   * Use this string to persist drawings externally.
   */
  export_drawings(): string;

  /**
   * Restore drawings from a JSON snapshot created by `export_drawings()`.
   * Existing drawings are replaced atomically (on validation failure, current drawings stay intact).
   */
  import_drawings(json: string): void;

  // ── Font / axis styling ────────────────────────────────────────────────────

  /** Set the font family for axis labels (CSS font-family string). */
  set_font_family(family: string): void;

  /** Set the axis label font size (CSS px). */
  set_font_size(size: number): void;

  // ── Misc ───────────────────────────────────────────────────────────────────

  /** Name of the active renderer backend: `"webgpu"` or `"canvas2d"`. */
  renderer_name(): string;

  /**
   * Returns the list of available renderer backends.
   * @example `["webgpu", "canvas2d"]` or `["canvas2d"]`
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
 * import init, { Aion_charts } from './pkg/aion_charts_wasm.js';
 * await init();
 * const chart = await Aion_charts.create_chart(document.getElementById('chart')!, { theme: 'dark' });
 * ```
 */
export default function init(
  module_or_path?: InitInput | Promise<InitInput>,
): Promise<void>;

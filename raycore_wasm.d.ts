/* tslint:disable */
/* eslint-disable */

export class RayCore {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add a new area series overlay. Returns the series ID.
     *
     * `line_color_*`: RGBA for the line stroke.
     * `top_color_*`: RGBA for the fill at the line (top of gradient).
     * `bottom_color_*`: RGBA for the fill at the base (bottom of gradient).
     */
    add_area_series(line_color_r: number, line_color_g: number, line_color_b: number, line_color_a: number, top_color_r: number, top_color_g: number, top_color_b: number, top_color_a: number, bottom_color_r: number, bottom_color_g: number, bottom_color_b: number, bottom_color_a: number, line_width: number): number;
    /**
     * Add a new bar (OHLC) series overlay. Returns the series ID.
     *
     * `up_color_*`: RGBA for bullish bars (close >= open).
     * `down_color_*`: RGBA for bearish bars (close < open).
     * `open_visible`: whether to show the open tick.
     * `thin_bars`: use 1px stems (like LWC thinBars option).
     */
    add_bar_series(up_color_r: number, up_color_g: number, up_color_b: number, up_color_a: number, down_color_r: number, down_color_g: number, down_color_b: number, down_color_a: number, open_visible: boolean, thin_bars: boolean): number;
    /**
     * Add a new baseline series overlay. Returns the series ID.
     *
     * A baseline series renders a line with two-tone fill above/below a base value.
     * Above the base: `top_line_color` line + `top_fill_color1`→`top_fill_color2` gradient.
     * Below the base: `bottom_line_color` line + `bottom_fill_color1`→`bottom_fill_color2` gradient.
     */
    add_baseline_series(base_value: number, top_line_r: number, top_line_g: number, top_line_b: number, top_line_a: number, bottom_line_r: number, bottom_line_g: number, bottom_line_b: number, bottom_line_a: number, top_fill1_r: number, top_fill1_g: number, top_fill1_b: number, top_fill1_a: number, top_fill2_r: number, top_fill2_g: number, top_fill2_b: number, top_fill2_a: number, bottom_fill1_r: number, bottom_fill1_g: number, bottom_fill1_b: number, bottom_fill1_a: number, bottom_fill2_r: number, bottom_fill2_g: number, bottom_fill2_b: number, bottom_fill2_a: number, line_width: number): number;
    /**
     * Add a new histogram series overlay. Returns the series ID.
     *
     * `color_*`: RGBA for the default bar color.
     * `base`: the base value (bars extend from base to data value).
     */
    add_histogram_series(color_r: number, color_g: number, color_b: number, color_a: number, base: number): number;
    /**
     * Create a new indicator sub-pane below the main chart.
     * Returns the pane ID. The indicator type should be one of: "rsi", "stochastic", "atr".
     * The study must already be created with `create_study()`.
     */
    add_indicator_pane(study_id: number, indicator_type: string, height_css: number): number;
    /**
     * Add a new line series overlay. Returns the series ID.
     *
     * Default color is TradingView blue (#2962FF). Use RGBA [0.0–1.0].
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     */
    add_line_series(color_r: number, color_g: number, color_b: number, color_a: number, line_width: number, line_style: string): number;
    /**
     * Add a marker to a series at the specified bar index.
     *
     * `shape`: "arrow_up", "arrow_down", "circle", "square"
     * `position`: "above_bar", "below_bar", "at_price"
     * `price`: Used only when position is "at_price"
     *
     * Returns the marker ID.
     */
    add_marker(series_id: number, bar_index: number, shape: string, position: string, price: number, color_r: number, color_g: number, color_b: number, color_a: number, size: number, text: string): number;
    /**
     * Append a single bar to the data array. Used for real-time streaming.
     */
    append_bar(timestamp: bigint, open: number, high: number, low: number, close: number, volume: number): void;
    /**
     * Cancel the drawing currently being created (e.g. on Escape key).
     */
    cancel_drawing(): void;
    /**
     * Clear all markers for all series.
     */
    clear_all_markers(): void;
    /**
     * Remove all drawings.
     */
    clear_drawings(): void;
    /**
     * Clear all markers for a series.
     */
    clear_markers(series_id: number): void;
    /**
     * Create a new RayCore instance inside a container div.
     */
    static create(container_id: string): Promise<RayCore>;
    /**
     * Create a new price line at the specified price level. Returns the price line ID.
     *
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     */
    create_price_line(price: number, color_r: number, color_g: number, color_b: number, color_a: number, line_width: number, line_style: string, draggable: boolean): number;
    /**
     * Create a new study instance. Returns the study ID, or 0 if the type is unknown.
     *
     * Supported types: "sma", "ema", "rsi", "macd".
     */
    create_study(study_type: string): number;
    /**
     * Create with a specific renderer backend ("webgpu" or "canvas2d").
     */
    static create_with(container_id: string, renderer: string): Promise<RayCore>;
    demo_mode(): void;
    /**
     * Dispose: remove all event listeners, disconnect resize observer, and clean up resources.
     *
     * IMPORTANT: Call this when destroying the chart to prevent memory leaks.
     * Event listeners attached to DOM elements will keep the closures alive
     * even after RayCore is dropped, unless explicitly removed.
     */
    dispose(): void;
    /**
     * Drag a separator to resize adjacent panes.
     * `separator_idx` is 0 for separator between main and first subpane.
     * `delta_y` is positive for moving down, negative for up.
     * This uses the PaneManager's coordinated height algorithm.
     */
    drag_pane_separator(separator_idx: number, delta_y: number): void;
    /**
     * Get the number of drawings.
     */
    drawing_count(): number;
    /**
     * Get all available chart types as a comma-separated string.
     */
    static get_available_chart_types(): string;
    /**
     * Get the current chart type as a string.
     */
    get_chart_type(): string;
    /**
     * Get study output data as a JS object { timestamps: BigUint64Array, values: Float32Array }.
     * Returns null if the study or output index doesn't exist.
     */
    get_study_output(id: number, output_index: number): any;
    static get_supported_renderers(): Array<any>;
    /**
     * Get the number of indicator sub-panes.
     */
    indicator_pane_count(): number;
    /**
     * Handle keyboard events. Returns true if the key was handled.
     *
     * Supported shortcuts:
     * - Delete / Backspace: Remove selected drawing
     * - Escape: Cancel drawing creation, deselect all
     * - Arrow Left/Right: Scroll chart by one bar
     * - Arrow Up/Down: Zoom price axis in/out
     * - Home: Scroll to first bar
     * - End: Scroll to last bar
     * - +/=: Zoom in (time axis)
     * - -: Zoom out (time axis)
     * - 0: Reset zoom to fit all data
     */
    on_key_down(key: string, ctrl: boolean, shift: boolean, _alt: boolean): boolean;
    /**
     * Get the number of price lines.
     */
    price_line_count(): number;
    /**
     * Remove all scale (measurement) drawings.
     */
    remove_all_scale_drawings(): void;
    /**
     * Remove an indicator sub-pane by ID.
     */
    remove_indicator_pane(pane_id: number): boolean;
    /**
     * Remove a specific marker from a series.
     */
    remove_marker(series_id: number, marker_id: number): boolean;
    /**
     * Remove a price line by ID.
     */
    remove_price_line(id: number): boolean;
    /**
     * Remove the currently selected drawing (e.g. on Delete key).
     */
    remove_selected_drawing(): void;
    /**
     * Remove a series by ID.
     */
    remove_series(id: number): boolean;
    /**
     * Remove a study by ID.
     */
    remove_study(id: number): boolean;
    /**
     * Render one frame. Call from requestAnimationFrame.
     */
    render(): void;
    renderer_name(): string;
    /**
     * Get the number of overlay series.
     */
    series_count(): number;
    /**
     * Set the axis border color (RGBA, 0.0-1.0).
     */
    set_axis_border_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set the axis text color (RGBA, 0.0-1.0).
     */
    set_axis_text_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set the chart background color (RGBA, 0.0-1.0).
     */
    set_background_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set data for a bar (OHLC) series.
     * All arrays must be the same length.
     */
    set_bar_series_data(id: number, timestamps: BigUint64Array, open: Float32Array, high: Float32Array, low: Float32Array, close: Float32Array): void;
    /**
     * Set the bar width ratio (0.0-1.0, default 0.8).
     */
    set_bar_width_ratio(ratio: number): void;
    /**
     * Set bearish (down) candle colors: body fill and wick/border.
     */
    set_bearish_color(fill_r: number, fill_g: number, fill_b: number, fill_a: number, wick_r: number, wick_g: number, wick_b: number, wick_a: number): void;
    /**
     * Set bullish (up) candle colors: body fill and wick/border.
     */
    set_bullish_color(fill_r: number, fill_g: number, fill_b: number, fill_a: number, wick_r: number, wick_g: number, wick_b: number, wick_a: number): void;
    /**
     * Set the main chart type.
     *
     * Accepted values: "candlestick", "candles", "ohlc", "bars", "line", "area",
     * "heikin_ashi", "ha", "baseline".
     */
    set_chart_type(chart_type: string): void;
    /**
     * Set the crosshair line color (RGBA, 0.0-1.0).
     */
    set_crosshair_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set the crosshair label background color (RGBA, 0.0-1.0).
     */
    set_crosshair_label_bg_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set the crosshair label text color (RGBA, 0.0-1.0).
     */
    set_crosshair_label_text_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set crosshair axis-label visibility.
     * `target`: "vert", "horz", or "both".
     */
    set_crosshair_label_visible(target: string, visible: boolean): void;
    /**
     * Set crosshair line color.
     * `target`: "vert", "horz", or "both".
     */
    set_crosshair_line_color(target: string, r: number, g: number, b: number, a: number): void;
    /**
     * Set crosshair label background color.
     * `target`: "vert", "horz", or "both".
     */
    set_crosshair_line_label_bg_color(target: string, r: number, g: number, b: number, a: number): void;
    /**
     * Set crosshair line style.
     * `target`: "vert", "horz", or "both".
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     */
    set_crosshair_line_style(target: string, line_style: string): void;
    /**
     * Set crosshair line visibility.
     * `target`: "vert", "horz", or "both".
     */
    set_crosshair_line_visible(target: string, visible: boolean): void;
    /**
     * Set crosshair line width in CSS pixels.
     * `target`: "vert", "horz", or "both".
     */
    set_crosshair_line_width(target: string, width: number): void;
    /**
     * Set crosshair mode: "normal", "magnet", or "magnet_ohlc".
     */
    set_crosshair_mode(mode: string): void;
    set_data(data: Float32Array): void;
    set_data_arrays(open: Float32Array, high: Float32Array, low: Float32Array, close: Float32Array, volume: Float32Array, timestamps: BigUint64Array): void;
    /**
     * Set active drawing tool: "none", "trend_line", "rectangle", "fibonacci", "scale".
     */
    set_drawing_tool(tool: string): void;
    /**
     * Set the font family for axis labels.
     */
    set_font_family(family: string): void;
    /**
     * Set the font size for axis labels (in CSS pixels).
     */
    set_font_size(size: number): void;
    /**
     * Set the grid line color (RGBA, 0.0-1.0).
     */
    set_grid_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set data for a histogram series. `values` and `timestamps` must be same length.
     * Per-bar colors are optional — pass empty arrays to use the series default color.
     */
    set_histogram_data(id: number, values: Float32Array, timestamps: BigUint64Array, colors_r: Float32Array, colors_g: Float32Array, colors_b: Float32Array, colors_a: Float32Array): void;
    /**
     * Set live last-price label visibility on the Y axis.
     */
    set_last_price_label_visible(visible: boolean): void;
    /**
     * Set live last-price line style.
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     */
    set_last_price_line_style(line_style: string): void;
    /**
     * Set live last-price line visibility.
     */
    set_last_price_line_visible(visible: boolean): void;
    /**
     * Set live last-price line width in CSS pixels.
     */
    set_last_price_line_width(width: number): void;
    /**
     * Set multiple markers for a series at once (replaces existing).
     * `marker_data` is a flat array: [bar_index, shape_idx, position_idx, price, r, g, b, a, size, ...]
     * where shape_idx: 0=arrowUp, 1=arrowDown, 2=circle, 3=square
     * and position_idx: 0=aboveBar, 1=belowBar, 2=atPrice
     */
    set_markers(series_id: number, marker_data: Float64Array): void;
    /**
     * Set the label text of a price line. Empty string uses formatted price.
     */
    set_price_line_label(id: number, label: string): void;
    /**
     * Update the price of an existing price line.
     */
    set_price_line_price(id: number, price: number): void;
    /**
     * Set whether a price line is visible.
     */
    set_price_line_visible(id: number, visible: boolean): void;
    /**
     * Set the price scale margins (top and bottom as fractions 0.0-1.0).
     * Default is 0.2 top, 0.1 bottom.
     */
    set_price_scale_margins(top: number, bottom: number): void;
    /**
     * Set the price scale mode.
     *
     * Accepted values: "normal", "logarithmic" (or "log"), "percentage" (or "percent"),
     * "indexed_to_100" (or "indexedTo100", "indexed").
     */
    set_price_scale_mode(mode: string): void;
    /**
     * Set data for a line series. `values` and `timestamps` must be same length.
     */
    set_series_data(id: number, values: Float32Array, timestamps: BigUint64Array): void;
    /**
     * Show or hide a series.
     */
    set_series_visible(id: number, visible: boolean): void;
    /**
     * Set a study parameter (e.g., "period" for SMA/EMA, "fast_period" for MACD).
     * The study will be recalculated on the next render.
     */
    set_study_parameter(id: number, key: string, value: number): void;
    /**
     * Set volume bar colors: bullish and bearish.
     */
    set_volume_colors(up_r: number, up_g: number, up_b: number, up_a: number, down_r: number, down_g: number, down_b: number, down_a: number): void;
    /**
     * Set watermark text displayed centered on the chart pane.
     */
    set_watermark(text: string): void;
    /**
     * Set the watermark text color (RGBA, 0.0-1.0).
     */
    set_watermark_color(r: number, g: number, b: number, a: number): void;
    /**
     * Get the number of studies.
     */
    study_count(): number;
    /**
     * Update indicator sub-pane data from a study.
     */
    update_indicator_pane(pane_id: number, study_id: number): void;
    /**
     * Update the last bar in the data array. Used for real-time tick updates.
     */
    update_last_bar(timestamp: bigint, open: number, high: number, low: number, close: number, volume: number): void;
    visible_range(): Float64Array;
    zoom_to_range(start: bigint, end: bigint): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_raycore_free: (a: number, b: number) => void;
    readonly raycore_add_area_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => number;
    readonly raycore_add_bar_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => number;
    readonly raycore_add_baseline_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number, v: number, w: number, x: number, y: number, z: number, a1: number) => number;
    readonly raycore_add_histogram_series: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly raycore_add_indicator_pane: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly raycore_add_line_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly raycore_add_marker: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => number;
    readonly raycore_append_bar: (a: number, b: bigint, c: number, d: number, e: number, f: number, g: number) => void;
    readonly raycore_cancel_drawing: (a: number) => void;
    readonly raycore_clear_all_markers: (a: number) => void;
    readonly raycore_clear_drawings: (a: number) => void;
    readonly raycore_clear_markers: (a: number, b: number) => void;
    readonly raycore_create: (a: number, b: number) => number;
    readonly raycore_create_price_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly raycore_create_study: (a: number, b: number, c: number) => number;
    readonly raycore_create_with: (a: number, b: number, c: number, d: number) => number;
    readonly raycore_demo_mode: (a: number) => void;
    readonly raycore_dispose: (a: number) => void;
    readonly raycore_drag_pane_separator: (a: number, b: number, c: number) => void;
    readonly raycore_drawing_count: (a: number) => number;
    readonly raycore_get_available_chart_types: (a: number) => void;
    readonly raycore_get_chart_type: (a: number, b: number) => void;
    readonly raycore_get_study_output: (a: number, b: number, c: number) => number;
    readonly raycore_get_supported_renderers: () => number;
    readonly raycore_indicator_pane_count: (a: number) => number;
    readonly raycore_on_key_down: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly raycore_price_line_count: (a: number) => number;
    readonly raycore_remove_all_scale_drawings: (a: number) => void;
    readonly raycore_remove_indicator_pane: (a: number, b: number) => number;
    readonly raycore_remove_marker: (a: number, b: number, c: number) => number;
    readonly raycore_remove_price_line: (a: number, b: number) => number;
    readonly raycore_remove_selected_drawing: (a: number) => void;
    readonly raycore_remove_series: (a: number, b: number) => number;
    readonly raycore_remove_study: (a: number, b: number) => number;
    readonly raycore_render: (a: number) => void;
    readonly raycore_renderer_name: (a: number, b: number) => void;
    readonly raycore_series_count: (a: number) => number;
    readonly raycore_set_axis_border_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_axis_text_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_background_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_bar_series_data: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly raycore_set_bar_width_ratio: (a: number, b: number) => void;
    readonly raycore_set_bearish_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly raycore_set_bullish_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly raycore_set_chart_type: (a: number, b: number, c: number) => void;
    readonly raycore_set_crosshair_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_crosshair_label_bg_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_crosshair_label_text_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_crosshair_label_visible: (a: number, b: number, c: number, d: number) => void;
    readonly raycore_set_crosshair_line_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly raycore_set_crosshair_line_label_bg_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly raycore_set_crosshair_line_style: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_crosshair_line_visible: (a: number, b: number, c: number, d: number) => void;
    readonly raycore_set_crosshair_line_width: (a: number, b: number, c: number, d: number) => void;
    readonly raycore_set_crosshair_mode: (a: number, b: number, c: number) => void;
    readonly raycore_set_data: (a: number, b: number, c: number) => void;
    readonly raycore_set_data_arrays: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => void;
    readonly raycore_set_drawing_tool: (a: number, b: number, c: number) => void;
    readonly raycore_set_font_family: (a: number, b: number, c: number) => void;
    readonly raycore_set_font_size: (a: number, b: number) => void;
    readonly raycore_set_grid_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_histogram_data: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => void;
    readonly raycore_set_last_price_label_visible: (a: number, b: number) => void;
    readonly raycore_set_last_price_line_style: (a: number, b: number, c: number) => void;
    readonly raycore_set_last_price_line_visible: (a: number, b: number) => void;
    readonly raycore_set_last_price_line_width: (a: number, b: number) => void;
    readonly raycore_set_markers: (a: number, b: number, c: number, d: number) => void;
    readonly raycore_set_price_line_label: (a: number, b: number, c: number, d: number) => void;
    readonly raycore_set_price_line_price: (a: number, b: number, c: number) => void;
    readonly raycore_set_price_line_visible: (a: number, b: number, c: number) => void;
    readonly raycore_set_price_scale_margins: (a: number, b: number, c: number) => void;
    readonly raycore_set_price_scale_mode: (a: number, b: number, c: number) => void;
    readonly raycore_set_series_data: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly raycore_set_series_visible: (a: number, b: number, c: number) => void;
    readonly raycore_set_study_parameter: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_set_volume_colors: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly raycore_set_watermark: (a: number, b: number, c: number) => void;
    readonly raycore_set_watermark_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly raycore_study_count: (a: number) => number;
    readonly raycore_update_indicator_pane: (a: number, b: number, c: number) => void;
    readonly raycore_update_last_bar: (a: number, b: bigint, c: number, d: number, e: number, f: number, g: number) => void;
    readonly raycore_visible_range: (a: number, b: number) => void;
    readonly raycore_zoom_to_range: (a: number, b: bigint, c: bigint) => void;
    readonly __wasm_bindgen_func_elem_517: (a: number, b: number) => void;
    readonly __wasm_bindgen_func_elem_1170: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_518: (a: number, b: number, c: number) => void;
    readonly __wasm_bindgen_func_elem_521: (a: number, b: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

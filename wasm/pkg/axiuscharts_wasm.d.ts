/* tslint:disable */
/* eslint-disable */

export class AxiusCharts {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Get the number of active (pending/working) order lines.
     */
    active_order_line_count(): number;
    /**
     * Add a new area series overlay. Returns the series ID.
     *
     * `line_color_*`: RGBA for the line stroke.
     * `top_color_*`: RGBA for the fill at the line (top of gradient).
     * `bottom_color_*`: RGBA for the fill at the base (bottom of gradient).
     */
    add_area_series(line_color_r: number, line_color_g: number, line_color_b: number, line_color_a: number, top_color_r: number, top_color_g: number, top_color_b: number, top_color_a: number, bottom_color_r: number, bottom_color_g: number, bottom_color_b: number, bottom_color_a: number, line_width: number): number;
    /**
     * Add an external price range that participates in automatic price scaling.
     */
    add_autoscale_contribution(min_price: number, max_price: number): number;
    /**
     * Add a new bar (OHLC) series overlay. Returns the series ID.
     *
     * `up_color_*`: RGBA for bullish bars (close >= open).
     * `down_color_*`: RGBA for bearish bars (close < open).
     * `open_visible`: whether to show the open tick.
     * `thin_bars`: use 1px stems (like reference implementation thinBars option).
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
     * Add a single execution mark to the chart.
     *
     * `side`: "buy" or "sell"
     * `role`: "entry", "scale_in", "scale_out", or "exit"
     *
     * Returns the execution mark ID.
     */
    add_execution_mark(id: string, timestamp_ms: bigint, price: number, quantity: number, side: string, role: string): void;
    /**
     * Add an execution mark with all optional fields.
     *
     * `side`: "buy" or "sell"
     * `role`: "entry", "scale_in", "scale_out", or "exit"
     * `order_type`: e.g., "market", "limit", "stop" (empty string for none)
     * `label`: custom label text (empty string for default)
     * `group_id`: group ID for related fills (empty string for none)
     * `color_*`: custom color override (pass all zeros to use default)
     * `realized_pnl`: realized P&L (pass NaN for none)
     */
    add_execution_mark_full(id: string, timestamp_ms: bigint, price: number, quantity: number, side: string, role: string, order_type: string, label: string, group_id: string, color_r: number, color_g: number, color_b: number, color_a: number, realized_pnl: number): void;
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
     * Default color is default blue (#2962FF). Use RGBA [0.0–1.0].
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
     * Add a marker anchored by timestamp instead of mutable bar index.
     *
     * The timestamp is kept as the canonical render anchor. The resolved bar
     * index is only used as a fallback and for above/below bar price placement.
     */
    add_marker_at_time(series_id: number, timestamp: bigint, shape: string, position: string, price: number, color_r: number, color_g: number, color_b: number, color_a: number, size: number, text: string): number;
    /**
     * Get the allowed interval list. Returns an empty array when all intervals are allowed.
     */
    allowed_intervals(): Array<any>;
    /**
     * Append a single bar to the data array. Used for real-time streaming.
     */
    append_bar(timestamp: bigint, open: number, high: number, low: number, close: number, volume: number): void;
    /**
     * Append a single point to a bar (OHLC) overlay series.
     */
    append_bar_series_point(id: number, timestamp: bigint, open: number, high: number, low: number, close: number): void;
    /**
     * Append a single point to a histogram overlay series.
     */
    append_histogram_point(id: number, timestamp: bigint, value: number, color_r: number, color_g: number, color_b: number, color_a: number): void;
    /**
     * Append a single point to a line/area/baseline overlay series.
     */
    append_series_point(id: number, timestamp: bigint, value: number): void;
    /**
     * Apply partial options update at runtime.
     *
     * Accepts the same options shape as `create_chart()`. Only provided
     * fields are updated; omitted fields keep their current values.
     */
    apply_options(options: any): void;
    /**
     * Convert a bar index to a timestamp (in milliseconds).
     * Returns 0 if the bar index is out of bounds.
     */
    bar_index_to_timestamp(bar_index: number): bigint;
    begin_selected_drawing_text_edit(): boolean;
    /**
     * Return whether another indicator pane can be created under the current cap.
     */
    can_add_indicator_pane(): boolean;
    /**
     * Return whether a historical load of the given size would be accepted.
     */
    can_load_bar_count(bar_count: number): boolean;
    /**
     * Return whether the chart can switch from the current interval to the requested one.
     */
    can_set_interval(interval: string): boolean;
    /**
     * Cancel the drawing currently being created (e.g. on Escape key).
     */
    cancel_drawing(): void;
    /**
     * Clear all markers for all series.
     */
    clear_all_markers(): void;
    /**
     * Clear the interval allowlist.
     */
    clear_allowed_intervals(): void;
    /**
     * Remove all external autoscale contributions.
     */
    clear_autoscale_contributions(): void;
    /**
     * Hide crosshair immediately.
     */
    clear_crosshair(): void;
    /**
     * Remove all drawings.
     */
    clear_drawings(): void;
    /**
     * Clear all execution marks.
     */
    clear_execution_marks(): void;
    /**
     * Clear all footprint data.
     */
    clear_footprint_data(): void;
    /**
     * Clear all markers for a series.
     */
    clear_markers(series_id: number): void;
    /**
     * Remove all order lines.
     */
    clear_order_lines(): void;
    /**
     * Clear the selected execution mark.
     */
    clear_selected_execution_mark(): void;
    /**
     * Create a new AxiusCharts instance with a full options object.
     *
     * `container` can be an `HTMLElement` reference or a string container ID.
     * `options` is an optional JS object:
     * ```js
     * {
     *   theme: "dark" | "light" | { colors: {...}, crosshair: {...}, ... },
     *   renderer: "webgpu" | "canvas2d" | "auto",
     *   autoRender: true,
     *   symbol: "BTCUSD",
     *   interval: "1D",
     *   crosshair: { mode: "normal" | "magnet_ohlc" },
     *   priceScale: { mode: "normal", margins: { top: 0.1, bottom: 0.1 } },
     * }
     * ```
     */
    static create_chart(container: any, options: any): Promise<AxiusCharts>;
    /**
     * Create a new order line at the specified price level.
     *
     * This creates a platform-style order management line with:
     * - Order type label (Limit, Stop, TP, SL)
     * - Side indication (Buy/Sell) with appropriate colors
     * - Quantity display
     * - Draggable price modification
     * - Cancel button
     *
     * `order_type`: "limit", "stop", "stop_limit", "take_profit", "stop_loss", "trailing_stop"
     * `side`: "buy" or "sell"
     * `status`: "pending", "working", "partial", "filled", "cancelled"
     *
     * Returns the order line ID (the same string you passed in).
     */
    create_order_line(id: string, price: number, order_type: string, side: string, quantity: number, modifiable: boolean, cancellable: boolean): string;
    /**
     * Create an order line with full options.
     *
     * `order_type`: "limit", "stop", "stop_limit", "take_profit", "stop_loss", "trailing_stop"
     * `side`: "buy" or "sell"
     * `status`: "pending", "working", "partial", "filled", "cancelled"
     * `color_*`: Custom color override (pass all zeros to use default)
     * `custom_label`: Custom label text (empty string for auto-generated)
     */
    create_order_line_full(id: string, price: number, order_type: string, side: string, status: string, quantity: number, filled_quantity: number, modifiable: boolean, cancellable: boolean, color_r: number, color_g: number, color_b: number, color_a: number, custom_label: string, linked_position_id: string): string;
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
     * Create with a specific renderer backend (`auto`, `webgpu`, `canvas2d`).
     */
    static create_with(container_id: string, renderer: string): Promise<AxiusCharts>;
    crosshair_mode(): string;
    /**
     * Returns `[active, x, y, bar_index, price]`.
     */
    crosshair_state(): Float64Array;
    /**
     * Data timestamp range as `[from_ts, to_ts]`, or empty if no bars.
     */
    data_range(): Float64Array;
    demo_mode(): void;
    /**
     * Load synthetic demo data dedicated for footprint chart mode.
     *
     * This generates OHLCV bars plus aligned per-bar footprint levels and
     * switches the chart type to `footprint`.
     */
    demo_mode_footprint(): void;
    /**
     * Dispose: remove all event listeners, disconnect resize observer, and clean up resources.
     *
     * IMPORTANT: Call this when destroying the chart to prevent memory leaks.
     * Event listeners attached to DOM elements will keep the closures alive
     * even after AxiusCharts is dropped, unless explicitly removed.
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
    end_selected_drawing_text_edit(cancel: boolean): boolean;
    /**
     * Get the number of execution marks.
     */
    execution_mark_count(): number;
    /**
     * Expand the currently rendered execution cluster for a given leader ID.
     */
    expand_execution_cluster(leader_id: string): string[];
    /**
     * Export all drawings (main pane + indicator subpanes) as JSON.
     *
     * The returned string is versioned and can be stored externally.
     */
    export_drawings(): string;
    /**
     * Export a full chart persistence snapshot (styles + viewport + pane layout + drawings).
     *
     * `layout_id` is an optional caller-defined identifier to help external storage routing.
     */
    export_persistence_state(layout_id?: string | null): string;
    /**
     * Return whether auto-scroll is currently enabled.
     */
    get_auto_scroll(): boolean;
    /**
     * Get all available chart types as a comma-separated string.
     */
    static get_available_chart_types(): string;
    /**
     * Get the current chart type as a string.
     */
    get_chart_type(): string;
    /**
     * Get current CSS variables as a JS object.
     */
    get_css_variables(): any;
    /**
     * Get chart-wide drawing lock summary across the main pane and all subpanes.
     */
    get_drawings_lock_summary_json(): string;
    /**
     * Get the chart-wide execution label mode.
     */
    get_execution_label_mode(): string;
    /**
     * Whether execution mark text labels are currently rendered.
     */
    get_execution_mark_text_visible(): boolean;
    /**
     * Serialize all execution marks to JSON.
     */
    get_execution_marks_json(): string;
    /**
     * Whether realized P&L text is currently rendered for eligible execution marks.
     */
    get_execution_pnl_visible(): boolean;
    /**
     * Return whether footprint pane two-axis zoom (X+Y) is enabled.
     */
    get_footprint_xy_zoom_enabled(): boolean;
    /**
     * Serialize all order lines to JSON.
     */
    get_order_lines_json(): string;
    /**
     * Get the currently selected drawing's text/alignment inspector payload as JSON.
     * Returns `"null"` when no drawing is selected.
     */
    get_selected_drawing_info_json(): string;
    /**
     * Get the currently selected execution mark ID, or null if none.
     */
    get_selected_execution_mark(): string | undefined;
    /**
     * Get study output data as a JS object { timestamps: BigUint64Array, values: Float64Array }.
     * Returns null if the study or output index doesn't exist.
     */
    get_study_output(id: number, output_index: number): any;
    static get_supported_renderers(): Array<any>;
    /**
     * Whether volume bars are currently visible in the main pane.
     */
    get_volume_visible(): boolean;
    /**
     * Hit-test rendered series markers at pane CSS coordinates.
     *
     * Returns `null` when no rendered marker contains the point.
     */
    hit_test_marker(x_css: number, y_css: number): any;
    /**
     * Restore all drawings (main pane + indicator subpanes) from JSON.
     *
     * Existing drawings are replaced atomically. Unknown subpane IDs in the payload are ignored.
     */
    import_drawings(json: string): void;
    /**
     * Restore a full chart persistence snapshot (styles + viewport + pane layout + drawings).
     */
    import_persistence_state(json: string): void;
    /**
     * Attach a compiled indicator program to the current chart.
     * Returns a runtime instance ID, or 0 on failure.
     */
    indicator_attach(indicator_id: number, opts_json: string): number;
    /**
     * Compile user indicator source into the internal IR program artifact.
     * Returns: `{ indicatorId, diagnostics }`.
     */
    indicator_compile(source: string, meta_json: string): any;
    /**
     * Detach an indicator runtime instance.
     */
    indicator_detach(instance_id: number): boolean;
    /**
     * Drain and return pending runtime events from indicator instances.
     *
     * Returns an array of objects:
     * `{ instanceId, indicatorId, type, code, message, barIndex }`
     */
    indicator_drain_events(): any;
    /**
     * Get diagnostics for a compiled indicator.
     */
    indicator_get_diagnostics(indicator_id: number): any;
    /**
     * Get compile-time-discovered MTF request templates from a compiled indicator.
     */
    indicator_get_mtf_requests(indicator_id: number): any;
    /**
     * Get runtime stats for an indicator instance.
     */
    indicator_get_stats(instance_id: number): any;
    /**
     * List attached indicator instances.
     */
    indicator_list(): any;
    /**
     * Get the number of indicator sub-panes.
     */
    indicator_pane_count(): number;
    /**
     * Enable or disable a runtime indicator instance.
     */
    indicator_set_enabled(instance_id: number, enabled: boolean): boolean;
    /**
     * Set runtime inputs for an attached indicator instance.
     */
    indicator_set_inputs(instance_id: number, inputs_json: string): boolean;
    /**
     * Load backend-resolved MTF series snapshots into the runtime resolver cache.
     *
     * JSON payload:
     * `{ clear?: bool, series: [{ symbol, chartTimeframe, requestId?, timeframe, field, mode?, points: [...] }] }`
     */
    indicator_set_mtf_snapshot(snapshot_json: string): boolean;
    /**
     * Privileged runtime-only resource limit override for an indicator instance.
     */
    indicator_set_resource_limits(instance_id: number, limits_json: string): boolean;
    interval(): string;
    /**
     * Return whether interval changes are locked.
     */
    interval_change_locked(): boolean;
    /**
     * Returns whether auto-render is currently active.
     */
    is_auto_render(): boolean;
    /**
     * Return whether a specific interval is permitted by the current guardrails.
     */
    is_interval_allowed(interval: string): boolean;
    /**
     * Whether marker visual size participates in automatic price scaling.
     */
    marker_auto_scale(): boolean;
    /**
     * Get the current global marker z-order.
     */
    marker_z_order(): string;
    /**
     * Get the maximum historical bar count allowed in a single load. Returns 0 when uncapped.
     */
    max_bars_per_load(): number;
    /**
     * Get the maximum indicator sub-pane count. Returns 0 when uncapped.
     */
    max_indicator_panes(): number;
    /**
     * Remove a specific event callback.
     */
    off(event: string, callback: Function): void;
    /**
     * Register an event callback.
     *
     * ```js
     * chart.on("crosshairMove", (event) => {
     *   console.log(event.x, event.y, event.price);
     * });
     * ```
     *
     * Valid event names: crosshairMove, visibleRangeChange, click,
     * drawingCreated, drawingSelected, symbolChange, intervalChange,
     * priceScaleChange, chartTypeChange, resize, error.
     */
    on(event: string, callback: Function): void;
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
     * Register a one-shot event callback (auto-removes after first call).
     */
    once(event: string, callback: Function): void;
    /**
     * Get the number of order lines.
     */
    order_line_count(): number;
    /**
     * Get the number of price lines.
     */
    price_line_count(): number;
    /**
     * Project a timestamp/price coordinate into current pane CSS coordinates.
     */
    project_point(timestamp_ms: bigint, price: number): any;
    /**
     * Remove all scale (measurement) drawings.
     */
    remove_all_scale_drawings(): void;
    /**
     * Remove a previously registered autoscale contribution.
     */
    remove_autoscale_contribution(id: number): boolean;
    /**
     * Remove an execution mark by ID.
     */
    remove_execution_mark(id: string): boolean;
    /**
     * Remove an indicator sub-pane by ID.
     */
    remove_indicator_pane(pane_id: number): boolean;
    /**
     * Remove a specific marker from a series.
     */
    remove_marker(series_id: number, marker_id: number): boolean;
    /**
     * Remove an order line by ID.
     */
    remove_order_line(id: string): boolean;
    /**
     * Remove all order lines with a specific status.
     *
     * `status`: "pending", "working", "partial", "filled", "cancelled", "rejected", "expired"
     */
    remove_order_lines_by_status(status: string): void;
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
     * Get replay cutoff bar index, or -1 when unavailable.
     */
    replay_cutoff_bar(): bigint;
    /**
     * Whether replay mode is currently active.
     */
    replay_mode(): boolean;
    /**
     * Get current replay runtime options.
     */
    replay_options(): any;
    /**
     * Whether replay playback is currently running.
     */
    replay_playing(): boolean;
    /**
     * Step replay backward by 1 bar.
     */
    replay_step_back(): void;
    /**
     * Step replay forward by 1 bar.
     */
    replay_step_forward(): void;
    /**
     * Reset the main chart viewport.
     *
     * Supported modes:
     * - `"default"`: restore the recent-bars default view with a small right gap
     * - `"fit_all"`: show the full dataset with a small right gap
     *
     * Unknown or omitted modes fall back to `"default"`.
     */
    reset_viewport(mode?: string | null): void;
    /**
     * Get the number of overlay series.
     */
    series_count(): number;
    /**
     * Lock or unlock every drawing on the chart across the main pane and all subpanes.
     */
    set_all_drawings_locked(locked: boolean): boolean;
    /**
     * Replace the allowed interval list. Pass an empty array to remove the allowlist.
     */
    set_allowed_intervals(intervals: Array<any>): void;
    /**
     * Enable or disable auto-scroll on new bars.
     *
     * When `true` (default) the viewport advances by 1 bar each time a new bar
     * is appended and the chart is already showing the latest data — identical
     * to the reference implementation's `shiftVisibleRangeOnNewBar` behaviour.
     *
     * When `false` the viewport never moves during live streaming regardless of
     * the current scroll position, giving the user a fully static view even
     * while data is updating in real time.
     */
    set_auto_scroll(enabled: boolean): void;
    /**
     * Set the axis border (separator line) color (RGBA 0-1).
     */
    set_axis_border_color(r: number, g: number, b: number, a: number): void;
    /**
     * Show or hide the axis border line. Layout is unaffected.
     */
    set_axis_border_visible(visible: boolean): void;
    /**
     * Set the axis label text color (RGBA 0-1).
     */
    set_axis_text_color(r: number, g: number, b: number, a: number): void;
    /**
     * Show or hide axis tick marks. Layout is unaffected.
     */
    set_axis_ticks_visible(visible: boolean): void;
    /**
     * Set chart and axis background color (RGBA 0-1).
     */
    set_background_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set data for a bar (OHLC) series.
     * All arrays must be the same length.
     */
    set_bar_series_data(id: number, timestamps: BigUint64Array, open: Float64Array, high: Float64Array, low: Float64Array, close: Float64Array): void;
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
     * "heikin_ashi", "ha", "footprint", "fp", "order_flow".
     */
    set_chart_type(chart_type: string): void;
    /**
     * Set the shared crosshair label text color (applies to both axes).
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
     * Set crosshair mode: "normal" or "magnet_ohlc".
     *
     * Legacy alias:
     * - "magnet" is accepted and treated as "magnet_ohlc".
     */
    set_crosshair_mode(mode: string): void;
    /**
     * Set crosshair state for synchronized groups.
     */
    set_crosshair_state(active: boolean, x: number, y: number, bar_index: number, price: number, mode: string): void;
    /**
     * Set crosshair state for synchronized panes by semantic values only.
     * This keeps the target pane snapped to its own viewport/grid.
     */
    set_crosshair_sync_state(active: boolean, bar_index: number, price: number, mode: string): void;
    set_data_arrays(open: Float64Array, high: Float64Array, low: Float64Array, close: Float64Array, volume: Float64Array, timestamps: BigUint64Array): void;
    /**
     * Atomically load OHLCV bars plus aligned footprint data from typed arrays.
     *
     * This is the canonical historical footprint initialization path for
     * production integrations. `level_offsets` is bar-aligned and must have
     * length `bars.len() + 1`; sparse bars use empty ranges.
     */
    set_data_with_footprint_arrays(open: Float64Array, high: Float64Array, low: Float64Array, close: Float64Array, volume: Float64Array, timestamps: BigUint64Array, level_offsets: Uint32Array, prices: Float64Array, bid_volumes: Float64Array, ask_volumes: Float64Array): void;
    /**
     * Atomically load OHLCV bars plus footprint levels from JSON.
     *
     * Expected canonical format:
     * `[{"timestamp": 1710000000000, "open": 100.0, "high": 101.0, "low": 99.5, "close": 100.5, "volume": 2500.0, "levels": [{"price": 99.5, "bid": 120.0, "ask": 80.0}]}]`
     *
     * Also accepts `{ "bars": [...] }` as the top-level wrapper and the
     * existing `bid_volume` / `bidVolume` / `ask_volume` / `askVolume` level aliases.
     */
    set_data_with_footprint_json(json: string): void;
    /**
     * Set active drawing tool: "none", "trend_line", "rectangle", "fibonacci",
     * "scale", "brush", "horizontal_line", "vertical_line", "ray", "path".
     */
    set_drawing_tool(tool: string): void;
    /**
     * Set the CSS-pixel clustering threshold for dense execution marks.
     */
    set_execution_cluster_threshold_px(threshold_px: number): void;
    /**
     * Set the chart-wide execution label mode.
     *
     * Accepted values: `"side"`, `"role"`, `"side_and_role"` (case-insensitive).
     */
    set_execution_label_mode(mode: string): void;
    /**
     * Show/hide execution mark text labels.
     */
    set_execution_mark_text_visible(visible: boolean): void;
    /**
     * Set multiple execution marks at once (replaces existing).
     *
     * `mark_data` is a flat array of execution mark data with stride 6:
     * [timestamp_ms, price, quantity, side_idx, role_idx, ...]
     * where side_idx: 0=buy, 1=sell
     * and role_idx: 0=entry, 1=scale_in, 2=scale_out, 3=exit
     *
     * `ids` is an array of string IDs (must match mark_data length / 5).
     */
    set_execution_marks(ids: string[], mark_data: Float64Array): void;
    /**
     * Set execution marks from a JSON string.
     *
     * Expected format:
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
     */
    set_execution_marks_json(json: string): void;
    /**
     * Show or hide realized P&L text for eligible execution marks.
     */
    set_execution_pnl_visible(visible: boolean): void;
    /**
     * Set the font family for axis labels.
     */
    set_font_family(family: string): void;
    /**
     * Set the font size for axis labels (in CSS pixels).
     */
    set_font_size(size: number): void;
    /**
     * Set footprint (order-flow) data for a specific bar.
     *
     * `bar_index`: the bar index in the main data array.
     * `prices`: price levels (ascending order).
     * `bid_volumes`: bid volume at each price level.
     * `ask_volumes`: ask volume at each price level.
     *
     * All three arrays must be the same length.
     */
    set_footprint_bar(bar_index: number, prices: Float64Array, bid_volumes: Float64Array, ask_volumes: Float64Array): void;
    /**
     * Bulk set footprint data with typed arrays (fast path for external feeds).
     *
     * Layout:
     * - `bar_indices`: one entry per footprint bar.
     * - `level_offsets`: length must be `bar_indices.len() + 1`.
     *   Each bar `i` uses level range `[level_offsets[i], level_offsets[i + 1])`.
     * - `prices`, `bid_volumes`, `ask_volumes`: flattened level arrays.
     *
     * Example:
     * - bar_indices = [10, 11]
     * - level_offsets = [0, 3, 5]
     * - levels for bar 10 = [0..3), bar 11 = [3..5)
     */
    set_footprint_data_arrays(bar_indices: Uint32Array, level_offsets: Uint32Array, prices: Float64Array, bid_volumes: Float64Array, ask_volumes: Float64Array): void;
    /**
     * Set footprint data from a JSON string for bulk loading.
     *
     * Expected format:
     * `[{"bar_index": 0, "levels": [{"price": 100.0, "bid": 150, "ask": 200}, ...]}]`
     *
     * Also accepts aliases:
     * - `barIndex` / `index` for `bar_index`
     * - `bid_volume` / `bidVolume` for `bid`
     * - `ask_volume` / `askVolume` for `ask`
     */
    set_footprint_data_json(json: string): void;
    /**
     * Set footprint display mode.
     * Accepted values: "bid_ask", "delta", "volume", "delta_profile", "volume_profile".
     */
    set_footprint_display_mode(mode: string): void;
    /**
     * Configure footprint options from a JSON object.
     *
     * Supported keys:
     * - `display_mode`: string ("bid_ask", "delta", "volume", etc.)
     * - `tick_size`: number
     * - `palette`: string (`"blue_red"` default, `"green_red"`)
     * - `gradient_style`: string (`"no_glow"` default, `"soft_glow"`, `"strong_glow"`)
     * - `poc_color`: CSS color string or `[r, g, b, a]`
     * - `imbalance_ratio`: number (default 3.0)
     * - `show_imbalances`: boolean
     * - `show_poc`: boolean
     * - `show_value_area`: boolean
     * - `value_area_pct`: number (0.0-1.0, default 0.70)
     * - `show_delta_bar`: boolean
     * - `show_volume_text`: boolean
     * - `show_unfinished_auction`: boolean
     * - `zoom_price_with_time`: boolean (footprint wheel/pinch X+Y zoom)
     */
    set_footprint_options(json: string): void;
    /**
     * Set footprint tick size (price granularity). Pass 0.0 for auto-detection.
     */
    set_footprint_tick_size(tick_size: number): void;
    /**
     * Enable/disable footprint pane two-axis zoom (X+Y) for wheel and pinch.
     */
    set_footprint_xy_zoom_enabled(enabled: boolean): void;
    /**
     * Set the grid line color (RGBA 0-1).
     */
    set_grid_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set data for a histogram series. `values` and `timestamps` must be same length.
     * Per-bar colors are optional — pass empty arrays to use the series default color.
     */
    set_histogram_data(id: number, values: Float64Array, timestamps: BigUint64Array, colors_r: Float32Array, colors_g: Float32Array, colors_b: Float32Array, colors_a: Float32Array): void;
    set_interval(interval: string): void;
    /**
     * Lock or unlock interval changes away from the current interval.
     */
    set_interval_change_locked(locked: boolean): void;
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
     * Include marker visual size in automatic price scaling.
     */
    set_marker_auto_scale(auto_scale: boolean): void;
    /**
     * Set the global marker z-order: "normal", "aboveSeries", or "top".
     */
    set_marker_z_order(z_order: string): void;
    /**
     * Set multiple markers for a series at once (replaces existing).
     * `marker_data` is a flat array: [bar_index, shape_idx, position_idx, price, r, g, b, a, size, ...]
     * where shape_idx: 0=arrowUp, 1=arrowDown, 2=circle, 3=square
     * and position_idx: 0=aboveBar, 1=belowBar, 2=atPrice
     */
    set_markers(series_id: number, marker_data: Float64Array): void;
    /**
     * Set the maximum historical bar count allowed in a single load. Pass 0 to disable the cap.
     */
    set_max_bars_per_load(max_bars: number): void;
    /**
     * Set the maximum indicator sub-pane count. Pass 0 to disable the cap.
     */
    set_max_indicator_panes(max_panes: number): void;
    /**
     * Update the filled quantity of an order line (for partial fills).
     */
    set_order_line_filled_quantity(id: string, filled: number): boolean;
    /**
     * Update the live PNL displayed on an existing order line.
     */
    set_order_line_pnl(id: string, pnl: number): boolean;
    /**
     * Update the price of an existing order line.
     */
    set_order_line_price(id: string, price: number): boolean;
    /**
     * Set the price precision (decimal places) for order line labels.
     */
    set_order_line_price_precision(precision: number): void;
    /**
     * Set whether to show cancel buttons on order lines.
     */
    set_order_line_show_cancel_buttons(show: boolean): void;
    /**
     * Update the status of an order line.
     *
     * `status`: "pending", "working", "partial", "filled", "cancelled", "rejected", "expired"
     */
    set_order_line_status(id: string, status: string): boolean;
    /**
     * Set whether an order line is visible.
     */
    set_order_line_visible(id: string, visible: boolean): boolean;
    /**
     * Load order lines from JSON (replaces existing).
     *
     * Expected format:
     * ```json
     * {
     *   "version": 1,
     *   "orders": [
     *     {
     *       "id": "order-1",
     *       "price": 50000.0,
     *       "order_type": "Limit",
     *       "side": "Buy",
     *       "status": "Pending",
     *       "quantity": 0.5,
     *       "filled_quantity": 0.0,
     *       "visible": true,
     *       "cancellable": true,
     *       "modifiable": true
     *     }
     *   ]
     * }
     * ```
     */
    set_order_lines_json(json: string): void;
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
     * Set the price scale tick mark density multiplier.
     */
    set_price_scale_tick_density(density: number): void;
    /**
     * Set replay cutoff bar (inclusive right-edge trim).
     */
    set_replay_cutoff_bar(index: number): void;
    /**
     * Enter/exit market replay mode.
     */
    set_replay_mode(enabled: boolean): void;
    /**
     * Update replay runtime options.
     */
    set_replay_options(options: any): void;
    /**
     * Start/pause replay playback.
     */
    set_replay_playing(playing: boolean): void;
    /**
     * Lock or unlock the currently selected drawing.
     */
    set_selected_drawing_locked(locked: boolean): boolean;
    /**
     * Set inline text on the currently selected drawing.
     */
    set_selected_drawing_text(text: string): boolean;
    /**
     * Set text alignment on the currently selected drawing.
     */
    set_selected_drawing_text_alignment(horizontal: string, vertical: string): boolean;
    /**
     * Set font size / italic / color override on the currently selected drawing label.
     */
    set_selected_drawing_text_style(font_size: number, italic: boolean, r: number, g: number, b: number, a: number, follow_drawing_color: boolean): boolean;
    /**
     * Set the selected execution mark ID (shows selected-trade execution locators).
     * Pass empty string or null to deselect.
     */
    set_selected_execution_mark(mark_id?: string | null): void;
    /**
     * Replace the currently selected Fibonacci drawing's levels from JSON.
     * Input shape: `[{"ratio":0.5,"label":"Mid"}, ...]`
     */
    set_selected_fibonacci_levels_json(json: string): boolean;
    /**
     * Toggle / configure the optional horizontal middle line on the currently
     * selected Rectangle drawing (platform-style midline).
     *
     * `dash_on`/`dash_off` ≤ 0 means a solid line. Returns `false` when the
     * current selection is not a Rectangle, or when nothing is selected.
     */
    set_selected_rectangle_middle_line(enabled: boolean, r: number, g: number, b: number, a: number, line_width: number, dash_on: number, dash_off: number): boolean;
    /**
     * Update the border on the currently selected Text drawing. The color,
     * width, and dash are always written so toggling `enabled` off and back
     * on preserves the user's last picks.
     *
     * `dash_on`/`dash_off` ≤ 0 means a solid line. Returns `false` when the
     * current selection is not a Text drawing, or when nothing is selected.
     */
    set_selected_text_border(enabled: boolean, r: number, g: number, b: number, a: number, line_width: number, dash_on: number, dash_off: number): boolean;
    /**
     * Update the background fill on the currently selected Text drawing.
     * The color (including alpha) is always written so toggling `enabled`
     * off and back on preserves the user's last picked color.
     */
    set_selected_text_fill(enabled: boolean, r: number, g: number, b: number, a: number): boolean;
    /**
     * Set data for a line series. `values` and `timestamps` must be same length.
     */
    set_series_data(id: number, values: Float64Array, timestamps: BigUint64Array): void;
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
     * Set indicator sub-pane separator line color (RGBA, 0.0-1.0).
     */
    set_subpane_separator_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set indicator sub-pane separator drag hit-area thickness (CSS px).
     */
    set_subpane_separator_hit_area(hit_area_css: number): void;
    /**
     * Set indicator sub-pane separator hover/active color (RGBA, 0.0-1.0).
     */
    set_subpane_separator_hover_color(r: number, g: number, b: number, a: number): void;
    /**
     * Set indicator sub-pane separator visible line thickness (CSS px).
     */
    set_subpane_separator_thickness(thickness_css: number): void;
    set_symbol(symbol: string): void;
    /**
     * Set multiple timestamp-anchored markers for a series at once.
     *
     * `timestamps` contains one timestamp per marker. `marker_data` is a flat
     * array with stride 8: [shape_idx, position_idx, price, r, g, b, a, size, ...].
     */
    set_time_markers(series_id: number, timestamps: BigUint64Array, marker_data: Float64Array): void;
    /**
     * Set visible bar range using fractional bar indices.
     */
    set_visible_range(start: number, end: number): void;
    /**
     * Set volume bar colors: bullish and bearish.
     */
    set_volume_colors(up_r: number, up_g: number, up_b: number, up_a: number, down_r: number, down_g: number, down_b: number, down_a: number): void;
    /**
     * Show/hide volume bars in the main pane.
     */
    set_volume_visible(visible: boolean): void;
    /**
     * Start the auto-render RAF loop.
     */
    start_auto_render(): void;
    /**
     * Stop the auto-render RAF loop. Caller must manually call render().
     */
    stop_auto_render(): void;
    /**
     * Get the number of studies.
     */
    study_count(): number;
    symbol(): string;
    /**
     * Get the current theme preset name ("dark", "light", or "custom").
     */
    theme(): string;
    /**
     * Advance the text-edit caret blink phase. The host should call this on
     * each animation frame (e.g. inside the rAF loop) passing `performance.now()`
     * in milliseconds. Returns true when the caret visibility flipped, in which
     * case the canvas is automatically marked dirty for repaint. When no text
     * edit is active this is a cheap no-op.
     */
    tick_drawing_caret_blink(now_ms: number): boolean;
    /**
     * Convert a timestamp (in milliseconds) to a bar index.
     * Returns -1 if the timestamp is before all bars.
     */
    timestamp_to_bar_index(timestamp_ms: bigint): bigint;
    /**
     * Update indicator sub-pane data from a study.
     */
    update_indicator_pane(pane_id: number, study_id: number): void;
    /**
     * Update the last bar in the data array. Used for real-time tick updates.
     */
    update_last_bar(timestamp: bigint, open: number, high: number, low: number, close: number, volume: number): void;
    /**
     * Update the last point in a bar (OHLC) overlay series.
     */
    update_last_bar_series_point(id: number, timestamp: bigint, open: number, high: number, low: number, close: number): void;
    /**
     * Update the last point in a histogram overlay series.
     */
    update_last_histogram_point(id: number, timestamp: bigint, value: number, color_r: number, color_g: number, color_b: number, color_a: number): void;
    /**
     * Update the last point in a line/area/baseline overlay series.
     */
    update_last_series_point(id: number, timestamp: bigint, value: number): void;
    /**
     * compatibility-style main series update semantics:
     * update last bar if timestamp matches, append if timestamp is newer.
     */
    upsert_bar(timestamp: bigint, open: number, high: number, low: number, close: number, volume: number): void;
    /**
     * compatibility-style update semantics for OHLC bar overlays:
     * update last point if timestamp matches, append if timestamp is newer.
     */
    upsert_bar_series_point(id: number, timestamp: bigint, open: number, high: number, low: number, close: number): void;
    /**
     * Upsert a main bar and atomically set its footprint levels.
     *
     * This is the preferred real-time API for external order-flow feeds:
     * one call updates OHLCV + footprint for the same logical bar.
     */
    upsert_bar_with_footprint(timestamp: bigint, open: number, high: number, low: number, close: number, volume: number, prices: Float64Array, bid_volumes: Float64Array, ask_volumes: Float64Array): void;
    /**
     * compatibility-style update semantics for histogram overlays:
     * update last point if timestamp matches, append if timestamp is newer.
     */
    upsert_histogram_point(id: number, timestamp: bigint, value: number, color_r: number, color_g: number, color_b: number, color_a: number): void;
    /**
     * compatibility-style update semantics for line/area/baseline overlays:
     * update last point if timestamp matches, append if timestamp is newer.
     */
    upsert_series_point(id: number, timestamp: bigint, value: number): void;
    visible_range(): Float64Array;
    zoom_to_range(start: bigint, end: bigint): void;
}

export class ChartGroup {
    free(): void;
    [Symbol.dispose](): void;
    add_pane(symbol: string, interval: string): number;
    link_panes(a: number, b: number): boolean;
    constructor();
    pane_count(): number;
    /**
     * Returns `[from_timestamp, to_timestamp]`, or empty if unavailable.
     */
    pane_data_range(pane_id: number): Float64Array;
    pane_interval(pane_id: number): string;
    pane_symbol(pane_id: number): string;
    /**
     * Returns `[start_bar, end_bar]`, or empty if pane is missing.
     */
    pane_time_range(pane_id: number): Float64Array;
    remove_pane(pane_id: number): boolean;
    set_auto_link(enabled: boolean): void;
    set_sync(feature: string, enabled: boolean): void;
    set_sync_for_link(pane_a: number, pane_b: number, feature: string, enabled: boolean): void;
    set_sync_for_pane(pane_id: number, feature: string, enabled: boolean): void;
    unlink_panes(a: number, b: number): boolean;
    /**
     * `crosshair` format: `[active, x, y, bar_index, price, magnet]`.
     * `magnet`: 0 = normal, 1 = OHLC magnet.
     */
    update_crosshair(source: number, crosshair: Float64Array): Array<any>;
    update_data_range(source: number, from_timestamp: number, to_timestamp: number): Array<any>;
    update_interval(source: number, interval: string): Array<any>;
    update_symbol(source: number, symbol: string): Array<any>;
    update_time_range(source: number, start_bar: number, end_bar: number): Array<any>;
}

export class ChartWorkspace {
    free(): void;
    [Symbol.dispose](): void;
    active_pane_id(): number;
    can_split_active(): boolean;
    can_split_pane(pane_id: number): boolean;
    clear_on_active_pane_change(): void;
    clear_pane_fullscreen(): boolean;
    dispose(): void;
    fullscreen_pane_id(): number;
    is_pane_fullscreen(): boolean;
    max_panes(): number;
    constructor(container_id: string);
    pane_host_id(pane_id: number): string;
    pane_ids(): Array<any>;
    root_pane_id(): number;
    set_active_pane(pane_id: number): boolean;
    set_max_panes(max_panes: number): void;
    set_on_active_pane_change(callback: Function): void;
    set_split_divider_active_color(r: number, g: number, b: number, a: number): void;
    set_split_divider_color(r: number, g: number, b: number, a: number): void;
    set_split_divider_hit_area(hit_area_css: number): void;
    set_split_divider_thickness(thickness_css: number): void;
    set_workspace_active_pane_border_color(r: number, g: number, b: number, a: number): void;
    set_workspace_active_pane_border_width(width_css: number): void;
    set_workspace_pane_background_color(r: number, g: number, b: number, a: number): void;
    split_active(direction: string): number;
    split_pane(pane_id: number, direction: string): number;
    toggle_pane_fullscreen(pane_id: number): boolean;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_axiuscharts_free: (a: number, b: number) => void;
    readonly __wbg_chartgroup_free: (a: number, b: number) => void;
    readonly __wbg_chartworkspace_free: (a: number, b: number) => void;
    readonly axiuscharts_active_order_line_count: (a: number) => number;
    readonly axiuscharts_add_area_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => number;
    readonly axiuscharts_add_autoscale_contribution: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_add_bar_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => number;
    readonly axiuscharts_add_baseline_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number, v: number, w: number, x: number, y: number, z: number, a1: number) => number;
    readonly axiuscharts_add_execution_mark: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly axiuscharts_add_execution_mark_full: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number) => void;
    readonly axiuscharts_add_histogram_series: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly axiuscharts_add_indicator_pane: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly axiuscharts_add_line_series: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly axiuscharts_add_marker: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number) => void;
    readonly axiuscharts_add_marker_at_time: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number) => void;
    readonly axiuscharts_allowed_intervals: (a: number) => number;
    readonly axiuscharts_append_bar: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_append_bar_series_point: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_append_histogram_point: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_append_series_point: (a: number, b: number, c: number, d: bigint, e: number) => void;
    readonly axiuscharts_apply_options: (a: number, b: number) => void;
    readonly axiuscharts_bar_index_to_timestamp: (a: number, b: number) => bigint;
    readonly axiuscharts_begin_selected_drawing_text_edit: (a: number) => number;
    readonly axiuscharts_can_add_indicator_pane: (a: number) => number;
    readonly axiuscharts_can_load_bar_count: (a: number, b: number) => number;
    readonly axiuscharts_can_set_interval: (a: number, b: number, c: number) => number;
    readonly axiuscharts_cancel_drawing: (a: number) => void;
    readonly axiuscharts_clear_all_markers: (a: number) => void;
    readonly axiuscharts_clear_allowed_intervals: (a: number) => void;
    readonly axiuscharts_clear_autoscale_contributions: (a: number) => void;
    readonly axiuscharts_clear_crosshair: (a: number) => void;
    readonly axiuscharts_clear_drawings: (a: number) => void;
    readonly axiuscharts_clear_execution_marks: (a: number) => void;
    readonly axiuscharts_clear_footprint_data: (a: number) => void;
    readonly axiuscharts_clear_markers: (a: number, b: number) => void;
    readonly axiuscharts_clear_order_lines: (a: number) => void;
    readonly axiuscharts_clear_selected_execution_mark: (a: number) => void;
    readonly axiuscharts_create_chart: (a: number, b: number) => number;
    readonly axiuscharts_create_order_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly axiuscharts_create_order_line_full: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number, v: number, w: number) => void;
    readonly axiuscharts_create_price_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly axiuscharts_create_study: (a: number, b: number, c: number) => number;
    readonly axiuscharts_create_with: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_crosshair_mode: (a: number, b: number) => void;
    readonly axiuscharts_crosshair_state: (a: number, b: number) => void;
    readonly axiuscharts_data_range: (a: number, b: number) => void;
    readonly axiuscharts_demo_mode: (a: number) => void;
    readonly axiuscharts_demo_mode_footprint: (a: number) => void;
    readonly axiuscharts_dispose: (a: number) => void;
    readonly axiuscharts_drag_pane_separator: (a: number, b: number, c: number) => void;
    readonly axiuscharts_drawing_count: (a: number) => number;
    readonly axiuscharts_end_selected_drawing_text_edit: (a: number, b: number) => number;
    readonly axiuscharts_execution_mark_count: (a: number) => number;
    readonly axiuscharts_expand_execution_cluster: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_export_drawings: (a: number, b: number) => void;
    readonly axiuscharts_export_persistence_state: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_get_auto_scroll: (a: number) => number;
    readonly axiuscharts_get_available_chart_types: (a: number) => void;
    readonly axiuscharts_get_chart_type: (a: number, b: number) => void;
    readonly axiuscharts_get_css_variables: (a: number) => number;
    readonly axiuscharts_get_drawings_lock_summary_json: (a: number, b: number) => void;
    readonly axiuscharts_get_execution_label_mode: (a: number, b: number) => void;
    readonly axiuscharts_get_execution_mark_text_visible: (a: number) => number;
    readonly axiuscharts_get_execution_marks_json: (a: number, b: number) => void;
    readonly axiuscharts_get_execution_pnl_visible: (a: number) => number;
    readonly axiuscharts_get_footprint_xy_zoom_enabled: (a: number) => number;
    readonly axiuscharts_get_order_lines_json: (a: number, b: number) => void;
    readonly axiuscharts_get_selected_drawing_info_json: (a: number, b: number) => void;
    readonly axiuscharts_get_selected_execution_mark: (a: number, b: number) => void;
    readonly axiuscharts_get_study_output: (a: number, b: number, c: number) => number;
    readonly axiuscharts_get_supported_renderers: () => number;
    readonly axiuscharts_get_volume_visible: (a: number) => number;
    readonly axiuscharts_hit_test_marker: (a: number, b: number, c: number) => number;
    readonly axiuscharts_import_drawings: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_import_persistence_state: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_indicator_attach: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_indicator_compile: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly axiuscharts_indicator_detach: (a: number, b: number) => number;
    readonly axiuscharts_indicator_drain_events: (a: number) => number;
    readonly axiuscharts_indicator_get_diagnostics: (a: number, b: number) => number;
    readonly axiuscharts_indicator_get_mtf_requests: (a: number, b: number) => number;
    readonly axiuscharts_indicator_get_stats: (a: number, b: number) => number;
    readonly axiuscharts_indicator_list: (a: number) => number;
    readonly axiuscharts_indicator_pane_count: (a: number) => number;
    readonly axiuscharts_indicator_set_enabled: (a: number, b: number, c: number) => number;
    readonly axiuscharts_indicator_set_inputs: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_indicator_set_mtf_snapshot: (a: number, b: number, c: number) => number;
    readonly axiuscharts_indicator_set_resource_limits: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_interval: (a: number, b: number) => void;
    readonly axiuscharts_interval_change_locked: (a: number) => number;
    readonly axiuscharts_is_auto_render: (a: number) => number;
    readonly axiuscharts_is_interval_allowed: (a: number, b: number, c: number) => number;
    readonly axiuscharts_marker_auto_scale: (a: number) => number;
    readonly axiuscharts_marker_z_order: (a: number, b: number) => void;
    readonly axiuscharts_max_bars_per_load: (a: number) => number;
    readonly axiuscharts_max_indicator_panes: (a: number) => number;
    readonly axiuscharts_off: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_on: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_on_key_down: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly axiuscharts_once: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_order_line_count: (a: number) => number;
    readonly axiuscharts_price_line_count: (a: number) => number;
    readonly axiuscharts_project_point: (a: number, b: bigint, c: number) => number;
    readonly axiuscharts_remove_all_scale_drawings: (a: number) => void;
    readonly axiuscharts_remove_autoscale_contribution: (a: number, b: number) => number;
    readonly axiuscharts_remove_execution_mark: (a: number, b: number, c: number) => number;
    readonly axiuscharts_remove_indicator_pane: (a: number, b: number) => number;
    readonly axiuscharts_remove_marker: (a: number, b: number, c: number) => number;
    readonly axiuscharts_remove_order_line: (a: number, b: number, c: number) => number;
    readonly axiuscharts_remove_order_lines_by_status: (a: number, b: number, c: number) => void;
    readonly axiuscharts_remove_price_line: (a: number, b: number) => number;
    readonly axiuscharts_remove_selected_drawing: (a: number) => void;
    readonly axiuscharts_remove_series: (a: number, b: number) => number;
    readonly axiuscharts_remove_study: (a: number, b: number) => number;
    readonly axiuscharts_render: (a: number) => void;
    readonly axiuscharts_renderer_name: (a: number, b: number) => void;
    readonly axiuscharts_replay_cutoff_bar: (a: number) => bigint;
    readonly axiuscharts_replay_mode: (a: number) => number;
    readonly axiuscharts_replay_options: (a: number) => number;
    readonly axiuscharts_replay_playing: (a: number) => number;
    readonly axiuscharts_replay_step_back: (a: number, b: number) => void;
    readonly axiuscharts_replay_step_forward: (a: number, b: number) => void;
    readonly axiuscharts_reset_viewport: (a: number, b: number, c: number) => void;
    readonly axiuscharts_series_count: (a: number) => number;
    readonly axiuscharts_set_all_drawings_locked: (a: number, b: number) => number;
    readonly axiuscharts_set_allowed_intervals: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_auto_scroll: (a: number, b: number) => void;
    readonly axiuscharts_set_axis_border_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_axis_border_visible: (a: number, b: number) => void;
    readonly axiuscharts_set_axis_text_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_axis_ticks_visible: (a: number, b: number) => void;
    readonly axiuscharts_set_background_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_bar_series_data: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => void;
    readonly axiuscharts_set_bar_width_ratio: (a: number, b: number) => void;
    readonly axiuscharts_set_bearish_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_set_bullish_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_set_chart_type: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_crosshair_label_text_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_crosshair_label_visible: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_crosshair_line_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly axiuscharts_set_crosshair_line_label_bg_color: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly axiuscharts_set_crosshair_line_style: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_crosshair_line_visible: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_crosshair_line_width: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_crosshair_mode: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_crosshair_state: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_set_crosshair_sync_state: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly axiuscharts_set_data_arrays: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => void;
    readonly axiuscharts_set_data_with_footprint_arrays: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number, v: number) => void;
    readonly axiuscharts_set_data_with_footprint_json: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_drawing_tool: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_execution_cluster_threshold_px: (a: number, b: number) => void;
    readonly axiuscharts_set_execution_label_mode: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_execution_mark_text_visible: (a: number, b: number) => void;
    readonly axiuscharts_set_execution_marks: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_execution_marks_json: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_execution_pnl_visible: (a: number, b: number) => void;
    readonly axiuscharts_set_font_family: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_font_size: (a: number, b: number) => void;
    readonly axiuscharts_set_footprint_bar: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_set_footprint_data_arrays: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly axiuscharts_set_footprint_data_json: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_footprint_display_mode: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_footprint_options: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_footprint_tick_size: (a: number, b: number) => void;
    readonly axiuscharts_set_footprint_xy_zoom_enabled: (a: number, b: number) => void;
    readonly axiuscharts_set_grid_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_histogram_data: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => void;
    readonly axiuscharts_set_interval: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_interval_change_locked: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_last_price_label_visible: (a: number, b: number) => void;
    readonly axiuscharts_set_last_price_line_style: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_last_price_line_visible: (a: number, b: number) => void;
    readonly axiuscharts_set_last_price_line_width: (a: number, b: number) => void;
    readonly axiuscharts_set_marker_auto_scale: (a: number, b: number) => void;
    readonly axiuscharts_set_marker_z_order: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_markers: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_max_bars_per_load: (a: number, b: number) => void;
    readonly axiuscharts_set_max_indicator_panes: (a: number, b: number) => void;
    readonly axiuscharts_set_order_line_filled_quantity: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_set_order_line_pnl: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_set_order_line_price: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_set_order_line_price_precision: (a: number, b: number) => void;
    readonly axiuscharts_set_order_line_show_cancel_buttons: (a: number, b: number) => void;
    readonly axiuscharts_set_order_line_status: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly axiuscharts_set_order_line_visible: (a: number, b: number, c: number, d: number) => number;
    readonly axiuscharts_set_order_lines_json: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_price_line_label: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_price_line_price: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_price_line_visible: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_price_scale_margins: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_price_scale_mode: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_price_scale_tick_density: (a: number, b: number) => void;
    readonly axiuscharts_set_replay_cutoff_bar: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_replay_mode: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_replay_options: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_replay_playing: (a: number, b: number) => void;
    readonly axiuscharts_set_selected_drawing_locked: (a: number, b: number) => number;
    readonly axiuscharts_set_selected_drawing_text: (a: number, b: number, c: number) => number;
    readonly axiuscharts_set_selected_drawing_text_alignment: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly axiuscharts_set_selected_drawing_text_style: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly axiuscharts_set_selected_execution_mark: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_selected_fibonacci_levels_json: (a: number, b: number, c: number, d: number) => void;
    readonly axiuscharts_set_selected_rectangle_middle_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly axiuscharts_set_selected_text_border: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly axiuscharts_set_selected_text_fill: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly axiuscharts_set_series_data: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly axiuscharts_set_series_visible: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_study_parameter: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_subpane_separator_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_subpane_separator_hit_area: (a: number, b: number) => void;
    readonly axiuscharts_set_subpane_separator_hover_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly axiuscharts_set_subpane_separator_thickness: (a: number, b: number) => void;
    readonly axiuscharts_set_symbol: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_time_markers: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly axiuscharts_set_visible_range: (a: number, b: number, c: number) => void;
    readonly axiuscharts_set_volume_colors: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_set_volume_visible: (a: number, b: number) => void;
    readonly axiuscharts_start_auto_render: (a: number) => void;
    readonly axiuscharts_stop_auto_render: (a: number) => void;
    readonly axiuscharts_study_count: (a: number) => number;
    readonly axiuscharts_symbol: (a: number, b: number) => void;
    readonly axiuscharts_theme: (a: number, b: number) => void;
    readonly axiuscharts_tick_drawing_caret_blink: (a: number, b: number) => number;
    readonly axiuscharts_timestamp_to_bar_index: (a: number, b: bigint) => bigint;
    readonly axiuscharts_update_indicator_pane: (a: number, b: number, c: number) => void;
    readonly axiuscharts_update_last_bar: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_update_last_bar_series_point: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_update_last_histogram_point: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_update_last_series_point: (a: number, b: number, c: number, d: bigint, e: number) => void;
    readonly axiuscharts_upsert_bar: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_upsert_bar_series_point: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number) => void;
    readonly axiuscharts_upsert_bar_with_footprint: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => void;
    readonly axiuscharts_upsert_histogram_point: (a: number, b: number, c: number, d: bigint, e: number, f: number, g: number, h: number, i: number) => void;
    readonly axiuscharts_upsert_series_point: (a: number, b: number, c: number, d: bigint, e: number) => void;
    readonly axiuscharts_visible_range: (a: number, b: number) => void;
    readonly axiuscharts_zoom_to_range: (a: number, b: bigint, c: bigint) => void;
    readonly chartgroup_add_pane: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly chartgroup_link_panes: (a: number, b: number, c: number) => number;
    readonly chartgroup_new: () => number;
    readonly chartgroup_pane_count: (a: number) => number;
    readonly chartgroup_pane_data_range: (a: number, b: number, c: number) => void;
    readonly chartgroup_pane_interval: (a: number, b: number, c: number) => void;
    readonly chartgroup_pane_symbol: (a: number, b: number, c: number) => void;
    readonly chartgroup_pane_time_range: (a: number, b: number, c: number) => void;
    readonly chartgroup_remove_pane: (a: number, b: number) => number;
    readonly chartgroup_set_auto_link: (a: number, b: number) => void;
    readonly chartgroup_set_sync: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly chartgroup_set_sync_for_link: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly chartgroup_set_sync_for_pane: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly chartgroup_unlink_panes: (a: number, b: number, c: number) => number;
    readonly chartgroup_update_crosshair: (a: number, b: number, c: number, d: number) => number;
    readonly chartgroup_update_data_range: (a: number, b: number, c: number, d: number) => number;
    readonly chartgroup_update_interval: (a: number, b: number, c: number, d: number) => number;
    readonly chartgroup_update_symbol: (a: number, b: number, c: number, d: number) => number;
    readonly chartgroup_update_time_range: (a: number, b: number, c: number, d: number) => number;
    readonly chartworkspace_active_pane_id: (a: number) => number;
    readonly chartworkspace_can_split_active: (a: number) => number;
    readonly chartworkspace_can_split_pane: (a: number, b: number) => number;
    readonly chartworkspace_clear_on_active_pane_change: (a: number) => void;
    readonly chartworkspace_clear_pane_fullscreen: (a: number) => number;
    readonly chartworkspace_dispose: (a: number) => void;
    readonly chartworkspace_fullscreen_pane_id: (a: number) => number;
    readonly chartworkspace_is_pane_fullscreen: (a: number) => number;
    readonly chartworkspace_max_panes: (a: number) => number;
    readonly chartworkspace_new: (a: number, b: number, c: number) => void;
    readonly chartworkspace_pane_host_id: (a: number, b: number, c: number) => void;
    readonly chartworkspace_pane_ids: (a: number) => number;
    readonly chartworkspace_root_pane_id: (a: number) => number;
    readonly chartworkspace_set_active_pane: (a: number, b: number) => number;
    readonly chartworkspace_set_max_panes: (a: number, b: number) => void;
    readonly chartworkspace_set_on_active_pane_change: (a: number, b: number) => void;
    readonly chartworkspace_set_split_divider_active_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly chartworkspace_set_split_divider_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly chartworkspace_set_split_divider_hit_area: (a: number, b: number) => void;
    readonly chartworkspace_set_split_divider_thickness: (a: number, b: number) => void;
    readonly chartworkspace_set_workspace_active_pane_border_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly chartworkspace_set_workspace_active_pane_border_width: (a: number, b: number) => void;
    readonly chartworkspace_set_workspace_pane_background_color: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly chartworkspace_split_active: (a: number, b: number, c: number, d: number) => void;
    readonly chartworkspace_split_pane: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly chartworkspace_toggle_pane_fullscreen: (a: number, b: number) => number;
    readonly __wasm_bindgen_func_elem_458: (a: number, b: number) => void;
    readonly __wasm_bindgen_func_elem_469: (a: number, b: number, c: number) => void;
    readonly __wasm_bindgen_func_elem_2669: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_459: (a: number, b: number, c: number) => void;
    readonly __wasm_bindgen_func_elem_462: (a: number, b: number, c: number) => void;
    readonly __wasm_bindgen_func_elem_467: (a: number, b: number) => void;
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

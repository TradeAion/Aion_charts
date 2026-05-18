/* @ts-self-types="./aion_charts_wasm.d.ts" */

//#region exports

export class Aion_charts {
    constructor() {
        throw new Error('cannot invoke `new` directly');
    }
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(Aion_charts.prototype);
        obj.__wbg_ptr = ptr;
        Aion_chartsFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        Aion_chartsFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_aion_charts_free(ptr, 0);
    }
    /**
     * Get the number of active (pending/working) order lines.
     * @returns {number}
     */
    active_order_line_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_active_order_line_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Add a new area series overlay. Returns the series ID.
     *
     * `line_color_*`: RGBA for the line stroke.
     * `top_color_*`: RGBA for the fill at the line (top of gradient).
     * `bottom_color_*`: RGBA for the fill at the base (bottom of gradient).
     * @param {number} line_color_r
     * @param {number} line_color_g
     * @param {number} line_color_b
     * @param {number} line_color_a
     * @param {number} top_color_r
     * @param {number} top_color_g
     * @param {number} top_color_b
     * @param {number} top_color_a
     * @param {number} bottom_color_r
     * @param {number} bottom_color_g
     * @param {number} bottom_color_b
     * @param {number} bottom_color_a
     * @param {number} line_width
     * @returns {number}
     */
    add_area_series(line_color_r, line_color_g, line_color_b, line_color_a, top_color_r, top_color_g, top_color_b, top_color_a, bottom_color_r, bottom_color_g, bottom_color_b, bottom_color_a, line_width) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_add_area_series(this.__wbg_ptr, line_color_r, line_color_g, line_color_b, line_color_a, top_color_r, top_color_g, top_color_b, top_color_a, bottom_color_r, bottom_color_g, bottom_color_b, bottom_color_a, line_width);
        return ret >>> 0;
    }
    /**
     * Add an external price range that participates in automatic price scaling.
     * @param {number} min_price
     * @param {number} max_price
     * @returns {number}
     */
    add_autoscale_contribution(min_price, max_price) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_add_autoscale_contribution(this.__wbg_ptr, min_price, max_price);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Add a new bar (OHLC) series overlay. Returns the series ID.
     *
     * `up_color_*`: RGBA for bullish bars (close >= open).
     * `down_color_*`: RGBA for bearish bars (close < open).
     * `open_visible`: whether to show the open tick.
     * `thin_bars`: use 1px stems (like reference implementation thinBars option).
     * @param {number} up_color_r
     * @param {number} up_color_g
     * @param {number} up_color_b
     * @param {number} up_color_a
     * @param {number} down_color_r
     * @param {number} down_color_g
     * @param {number} down_color_b
     * @param {number} down_color_a
     * @param {boolean} open_visible
     * @param {boolean} thin_bars
     * @returns {number}
     */
    add_bar_series(up_color_r, up_color_g, up_color_b, up_color_a, down_color_r, down_color_g, down_color_b, down_color_a, open_visible, thin_bars) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(open_visible);
        _assertBoolean(thin_bars);
        const ret = wasm.aion_charts_add_bar_series(this.__wbg_ptr, up_color_r, up_color_g, up_color_b, up_color_a, down_color_r, down_color_g, down_color_b, down_color_a, open_visible, thin_bars);
        return ret >>> 0;
    }
    /**
     * Add a new baseline series overlay. Returns the series ID.
     *
     * A baseline series renders a line with two-tone fill above/below a base value.
     * Above the base: `top_line_color` line + `top_fill_color1`→`top_fill_color2` gradient.
     * Below the base: `bottom_line_color` line + `bottom_fill_color1`→`bottom_fill_color2` gradient.
     * @param {number} base_value
     * @param {number} top_line_r
     * @param {number} top_line_g
     * @param {number} top_line_b
     * @param {number} top_line_a
     * @param {number} bottom_line_r
     * @param {number} bottom_line_g
     * @param {number} bottom_line_b
     * @param {number} bottom_line_a
     * @param {number} top_fill1_r
     * @param {number} top_fill1_g
     * @param {number} top_fill1_b
     * @param {number} top_fill1_a
     * @param {number} top_fill2_r
     * @param {number} top_fill2_g
     * @param {number} top_fill2_b
     * @param {number} top_fill2_a
     * @param {number} bottom_fill1_r
     * @param {number} bottom_fill1_g
     * @param {number} bottom_fill1_b
     * @param {number} bottom_fill1_a
     * @param {number} bottom_fill2_r
     * @param {number} bottom_fill2_g
     * @param {number} bottom_fill2_b
     * @param {number} bottom_fill2_a
     * @param {number} line_width
     * @returns {number}
     */
    add_baseline_series(base_value, top_line_r, top_line_g, top_line_b, top_line_a, bottom_line_r, bottom_line_g, bottom_line_b, bottom_line_a, top_fill1_r, top_fill1_g, top_fill1_b, top_fill1_a, top_fill2_r, top_fill2_g, top_fill2_b, top_fill2_a, bottom_fill1_r, bottom_fill1_g, bottom_fill1_b, bottom_fill1_a, bottom_fill2_r, bottom_fill2_g, bottom_fill2_b, bottom_fill2_a, line_width) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_add_baseline_series(this.__wbg_ptr, base_value, top_line_r, top_line_g, top_line_b, top_line_a, bottom_line_r, bottom_line_g, bottom_line_b, bottom_line_a, top_fill1_r, top_fill1_g, top_fill1_b, top_fill1_a, top_fill2_r, top_fill2_g, top_fill2_b, top_fill2_a, bottom_fill1_r, bottom_fill1_g, bottom_fill1_b, bottom_fill1_a, bottom_fill2_r, bottom_fill2_g, bottom_fill2_b, bottom_fill2_a, line_width);
        return ret >>> 0;
    }
    /**
     * Add a single execution mark to the chart.
     *
     * `side`: "buy" or "sell"
     * `role`: "entry", "scale_in", "scale_out", or "exit"
     *
     * Returns the execution mark ID.
     * @param {string} id
     * @param {bigint} timestamp_ms
     * @param {number} price
     * @param {number} quantity
     * @param {string} side
     * @param {string} role
     */
    add_execution_mark(id, timestamp_ms, price, quantity, side, role) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBigInt(timestamp_ms);
        const ptr1 = passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(role, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        wasm.aion_charts_add_execution_mark(this.__wbg_ptr, ptr0, len0, timestamp_ms, price, quantity, ptr1, len1, ptr2, len2);
    }
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
     * @param {string} id
     * @param {bigint} timestamp_ms
     * @param {number} price
     * @param {number} quantity
     * @param {string} side
     * @param {string} role
     * @param {string} order_type
     * @param {string} label
     * @param {string} group_id
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {number} realized_pnl
     */
    add_execution_mark_full(id, timestamp_ms, price, quantity, side, role, order_type, label, group_id, color_r, color_g, color_b, color_a, realized_pnl) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBigInt(timestamp_ms);
        const ptr1 = passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(role, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passStringToWasm0(order_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passStringToWasm0(label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passStringToWasm0(group_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len5 = WASM_VECTOR_LEN;
        wasm.aion_charts_add_execution_mark_full(this.__wbg_ptr, ptr0, len0, timestamp_ms, price, quantity, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, color_r, color_g, color_b, color_a, realized_pnl);
    }
    /**
     * Add a new histogram series overlay. Returns the series ID.
     *
     * `color_*`: RGBA for the default bar color.
     * `base`: the base value (bars extend from base to data value).
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {number} base
     * @returns {number}
     */
    add_histogram_series(color_r, color_g, color_b, color_a, base) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_add_histogram_series(this.__wbg_ptr, color_r, color_g, color_b, color_a, base);
        return ret >>> 0;
    }
    /**
     * Create a new indicator sub-pane below the main chart.
     * Returns the pane ID. The indicator type should be one of: "rsi", "stochastic", "atr".
     * The study must already be created with `create_study()`.
     * @param {number} study_id
     * @param {string} indicator_type
     * @param {number} height_css
     * @returns {number}
     */
    add_indicator_pane(study_id, indicator_type, height_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(study_id);
        const ptr0 = passStringToWasm0(indicator_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_add_indicator_pane(this.__wbg_ptr, study_id, ptr0, len0, height_css);
        return ret >>> 0;
    }
    /**
     * Add a new line series overlay. Returns the series ID.
     *
     * Default color is default blue (#2962FF). Use RGBA [0.0–1.0].
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {number} line_width
     * @param {string} line_style
     * @returns {number}
     */
    add_line_series(color_r, color_g, color_b, color_a, line_width, line_style) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(line_style, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_add_line_series(this.__wbg_ptr, color_r, color_g, color_b, color_a, line_width, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Add a marker to a series at the specified bar index.
     *
     * `shape`: "arrow_up", "arrow_down", "circle", "square"
     * `position`: "above_bar", "below_bar", "at_price"
     * `price`: Used only when position is "at_price"
     *
     * Returns the marker ID.
     * @param {number} series_id
     * @param {number} bar_index
     * @param {string} shape
     * @param {string} position
     * @param {number} price
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {number} size
     * @param {string} text
     * @returns {number}
     */
    add_marker(series_id, bar_index, shape, position, price, color_r, color_g, color_b, color_a, size, text) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(series_id);
        _assertNum(bar_index);
        const ptr0 = passStringToWasm0(shape, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(position, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_add_marker(this.__wbg_ptr, series_id, bar_index, ptr0, len0, ptr1, len1, price, color_r, color_g, color_b, color_a, size, ptr2, len2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Add a marker anchored by timestamp instead of mutable bar index.
     *
     * The timestamp is kept as the canonical render anchor. The resolved bar
     * index is only used as a fallback and for above/below bar price placement.
     * @param {number} series_id
     * @param {bigint} timestamp
     * @param {string} shape
     * @param {string} position
     * @param {number} price
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {number} size
     * @param {string} text
     * @returns {number}
     */
    add_marker_at_time(series_id, timestamp, shape, position, price, color_r, color_g, color_b, color_a, size, text) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(series_id);
        _assertBigInt(timestamp);
        const ptr0 = passStringToWasm0(shape, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(position, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_add_marker_at_time(this.__wbg_ptr, series_id, timestamp, ptr0, len0, ptr1, len1, price, color_r, color_g, color_b, color_a, size, ptr2, len2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Get the allowed interval list. Returns an empty array when all intervals are allowed.
     * @returns {Array<any>}
     */
    allowed_intervals() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_allowed_intervals(this.__wbg_ptr);
        return ret;
    }
    /**
     * Append a single bar to the data array. Used for real-time streaming.
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     * @param {number} volume
     */
    append_bar(timestamp, open, high, low, close, volume) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_append_bar(this.__wbg_ptr, timestamp, open, high, low, close, volume);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Append a single point to a bar (OHLC) overlay series.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     */
    append_bar_series_point(id, timestamp, open, high, low, close) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_append_bar_series_point(this.__wbg_ptr, id, timestamp, open, high, low, close);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Append multiple bars in one call. The arrays must be equal length and strictly increasing.
     * @param {Float64Array} open
     * @param {Float64Array} high
     * @param {Float64Array} low
     * @param {Float64Array} close
     * @param {Float64Array} volume
     * @param {BigUint64Array} timestamps
     */
    append_bars(open, high, low, close, volume, timestamps) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArrayF64ToWasm0(open, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(high, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(low, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(close, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(volume, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len5 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_append_bars(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Append a single point to a histogram overlay series.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} value
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     */
    append_histogram_point(id, timestamp, value, color_r, color_g, color_b, color_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_append_histogram_point(this.__wbg_ptr, id, timestamp, value, color_r, color_g, color_b, color_a);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Append a single point to a line/area/baseline overlay series.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} value
     */
    append_series_point(id, timestamp, value) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_append_series_point(this.__wbg_ptr, id, timestamp, value);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Apply partial options update at runtime.
     *
     * Accepts the same options shape as `create_chart()`. Only provided
     * fields are updated; omitted fields keep their current values.
     * @param {any} options
     */
    apply_options(options) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_apply_options(this.__wbg_ptr, options);
    }
    /**
     * Convert a bar index to a timestamp (in milliseconds).
     * Returns 0 if the bar index is out of bounds.
     * @param {number} bar_index
     * @returns {bigint}
     */
    bar_index_to_timestamp(bar_index) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(bar_index);
        const ret = wasm.aion_charts_bar_index_to_timestamp(this.__wbg_ptr, bar_index);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @returns {boolean}
     */
    begin_selected_drawing_text_edit() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_begin_selected_drawing_text_edit(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return whether another indicator pane can be created under the current cap.
     * @returns {boolean}
     */
    can_add_indicator_pane() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_can_add_indicator_pane(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return whether a historical load of the given size would be accepted.
     * @param {number} bar_count
     * @returns {boolean}
     */
    can_load_bar_count(bar_count) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(bar_count);
        const ret = wasm.aion_charts_can_load_bar_count(this.__wbg_ptr, bar_count);
        return ret !== 0;
    }
    /**
     * Return whether the chart can switch from the current interval to the requested one.
     * @param {string} interval
     * @returns {boolean}
     */
    can_set_interval(interval) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_can_set_interval(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Cancel the drawing currently being created.
     */
    cancel_drawing() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_cancel_drawing(this.__wbg_ptr);
    }
    /**
     * Clear all markers for all series.
     */
    clear_all_markers() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_all_markers(this.__wbg_ptr);
    }
    /**
     * Clear the interval allowlist.
     */
    clear_allowed_intervals() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_allowed_intervals(this.__wbg_ptr);
    }
    /**
     * Remove all external autoscale contributions.
     */
    clear_autoscale_contributions() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_autoscale_contributions(this.__wbg_ptr);
    }
    /**
     * Hide crosshair immediately.
     */
    clear_crosshair() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_crosshair(this.__wbg_ptr);
    }
    /**
     * Remove all drawings.
     */
    clear_drawings() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_drawings(this.__wbg_ptr);
    }
    /**
     * Clear all execution marks.
     */
    clear_execution_marks() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_execution_marks(this.__wbg_ptr);
    }
    /**
     * Clear all footprint data.
     */
    clear_footprint_data() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_footprint_data(this.__wbg_ptr);
    }
    /**
     * Clear all markers for a series.
     * @param {number} series_id
     */
    clear_markers(series_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(series_id);
        wasm.aion_charts_clear_markers(this.__wbg_ptr, series_id);
    }
    /**
     * Remove all order lines.
     */
    clear_order_lines() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_order_lines(this.__wbg_ptr);
    }
    /**
     * Clear the selected execution mark.
     */
    clear_selected_execution_mark() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_clear_selected_execution_mark(this.__wbg_ptr);
    }
    /**
     * Complete the drawing currently being created, when the active tool uses
     * explicit completion.
     * @returns {boolean}
     */
    complete_drawing() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_complete_drawing(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Create a new Aion_charts instance with a full options object.
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
     * @param {any} container
     * @param {any} options
     * @returns {Promise<Aion_charts>}
     */
    static create_chart(container, options) {
        const ret = wasm.aion_charts_create_chart(container, options);
        return ret;
    }
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
     * @param {string} id
     * @param {number} price
     * @param {string} order_type
     * @param {string} side
     * @param {number} quantity
     * @param {boolean} modifiable
     * @param {boolean} cancellable
     * @returns {string}
     */
    create_order_line(id, price, order_type, side, quantity, modifiable, cancellable) {
        let deferred4_0;
        let deferred4_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(order_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len2 = WASM_VECTOR_LEN;
            _assertBoolean(modifiable);
            _assertBoolean(cancellable);
            const ret = wasm.aion_charts_create_order_line(this.__wbg_ptr, ptr0, len0, price, ptr1, len1, ptr2, len2, quantity, modifiable, cancellable);
            deferred4_0 = ret[0];
            deferred4_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
        }
    }
    /**
     * Create an order line with full options.
     *
     * `order_type`: "limit", "stop", "stop_limit", "take_profit", "stop_loss", "trailing_stop"
     * `side`: "buy" or "sell"
     * `status`: "pending", "working", "partial", "filled", "cancelled"
     * `color_*`: Custom color override (pass all zeros to use default)
     * `custom_label`: Custom label text (empty string for auto-generated)
     * @param {string} id
     * @param {number} price
     * @param {string} order_type
     * @param {string} side
     * @param {string} status
     * @param {number} quantity
     * @param {number} filled_quantity
     * @param {boolean} modifiable
     * @param {boolean} cancellable
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {string} custom_label
     * @param {string} linked_position_id
     * @returns {string}
     */
    create_order_line_full(id, price, order_type, side, status, quantity, filled_quantity, modifiable, cancellable, color_r, color_g, color_b, color_a, custom_label, linked_position_id) {
        let deferred7_0;
        let deferred7_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ptr1 = passStringToWasm0(order_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            const ptr2 = passStringToWasm0(side, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len2 = WASM_VECTOR_LEN;
            const ptr3 = passStringToWasm0(status, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len3 = WASM_VECTOR_LEN;
            _assertBoolean(modifiable);
            _assertBoolean(cancellable);
            const ptr4 = passStringToWasm0(custom_label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len4 = WASM_VECTOR_LEN;
            const ptr5 = passStringToWasm0(linked_position_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len5 = WASM_VECTOR_LEN;
            const ret = wasm.aion_charts_create_order_line_full(this.__wbg_ptr, ptr0, len0, price, ptr1, len1, ptr2, len2, ptr3, len3, quantity, filled_quantity, modifiable, cancellable, color_r, color_g, color_b, color_a, ptr4, len4, ptr5, len5);
            deferred7_0 = ret[0];
            deferred7_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred7_0, deferred7_1, 1);
        }
    }
    /**
     * Create a new price line at the specified price level. Returns the price line ID.
     *
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     * @param {number} price
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     * @param {number} line_width
     * @param {string} line_style
     * @param {boolean} draggable
     * @returns {number}
     */
    create_price_line(price, color_r, color_g, color_b, color_a, line_width, line_style, draggable) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(line_style, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(draggable);
        const ret = wasm.aion_charts_create_price_line(this.__wbg_ptr, price, color_r, color_g, color_b, color_a, line_width, ptr0, len0, draggable);
        return ret >>> 0;
    }
    /**
     * Create a new study instance. Returns the study ID, or 0 if the type is unknown.
     *
     * Supported types: "sma", "ema", "rsi", "macd".
     * @param {string} study_type
     * @returns {number}
     */
    create_study(study_type) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(study_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_create_study(this.__wbg_ptr, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Create with a specific renderer backend (`auto`, `webgpu`, `canvas2d`).
     * @param {string} container_id
     * @param {string} renderer
     * @returns {Promise<Aion_charts>}
     */
    static create_with(container_id, renderer) {
        const ptr0 = passStringToWasm0(container_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(renderer, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_create_with(ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @returns {string}
     */
    crosshair_mode() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_crosshair_mode(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Returns `[active, x, y, bar_index, price]`.
     * @returns {Float64Array}
     */
    crosshair_state() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_crosshair_state(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * Data timestamp range as `[from_ts, to_ts]`, or empty if no bars.
     * @returns {Float64Array}
     */
    data_range() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_data_range(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    demo_mode() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_demo_mode(this.__wbg_ptr);
    }
    /**
     * Load synthetic demo data dedicated for footprint chart mode.
     *
     * This generates OHLCV bars plus aligned per-bar footprint levels and
     * switches the chart type to `footprint`.
     */
    demo_mode_footprint() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_demo_mode_footprint(this.__wbg_ptr);
    }
    /**
     * Deselect all drawings.
     */
    deselect_drawings() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_deselect_drawings(this.__wbg_ptr);
    }
    /**
     * Dispose: remove all event listeners, disconnect resize observer, and clean up resources.
     *
     * IMPORTANT: Call this when destroying the chart to prevent memory leaks.
     * Event listeners attached to DOM elements will keep the closures alive
     * even after Aion_charts is dropped, unless explicitly removed.
     */
    dispose() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_dispose(this.__wbg_ptr);
    }
    /**
     * Drag a separator to resize adjacent panes.
     * `separator_idx` is 0 for separator between main and first subpane.
     * `delta_y` is positive for moving down, negative for up.
     * This uses the PaneManager's coordinated height algorithm.
     * @param {number} separator_idx
     * @param {number} delta_y
     */
    drag_pane_separator(separator_idx, delta_y) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(separator_idx);
        wasm.aion_charts_drag_pane_separator(this.__wbg_ptr, separator_idx, delta_y);
    }
    /**
     * Get the number of drawings.
     * @returns {number}
     */
    drawing_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_drawing_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {boolean} cancel
     * @returns {boolean}
     */
    end_selected_drawing_text_edit(cancel) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(cancel);
        const ret = wasm.aion_charts_end_selected_drawing_text_edit(this.__wbg_ptr, cancel);
        return ret !== 0;
    }
    /**
     * Get the number of execution marks.
     * @returns {number}
     */
    execution_mark_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_execution_mark_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Expand the currently rendered execution cluster for a given leader ID.
     * @param {string} leader_id
     * @returns {string[]}
     */
    expand_execution_cluster(leader_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(leader_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_expand_execution_cluster(this.__wbg_ptr, ptr0, len0);
        var v2 = getArrayJsValueFromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v2;
    }
    /**
     * Export all drawings (main pane + indicator subpanes) as JSON.
     *
     * The returned string is versioned and can be stored externally.
     * @returns {string}
     */
    export_drawings() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_export_drawings(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Export the pane, price axis, and time axis canvases as one PNG data URL.
     * @returns {string}
     */
    export_image_data_url() {
        let deferred2_0;
        let deferred2_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_export_image_data_url(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Export the main chart pane canvases as a PNG data URL.
     * @returns {string}
     */
    export_pane_image_data_url() {
        let deferred2_0;
        let deferred2_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_export_pane_image_data_url(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Export a full chart persistence snapshot (styles + viewport + pane layout + drawings).
     *
     * `layout_id` is an optional caller-defined identifier to help external storage routing.
     * @param {string | null} [layout_id]
     * @returns {string}
     */
    export_persistence_state(layout_id) {
        let deferred2_0;
        let deferred2_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            var ptr0 = isLikeNone(layout_id) ? 0 : passStringToWasm0(layout_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            const ret = wasm.aion_charts_export_persistence_state(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Return whether auto-scroll is currently enabled.
     * @returns {boolean}
     */
    get_auto_scroll() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_auto_scroll(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get all available chart types as a comma-separated string.
     * @returns {string}
     */
    static get_available_chart_types() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.aion_charts_get_available_chart_types();
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the current chart type as a string.
     * @returns {string}
     */
    get_chart_type() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_get_chart_type(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get current CSS variables as a JS object.
     * @returns {any}
     */
    get_css_variables() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_css_variables(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get chart-wide drawing lock summary across the main pane and all subpanes.
     * @returns {string}
     */
    get_drawings_lock_summary_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_get_drawings_lock_summary_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the chart-wide execution label mode.
     * @returns {string}
     */
    get_execution_label_mode() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_get_execution_label_mode(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Whether execution mark text labels are currently rendered.
     * @returns {boolean}
     */
    get_execution_mark_text_visible() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_execution_mark_text_visible(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Serialize all execution marks to JSON.
     * @returns {string}
     */
    get_execution_marks_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_get_execution_marks_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Whether realized P&L text is currently rendered for eligible execution marks.
     * @returns {boolean}
     */
    get_execution_pnl_visible() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_execution_pnl_visible(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return whether footprint pane two-axis zoom (X+Y) is enabled.
     * @returns {boolean}
     */
    get_footprint_xy_zoom_enabled() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_footprint_xy_zoom_enabled(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Serialize all order lines to JSON.
     * @returns {string}
     */
    get_order_lines_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_get_order_lines_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the currently selected drawing's text/alignment inspector payload as JSON.
     * Returns `"null"` when no drawing is selected.
     * @returns {string}
     */
    get_selected_drawing_info_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_get_selected_drawing_info_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the currently selected execution mark ID, or null if none.
     * @returns {string | undefined}
     */
    get_selected_execution_mark() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_selected_execution_mark(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]).slice();
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * Get study output data as a JS object { timestamps: BigUint64Array, values: Float64Array }.
     * Returns null if the study or output index doesn't exist.
     * @param {number} id
     * @param {number} output_index
     * @returns {any}
     */
    get_study_output(id, output_index) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertNum(output_index);
        const ret = wasm.aion_charts_get_study_output(this.__wbg_ptr, id, output_index);
        return ret;
    }
    /**
     * @returns {Array<any>}
     */
    static get_supported_renderers() {
        const ret = wasm.aion_charts_get_supported_renderers();
        return ret;
    }
    /**
     * Whether volume bars are currently visible in the main pane.
     * @returns {boolean}
     */
    get_volume_visible() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_get_volume_visible(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Hit-test rendered series markers at pane CSS coordinates.
     *
     * Returns `null` when no rendered marker contains the point.
     * @param {number} x_css
     * @param {number} y_css
     * @returns {any}
     */
    hit_test_marker(x_css, y_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_hit_test_marker(this.__wbg_ptr, x_css, y_css);
        return ret;
    }
    /**
     * Restore all drawings (main pane + indicator subpanes) from JSON.
     *
     * Existing drawings are replaced atomically. Unknown subpane IDs in the payload are ignored.
     * @param {string} json
     */
    import_drawings(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_import_drawings(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Restore a full chart persistence snapshot (styles + viewport + pane layout + drawings).
     * @param {string} json
     */
    import_persistence_state(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_import_persistence_state(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Attach a compiled indicator program to the current chart.
     * Returns a runtime instance ID, or 0 on failure.
     * @param {number} indicator_id
     * @param {string} opts_json
     * @returns {number}
     */
    indicator_attach(indicator_id, opts_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(indicator_id);
        const ptr0 = passStringToWasm0(opts_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_indicator_attach(this.__wbg_ptr, indicator_id, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * Compile user indicator source into the internal IR program artifact.
     * Returns: `{ indicatorId, diagnostics }`.
     * @param {string} source
     * @param {string} meta_json
     * @returns {any}
     */
    indicator_compile(source, meta_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(meta_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_indicator_compile(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * Detach an indicator runtime instance.
     * @param {number} instance_id
     * @returns {boolean}
     */
    indicator_detach(instance_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(instance_id);
        const ret = wasm.aion_charts_indicator_detach(this.__wbg_ptr, instance_id);
        return ret !== 0;
    }
    /**
     * Drain and return pending runtime events from indicator instances.
     *
     * Returns an array of objects:
     * `{ instanceId, indicatorId, type, code, message, barIndex }`
     * @returns {any}
     */
    indicator_drain_events() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_indicator_drain_events(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get diagnostics for a compiled indicator.
     * @param {number} indicator_id
     * @returns {any}
     */
    indicator_get_diagnostics(indicator_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(indicator_id);
        const ret = wasm.aion_charts_indicator_get_diagnostics(this.__wbg_ptr, indicator_id);
        return ret;
    }
    /**
     * Get compile-time-discovered MTF request templates from a compiled indicator.
     * @param {number} indicator_id
     * @returns {any}
     */
    indicator_get_mtf_requests(indicator_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(indicator_id);
        const ret = wasm.aion_charts_indicator_get_mtf_requests(this.__wbg_ptr, indicator_id);
        return ret;
    }
    /**
     * Get runtime stats for an indicator instance.
     * @param {number} instance_id
     * @returns {any}
     */
    indicator_get_stats(instance_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(instance_id);
        const ret = wasm.aion_charts_indicator_get_stats(this.__wbg_ptr, instance_id);
        return ret;
    }
    /**
     * List attached indicator instances.
     * @returns {any}
     */
    indicator_list() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_indicator_list(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the number of indicator sub-panes.
     * @returns {number}
     */
    indicator_pane_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_indicator_pane_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Enable or disable a runtime indicator instance.
     * @param {number} instance_id
     * @param {boolean} enabled
     * @returns {boolean}
     */
    indicator_set_enabled(instance_id, enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(instance_id);
        _assertBoolean(enabled);
        const ret = wasm.aion_charts_indicator_set_enabled(this.__wbg_ptr, instance_id, enabled);
        return ret !== 0;
    }
    /**
     * Set runtime inputs for an attached indicator instance.
     * @param {number} instance_id
     * @param {string} inputs_json
     * @returns {boolean}
     */
    indicator_set_inputs(instance_id, inputs_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(instance_id);
        const ptr0 = passStringToWasm0(inputs_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_indicator_set_inputs(this.__wbg_ptr, instance_id, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Load backend-resolved MTF series snapshots into the runtime resolver cache.
     *
     * JSON payload:
     * `{ clear?: bool, series: [{ symbol, chartTimeframe, requestId?, timeframe, field, mode?, points: [...] }] }`
     * @param {string} snapshot_json
     * @returns {boolean}
     */
    indicator_set_mtf_snapshot(snapshot_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(snapshot_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_indicator_set_mtf_snapshot(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Privileged runtime-only resource limit override for an indicator instance.
     * @param {number} instance_id
     * @param {string} limits_json
     * @returns {boolean}
     */
    indicator_set_resource_limits(instance_id, limits_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(instance_id);
        const ptr0 = passStringToWasm0(limits_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_indicator_set_resource_limits(this.__wbg_ptr, instance_id, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @returns {string}
     */
    interval() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_interval(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Return whether interval changes are locked.
     * @returns {boolean}
     */
    interval_change_locked() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_interval_change_locked(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Returns whether auto-render is currently active.
     * @returns {boolean}
     */
    is_auto_render() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_is_auto_render(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Return whether a specific interval is permitted by the current guardrails.
     * @param {string} interval
     * @returns {boolean}
     */
    is_interval_allowed(interval) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_is_interval_allowed(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Whether marker visual size participates in automatic price scaling.
     * @returns {boolean}
     */
    marker_auto_scale() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_marker_auto_scale(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get the current global marker z-order.
     * @returns {string}
     */
    marker_z_order() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_marker_z_order(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the maximum historical bar count allowed in a single load. Returns 0 when uncapped.
     * @returns {number}
     */
    max_bars_per_load() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_max_bars_per_load(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the maximum indicator sub-pane count. Returns 0 when uncapped.
     * @returns {number}
     */
    max_indicator_panes() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_max_indicator_panes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Remove a specific event callback.
     * @param {string} event
     * @param {Function} callback
     */
    off(event, callback) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(event, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_off(this.__wbg_ptr, ptr0, len0, callback);
    }
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
     * @param {string} event
     * @param {Function} callback
     */
    on(event, callback) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(event, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_on(this.__wbg_ptr, ptr0, len0, callback);
    }
    /**
     * Handle host keyboard input for native drawing text editing.
     *
     * Returns true when the key was consumed by the drawing manager. Hosts
     * should call this from focused chart keydown handlers and prevent their
     * own shortcuts when this returns true.
     * @param {string} key
     * @param {boolean} ctrl
     * @param {boolean} shift
     * @param {boolean} alt
     * @returns {boolean}
     */
    on_key_down(key, ctrl, shift, alt) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(ctrl);
        _assertBoolean(shift);
        _assertBoolean(alt);
        const ret = wasm.aion_charts_on_key_down(this.__wbg_ptr, ptr0, len0, ctrl, shift, alt);
        return ret !== 0;
    }
    /**
     * Register a one-shot event callback (auto-removes after first call).
     * @param {string} event
     * @param {Function} callback
     */
    once(event, callback) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(event, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_once(this.__wbg_ptr, ptr0, len0, callback);
    }
    /**
     * Get the number of order lines.
     * @returns {number}
     */
    order_line_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_order_line_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the number of price lines.
     * @returns {number}
     */
    price_line_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_price_line_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Project a timestamp/price coordinate into current pane CSS coordinates.
     * @param {bigint} timestamp_ms
     * @param {number} price
     * @returns {any}
     */
    project_point(timestamp_ms, price) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp_ms);
        const ret = wasm.aion_charts_project_point(this.__wbg_ptr, timestamp_ms, price);
        return ret;
    }
    /**
     * Remove all scale (measurement) drawings.
     */
    remove_all_scale_drawings() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_remove_all_scale_drawings(this.__wbg_ptr);
    }
    /**
     * Remove a previously registered autoscale contribution.
     * @param {number} id
     * @returns {boolean}
     */
    remove_autoscale_contribution(id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ret = wasm.aion_charts_remove_autoscale_contribution(this.__wbg_ptr, id);
        return ret !== 0;
    }
    /**
     * Remove an execution mark by ID.
     * @param {string} id
     * @returns {boolean}
     */
    remove_execution_mark(id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_remove_execution_mark(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Remove an indicator sub-pane by ID.
     * @param {number} pane_id
     * @returns {boolean}
     */
    remove_indicator_pane(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.aion_charts_remove_indicator_pane(this.__wbg_ptr, pane_id);
        return ret !== 0;
    }
    /**
     * Remove a specific marker from a series.
     * @param {number} series_id
     * @param {number} marker_id
     * @returns {boolean}
     */
    remove_marker(series_id, marker_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(series_id);
        _assertNum(marker_id);
        const ret = wasm.aion_charts_remove_marker(this.__wbg_ptr, series_id, marker_id);
        return ret !== 0;
    }
    /**
     * Remove an order line by ID.
     * @param {string} id
     * @returns {boolean}
     */
    remove_order_line(id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_remove_order_line(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Remove all order lines with a specific status.
     *
     * `status`: "pending", "working", "partial", "filled", "cancelled", "rejected", "expired"
     * @param {string} status
     */
    remove_order_lines_by_status(status) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(status, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_remove_order_lines_by_status(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Remove a price line by ID.
     * @param {number} id
     * @returns {boolean}
     */
    remove_price_line(id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ret = wasm.aion_charts_remove_price_line(this.__wbg_ptr, id);
        return ret !== 0;
    }
    /**
     * Remove the currently selected drawing.
     */
    remove_selected_drawing() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_remove_selected_drawing(this.__wbg_ptr);
    }
    /**
     * Remove a series by ID.
     * @param {number} id
     * @returns {boolean}
     */
    remove_series(id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ret = wasm.aion_charts_remove_series(this.__wbg_ptr, id);
        return ret !== 0;
    }
    /**
     * Remove a study by ID.
     * @param {number} id
     * @returns {boolean}
     */
    remove_study(id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ret = wasm.aion_charts_remove_study(this.__wbg_ptr, id);
        return ret !== 0;
    }
    /**
     * Render one frame. Call from requestAnimationFrame.
     */
    render() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_render(this.__wbg_ptr);
    }
    /**
     * @returns {string}
     */
    renderer_name() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_renderer_name(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get replay cutoff bar index, or -1 when unavailable.
     * @returns {bigint}
     */
    replay_cutoff_bar() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_replay_cutoff_bar(this.__wbg_ptr);
        return ret;
    }
    /**
     * Whether replay mode is currently active.
     * @returns {boolean}
     */
    replay_mode() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_replay_mode(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Get current replay runtime options.
     * @returns {any}
     */
    replay_options() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_replay_options(this.__wbg_ptr);
        return ret;
    }
    /**
     * Whether replay playback is currently running.
     * @returns {boolean}
     */
    replay_playing() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_replay_playing(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Step replay backward by 1 bar.
     */
    replay_step_back() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_replay_step_back(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Step replay forward by 1 bar.
     */
    replay_step_forward() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_replay_step_forward(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Reset the main chart viewport.
     *
     * Supported modes:
     * - `"default"`: restore the recent-bars default view with a small right gap
     * - `"fit_all"`: show the full dataset with a small right gap
     *
     * Unknown or omitted modes fall back to `"default"`.
     * @param {string | null} [mode]
     */
    reset_viewport(mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        var ptr0 = isLikeNone(mode) ? 0 : passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_reset_viewport(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Get the number of overlay series.
     * @returns {number}
     */
    series_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_series_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Lock or unlock every drawing on the chart across the main pane and all subpanes.
     * @param {boolean} locked
     * @returns {boolean}
     */
    set_all_drawings_locked(locked) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(locked);
        const ret = wasm.aion_charts_set_all_drawings_locked(this.__wbg_ptr, locked);
        return ret !== 0;
    }
    /**
     * Replace the allowed interval list. Pass an empty array to remove the allowlist.
     * @param {Array<any>} intervals
     */
    set_allowed_intervals(intervals) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_set_allowed_intervals(this.__wbg_ptr, intervals);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {boolean} enabled
     */
    set_auto_scroll(enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        wasm.aion_charts_set_auto_scroll(this.__wbg_ptr, enabled);
    }
    /**
     * Set the axis border (separator line) color (RGBA 0-1).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_axis_border_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_axis_border_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Show or hide the axis border line. Layout is unaffected.
     * @param {boolean} visible
     */
    set_axis_border_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_axis_border_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set the axis label text color (RGBA 0-1).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_axis_text_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_axis_text_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Show or hide axis tick marks. Layout is unaffected.
     * @param {boolean} visible
     */
    set_axis_ticks_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_axis_ticks_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set chart and axis background color (RGBA 0-1).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_background_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_background_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set data for a bar (OHLC) series.
     * All arrays must be the same length.
     * @param {number} id
     * @param {BigUint64Array} timestamps
     * @param {Float64Array} open
     * @param {Float64Array} high
     * @param {Float64Array} low
     * @param {Float64Array} close
     */
    set_bar_series_data(id, timestamps, open, high, low, close) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ptr0 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(open, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(high, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(low, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(close, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_bar_series_data(this.__wbg_ptr, id, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set the bar width ratio (0.0-1.0, default 0.8).
     * @param {number} ratio
     */
    set_bar_width_ratio(ratio) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_bar_width_ratio(this.__wbg_ptr, ratio);
    }
    /**
     * Set bearish (down) candle colors: body fill and wick/border.
     * @param {number} fill_r
     * @param {number} fill_g
     * @param {number} fill_b
     * @param {number} fill_a
     * @param {number} wick_r
     * @param {number} wick_g
     * @param {number} wick_b
     * @param {number} wick_a
     */
    set_bearish_color(fill_r, fill_g, fill_b, fill_a, wick_r, wick_g, wick_b, wick_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_bearish_color(this.__wbg_ptr, fill_r, fill_g, fill_b, fill_a, wick_r, wick_g, wick_b, wick_a);
    }
    /**
     * Set bullish (up) candle colors: body fill and wick/border.
     * @param {number} fill_r
     * @param {number} fill_g
     * @param {number} fill_b
     * @param {number} fill_a
     * @param {number} wick_r
     * @param {number} wick_g
     * @param {number} wick_b
     * @param {number} wick_a
     */
    set_bullish_color(fill_r, fill_g, fill_b, fill_a, wick_r, wick_g, wick_b, wick_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_bullish_color(this.__wbg_ptr, fill_r, fill_g, fill_b, fill_a, wick_r, wick_g, wick_b, wick_a);
    }
    /**
     * Set the main chart type.
     *
     * Accepted values: "candlestick", "candles", "ohlc", "bars", "line", "area",
     * "heikin_ashi", "ha", "footprint", "fp", "order_flow".
     * @param {string} chart_type
     */
    set_chart_type(chart_type) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(chart_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_chart_type(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set the shared crosshair label text color (applies to both axes).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_crosshair_label_text_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_crosshair_label_text_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set crosshair axis-label visibility.
     * `target`: "vert", "horz", or "both".
     * @param {string} target
     * @param {boolean} visible
     */
    set_crosshair_label_visible(target, visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(visible);
        wasm.aion_charts_set_crosshair_label_visible(this.__wbg_ptr, ptr0, len0, visible);
    }
    /**
     * Set crosshair line color.
     * `target`: "vert", "horz", or "both".
     * @param {string} target
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_crosshair_line_color(target, r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_line_color(this.__wbg_ptr, ptr0, len0, r, g, b, a);
    }
    /**
     * Set crosshair label background color.
     * `target`: "vert", "horz", or "both".
     * @param {string} target
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_crosshair_line_label_bg_color(target, r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_line_label_bg_color(this.__wbg_ptr, ptr0, len0, r, g, b, a);
    }
    /**
     * Set crosshair line style.
     * `target`: "vert", "horz", or "both".
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     * @param {string} target
     * @param {string} line_style
     */
    set_crosshair_line_style(target, line_style) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(line_style, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_line_style(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * Set crosshair line visibility.
     * `target`: "vert", "horz", or "both".
     * @param {string} target
     * @param {boolean} visible
     */
    set_crosshair_line_visible(target, visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(visible);
        wasm.aion_charts_set_crosshair_line_visible(this.__wbg_ptr, ptr0, len0, visible);
    }
    /**
     * Set crosshair line width in CSS pixels.
     * `target`: "vert", "horz", or "both".
     * @param {string} target
     * @param {number} width
     */
    set_crosshair_line_width(target, width) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(target, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_line_width(this.__wbg_ptr, ptr0, len0, width);
    }
    /**
     * Set crosshair mode: "normal" or "magnet_ohlc".
     *
     * Legacy alias:
     * - "magnet" is accepted and treated as "magnet_ohlc".
     * @param {string} mode
     */
    set_crosshair_mode(mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_mode(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set crosshair state for synchronized groups.
     * @param {boolean} active
     * @param {number} x
     * @param {number} y
     * @param {number} bar_index
     * @param {number} price
     * @param {string} mode
     */
    set_crosshair_state(active, x, y, bar_index, price, mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(active);
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_state(this.__wbg_ptr, active, x, y, bar_index, price, ptr0, len0);
    }
    /**
     * Set crosshair state for synchronized panes by semantic values only.
     * This keeps the target pane snapped to its own viewport/grid.
     * @param {boolean} active
     * @param {number} bar_index
     * @param {number} price
     * @param {string} mode
     */
    set_crosshair_sync_state(active, bar_index, price, mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(active);
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_crosshair_sync_state(this.__wbg_ptr, active, bar_index, price, ptr0, len0);
    }
    /**
     * @param {Float64Array} open
     * @param {Float64Array} high
     * @param {Float64Array} low
     * @param {Float64Array} close
     * @param {Float64Array} volume
     * @param {BigUint64Array} timestamps
     */
    set_data_arrays(open, high, low, close, volume, timestamps) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArrayF64ToWasm0(open, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(high, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(low, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(close, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(volume, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len5 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_data_arrays(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Atomically load OHLCV bars plus aligned footprint data from typed arrays.
     *
     * This is the canonical historical footprint initialization path for
     * production integrations. `level_offsets` is bar-aligned and must have
     * length `bars.len() + 1`; sparse bars use empty ranges.
     * @param {Float64Array} open
     * @param {Float64Array} high
     * @param {Float64Array} low
     * @param {Float64Array} close
     * @param {Float64Array} volume
     * @param {BigUint64Array} timestamps
     * @param {Uint32Array} level_offsets
     * @param {Float64Array} prices
     * @param {Float64Array} bid_volumes
     * @param {Float64Array} ask_volumes
     */
    set_data_with_footprint_arrays(open, high, low, close, volume, timestamps, level_offsets, prices, bid_volumes, ask_volumes) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArrayF64ToWasm0(open, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(high, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(low, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(close, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(volume, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len5 = WASM_VECTOR_LEN;
        const ptr6 = passArray32ToWasm0(level_offsets, wasm.__wbindgen_malloc);
        const len6 = WASM_VECTOR_LEN;
        const ptr7 = passArrayF64ToWasm0(prices, wasm.__wbindgen_malloc);
        const len7 = WASM_VECTOR_LEN;
        const ptr8 = passArrayF64ToWasm0(bid_volumes, wasm.__wbindgen_malloc);
        const len8 = WASM_VECTOR_LEN;
        const ptr9 = passArrayF64ToWasm0(ask_volumes, wasm.__wbindgen_malloc);
        const len9 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_data_with_footprint_arrays(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5, ptr6, len6, ptr7, len7, ptr8, len8, ptr9, len9);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Atomically load OHLCV bars plus footprint levels from JSON.
     *
     * Expected canonical format:
     * `[{"timestamp": 1710000000000, "open": 100.0, "high": 101.0, "low": 99.5, "close": 100.5, "volume": 2500.0, "levels": [{"price": 99.5, "bid": 120.0, "ask": 80.0}]}]`
     *
     * Also accepts `{ "bars": [...] }` as the top-level wrapper and the
     * existing `bid_volume` / `bidVolume` / `ask_volume` / `askVolume` level aliases.
     * @param {string} json
     */
    set_data_with_footprint_json(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_data_with_footprint_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set active drawing tool: "none", "trend_line", "rectangle", "fibonacci",
     * "scale", "brush", "horizontal_line", "vertical_line", "ray", "path".
     * @param {string} tool
     */
    set_drawing_tool(tool) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(tool, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_drawing_tool(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set the CSS-pixel clustering threshold for dense execution marks.
     * @param {number} threshold_px
     */
    set_execution_cluster_threshold_px(threshold_px) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_execution_cluster_threshold_px(this.__wbg_ptr, threshold_px);
    }
    /**
     * Set the chart-wide execution label mode.
     *
     * Accepted values: `"side"`, `"role"`, `"side_and_role"` (case-insensitive).
     * @param {string} mode
     */
    set_execution_label_mode(mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_execution_label_mode(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Show/hide execution mark text labels.
     * @param {boolean} visible
     */
    set_execution_mark_text_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_execution_mark_text_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set multiple execution marks at once (replaces existing).
     *
     * `mark_data` is a flat array of execution mark data with stride 6:
     * [timestamp_ms, price, quantity, side_idx, role_idx, ...]
     * where side_idx: 0=buy, 1=sell
     * and role_idx: 0=entry, 1=scale_in, 2=scale_out, 3=exit
     *
     * `ids` is an array of string IDs (must match mark_data length / 5).
     * @param {string[]} ids
     * @param {Float64Array} mark_data
     */
    set_execution_marks(ids, mark_data) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArrayJsValueToWasm0(ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(mark_data, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_execution_marks(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
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
     * @param {string} json
     */
    set_execution_marks_json(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_execution_marks_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Show or hide realized P&L text for eligible execution marks.
     * @param {boolean} visible
     */
    set_execution_pnl_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_execution_pnl_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set the font family for axis labels.
     * @param {string} family
     */
    set_font_family(family) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(family, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_font_family(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set the font size for axis labels (in CSS pixels).
     * @param {number} size
     */
    set_font_size(size) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_font_size(this.__wbg_ptr, size);
    }
    /**
     * Set footprint (order-flow) data for a specific bar.
     *
     * `bar_index`: the bar index in the main data array.
     * `prices`: price levels (ascending order).
     * `bid_volumes`: bid volume at each price level.
     * `ask_volumes`: ask volume at each price level.
     *
     * All three arrays must be the same length.
     * @param {number} bar_index
     * @param {Float64Array} prices
     * @param {Float64Array} bid_volumes
     * @param {Float64Array} ask_volumes
     */
    set_footprint_bar(bar_index, prices, bid_volumes, ask_volumes) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(bar_index);
        const ptr0 = passArrayF64ToWasm0(prices, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(bid_volumes, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(ask_volumes, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_footprint_bar(this.__wbg_ptr, bar_index, ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {Uint32Array} bar_indices
     * @param {Uint32Array} level_offsets
     * @param {Float64Array} prices
     * @param {Float64Array} bid_volumes
     * @param {Float64Array} ask_volumes
     */
    set_footprint_data_arrays(bar_indices, level_offsets, prices, bid_volumes, ask_volumes) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArray32ToWasm0(bar_indices, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray32ToWasm0(level_offsets, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(prices, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(bid_volumes, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(ask_volumes, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_footprint_data_arrays(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {string} json
     */
    set_footprint_data_json(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_footprint_data_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set footprint display mode.
     * Accepted values: "bid_ask", "delta", "volume", "delta_profile", "volume_profile".
     * @param {string} mode
     */
    set_footprint_display_mode(mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_footprint_display_mode(this.__wbg_ptr, ptr0, len0);
    }
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
     * @param {string} json
     */
    set_footprint_options(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_footprint_options(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set footprint tick size (price granularity). Pass 0.0 for auto-detection.
     * @param {number} tick_size
     */
    set_footprint_tick_size(tick_size) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_footprint_tick_size(this.__wbg_ptr, tick_size);
    }
    /**
     * Enable/disable footprint pane two-axis zoom (X+Y) for wheel and pinch.
     * @param {boolean} enabled
     */
    set_footprint_xy_zoom_enabled(enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        wasm.aion_charts_set_footprint_xy_zoom_enabled(this.__wbg_ptr, enabled);
    }
    /**
     * Set the grid line color (RGBA 0-1).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_grid_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_grid_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set data for a histogram series. `values` and `timestamps` must be same length.
     * Per-bar colors are optional — pass empty arrays to use the series default color.
     * @param {number} id
     * @param {Float64Array} values
     * @param {BigUint64Array} timestamps
     * @param {Float32Array} colors_r
     * @param {Float32Array} colors_g
     * @param {Float32Array} colors_b
     * @param {Float32Array} colors_a
     */
    set_histogram_data(id, values, timestamps, colors_r, colors_g, colors_b, colors_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ptr0 = passArrayF64ToWasm0(values, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF32ToWasm0(colors_r, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF32ToWasm0(colors_g, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF32ToWasm0(colors_b, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArrayF32ToWasm0(colors_a, wasm.__wbindgen_malloc);
        const len5 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_histogram_data(this.__wbg_ptr, id, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} interval
     */
    set_interval(interval) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_interval(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Lock or unlock interval changes away from the current interval.
     * @param {boolean} locked
     */
    set_interval_change_locked(locked) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(locked);
        const ret = wasm.aion_charts_set_interval_change_locked(this.__wbg_ptr, locked);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set live last-price label visibility on the Y axis.
     * @param {boolean} visible
     */
    set_last_price_label_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_last_price_label_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set live last-price line style.
     * `line_style`: "solid", "dotted", "dashed", "large_dashed", "sparse_dotted".
     * @param {string} line_style
     */
    set_last_price_line_style(line_style) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(line_style, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_last_price_line_style(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set live last-price line visibility.
     * @param {boolean} visible
     */
    set_last_price_line_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_last_price_line_visible(this.__wbg_ptr, visible);
    }
    /**
     * Set live last-price line width in CSS pixels.
     * @param {number} width
     */
    set_last_price_line_width(width) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_last_price_line_width(this.__wbg_ptr, width);
    }
    /**
     * Include marker visual size in automatic price scaling.
     * @param {boolean} auto_scale
     */
    set_marker_auto_scale(auto_scale) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(auto_scale);
        wasm.aion_charts_set_marker_auto_scale(this.__wbg_ptr, auto_scale);
    }
    /**
     * Set the global marker z-order: "normal", "aboveSeries", or "top".
     * @param {string} z_order
     */
    set_marker_z_order(z_order) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(z_order, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_marker_z_order(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set multiple markers for a series at once (replaces existing).
     * `marker_data` is a flat array: [bar_index, shape_idx, position_idx, price, r, g, b, a, size, ...]
     * where shape_idx: 0=arrowUp, 1=arrowDown, 2=circle, 3=square
     * and position_idx: 0=aboveBar, 1=belowBar, 2=atPrice
     * @param {number} series_id
     * @param {Float64Array} marker_data
     */
    set_markers(series_id, marker_data) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(series_id);
        const ptr0 = passArrayF64ToWasm0(marker_data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_markers(this.__wbg_ptr, series_id, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set the maximum historical bar count allowed in a single load. Pass 0 to disable the cap.
     * @param {number} max_bars
     */
    set_max_bars_per_load(max_bars) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(max_bars);
        wasm.aion_charts_set_max_bars_per_load(this.__wbg_ptr, max_bars);
    }
    /**
     * Set the maximum indicator sub-pane count. Pass 0 to disable the cap.
     * @param {number} max_panes
     */
    set_max_indicator_panes(max_panes) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(max_panes);
        wasm.aion_charts_set_max_indicator_panes(this.__wbg_ptr, max_panes);
    }
    /**
     * Update the filled quantity of an order line (for partial fills).
     * @param {string} id
     * @param {number} filled
     * @returns {boolean}
     */
    set_order_line_filled_quantity(id, filled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_order_line_filled_quantity(this.__wbg_ptr, ptr0, len0, filled);
        return ret !== 0;
    }
    /**
     * Update the non-accent text color for an existing order line.
     * @param {string} id
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     * @returns {boolean}
     */
    set_order_line_label_text_color(id, r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_order_line_label_text_color(this.__wbg_ptr, ptr0, len0, r, g, b, a);
        return ret !== 0;
    }
    /**
     * Update the live PNL displayed on an existing order line.
     * @param {string} id
     * @param {number} pnl
     * @returns {boolean}
     */
    set_order_line_pnl(id, pnl) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_order_line_pnl(this.__wbg_ptr, ptr0, len0, pnl);
        return ret !== 0;
    }
    /**
     * Update the price of an existing order line.
     * @param {string} id
     * @param {number} price
     * @returns {boolean}
     */
    set_order_line_price(id, price) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_order_line_price(this.__wbg_ptr, ptr0, len0, price);
        return ret !== 0;
    }
    /**
     * Set the price precision (decimal places) for order line labels.
     * @param {number} precision
     */
    set_order_line_price_precision(precision) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(precision);
        wasm.aion_charts_set_order_line_price_precision(this.__wbg_ptr, precision);
    }
    /**
     * Set whether to show cancel buttons on order lines.
     * @param {boolean} show
     */
    set_order_line_show_cancel_buttons(show) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(show);
        wasm.aion_charts_set_order_line_show_cancel_buttons(this.__wbg_ptr, show);
    }
    /**
     * Update the status of an order line.
     *
     * `status`: "pending", "working", "partial", "filled", "cancelled", "rejected", "expired"
     * @param {string} id
     * @param {string} status
     * @returns {boolean}
     */
    set_order_line_status(id, status) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(status, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_order_line_status(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Set whether an order line is visible.
     * @param {string} id
     * @param {boolean} visible
     * @returns {boolean}
     */
    set_order_line_visible(id, visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(visible);
        const ret = wasm.aion_charts_set_order_line_visible(this.__wbg_ptr, ptr0, len0, visible);
        return ret !== 0;
    }
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
     * @param {string} json
     */
    set_order_lines_json(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_order_lines_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set the label text of a price line. Empty string uses formatted price.
     * @param {number} id
     * @param {string} label
     */
    set_price_line_label(id, label) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ptr0 = passStringToWasm0(label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_price_line_label(this.__wbg_ptr, id, ptr0, len0);
    }
    /**
     * Update the price of an existing price line.
     * @param {number} id
     * @param {number} price
     */
    set_price_line_price(id, price) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        wasm.aion_charts_set_price_line_price(this.__wbg_ptr, id, price);
    }
    /**
     * Set whether a price line is visible.
     * @param {number} id
     * @param {boolean} visible
     */
    set_price_line_visible(id, visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBoolean(visible);
        wasm.aion_charts_set_price_line_visible(this.__wbg_ptr, id, visible);
    }
    /**
     * Set the price scale margins (top and bottom as fractions 0.0-1.0).
     * Default is 0.2 top, 0.1 bottom.
     * @param {number} top
     * @param {number} bottom
     */
    set_price_scale_margins(top, bottom) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_price_scale_margins(this.__wbg_ptr, top, bottom);
    }
    /**
     * Set the price scale mode.
     *
     * Accepted values: "normal", "logarithmic" (or "log"), "percentage" (or "percent"),
     * "indexed_to_100" (or "indexedTo100", "indexed").
     * @param {string} mode
     */
    set_price_scale_mode(mode) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_price_scale_mode(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set the price scale tick mark density multiplier.
     * @param {number} density
     */
    set_price_scale_tick_density(density) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_price_scale_tick_density(this.__wbg_ptr, density);
    }
    /**
     * Set replay cutoff bar (inclusive right-edge trim).
     * @param {number} index
     */
    set_replay_cutoff_bar(index) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(index);
        const ret = wasm.aion_charts_set_replay_cutoff_bar(this.__wbg_ptr, index);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Enter/exit market replay mode.
     * @param {boolean} enabled
     */
    set_replay_mode(enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        const ret = wasm.aion_charts_set_replay_mode(this.__wbg_ptr, enabled);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Update replay runtime options.
     * @param {any} options
     */
    set_replay_options(options) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_set_replay_options(this.__wbg_ptr, options);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Start/pause replay playback.
     * @param {boolean} playing
     */
    set_replay_playing(playing) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(playing);
        wasm.aion_charts_set_replay_playing(this.__wbg_ptr, playing);
    }
    /**
     * Lock or unlock the currently selected drawing.
     * @param {boolean} locked
     * @returns {boolean}
     */
    set_selected_drawing_locked(locked) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(locked);
        const ret = wasm.aion_charts_set_selected_drawing_locked(this.__wbg_ptr, locked);
        return ret !== 0;
    }
    /**
     * Set inline text on the currently selected drawing.
     * @param {string} text
     * @returns {boolean}
     */
    set_selected_drawing_text(text) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_selected_drawing_text(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Set text alignment on the currently selected drawing.
     * @param {string} horizontal
     * @param {string} vertical
     * @returns {boolean}
     */
    set_selected_drawing_text_alignment(horizontal, vertical) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(horizontal, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(vertical, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_selected_drawing_text_alignment(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Set font size / italic / color override on the currently selected drawing label.
     * @param {number} font_size
     * @param {boolean} italic
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     * @param {boolean} follow_drawing_color
     * @returns {boolean}
     */
    set_selected_drawing_text_style(font_size, italic, r, g, b, a, follow_drawing_color) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(italic);
        _assertBoolean(follow_drawing_color);
        const ret = wasm.aion_charts_set_selected_drawing_text_style(this.__wbg_ptr, font_size, italic, r, g, b, a, follow_drawing_color);
        return ret !== 0;
    }
    /**
     * Set the selected execution mark ID (shows selected-trade execution locators).
     * Pass empty string or null to deselect.
     * @param {string | null} [mark_id]
     */
    set_selected_execution_mark(mark_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        var ptr0 = isLikeNone(mark_id) ? 0 : passStringToWasm0(mark_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_selected_execution_mark(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Replace the currently selected Fibonacci drawing's levels from JSON.
     * Input shape: `[{"ratio":0.5,"label":"Mid"}, ...]`
     * @param {string} json
     * @returns {boolean}
     */
    set_selected_fibonacci_levels_json(json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_selected_fibonacci_levels_json(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] !== 0;
    }
    /**
     * Toggle / configure the optional horizontal middle line on the currently
     * selected Rectangle drawing (platform-style midline).
     *
     * `dash_on`/`dash_off` ≤ 0 means a solid line. Returns `false` when the
     * current selection is not a Rectangle, or when nothing is selected.
     * @param {boolean} enabled
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     * @param {number} line_width
     * @param {number} dash_on
     * @param {number} dash_off
     * @returns {boolean}
     */
    set_selected_rectangle_middle_line(enabled, r, g, b, a, line_width, dash_on, dash_off) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        const ret = wasm.aion_charts_set_selected_rectangle_middle_line(this.__wbg_ptr, enabled, r, g, b, a, line_width, dash_on, dash_off);
        return ret !== 0;
    }
    /**
     * Update the border on the currently selected Text drawing. The color,
     * width, and dash are always written so toggling `enabled` off and back
     * on preserves the user's last picks.
     *
     * `dash_on`/`dash_off` ≤ 0 means a solid line. Returns `false` when the
     * current selection is not a Text drawing, or when nothing is selected.
     * @param {boolean} enabled
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     * @param {number} line_width
     * @param {number} dash_on
     * @param {number} dash_off
     * @returns {boolean}
     */
    set_selected_text_border(enabled, r, g, b, a, line_width, dash_on, dash_off) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        const ret = wasm.aion_charts_set_selected_text_border(this.__wbg_ptr, enabled, r, g, b, a, line_width, dash_on, dash_off);
        return ret !== 0;
    }
    /**
     * Update the background fill on the currently selected Text drawing.
     * The color (including alpha) is always written so toggling `enabled`
     * off and back on preserves the user's last picked color.
     * @param {boolean} enabled
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     * @returns {boolean}
     */
    set_selected_text_fill(enabled, r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        const ret = wasm.aion_charts_set_selected_text_fill(this.__wbg_ptr, enabled, r, g, b, a);
        return ret !== 0;
    }
    /**
     * Set data for a line series. `values` and `timestamps` must be same length.
     * @param {number} id
     * @param {Float64Array} values
     * @param {BigUint64Array} timestamps
     */
    set_series_data(id, values, timestamps) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ptr0 = passArrayF64ToWasm0(values, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_series_data(this.__wbg_ptr, id, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Show or hide a series.
     * @param {number} id
     * @param {boolean} visible
     */
    set_series_visible(id, visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBoolean(visible);
        wasm.aion_charts_set_series_visible(this.__wbg_ptr, id, visible);
    }
    /**
     * Set a study parameter (e.g., "period" for SMA/EMA, "fast_period" for MACD).
     * The study will be recalculated on the next render.
     * @param {number} id
     * @param {string} key
     * @param {number} value
     */
    set_study_parameter(id, key, value) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        const ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_study_parameter(this.__wbg_ptr, id, ptr0, len0, value);
    }
    /**
     * Set indicator sub-pane separator line color (RGBA, 0.0-1.0).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_subpane_separator_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_subpane_separator_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set indicator sub-pane separator drag hit-area thickness (CSS px).
     * @param {number} hit_area_css
     */
    set_subpane_separator_hit_area(hit_area_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_subpane_separator_hit_area(this.__wbg_ptr, hit_area_css);
    }
    /**
     * Set indicator sub-pane separator hover/active color (RGBA, 0.0-1.0).
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_subpane_separator_hover_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_subpane_separator_hover_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * Set indicator sub-pane separator visible line thickness (CSS px).
     * @param {number} thickness_css
     */
    set_subpane_separator_thickness(thickness_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_subpane_separator_thickness(this.__wbg_ptr, thickness_css);
    }
    /**
     * @param {string} symbol
     */
    set_symbol(symbol) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(symbol, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.aion_charts_set_symbol(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * Set multiple timestamp-anchored markers for a series at once.
     *
     * `timestamps` contains one timestamp per marker. `marker_data` is a flat
     * array with stride 8: [shape_idx, position_idx, price, r, g, b, a, size, ...].
     * @param {number} series_id
     * @param {BigUint64Array} timestamps
     * @param {Float64Array} marker_data
     */
    set_time_markers(series_id, timestamps, marker_data) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(series_id);
        const ptr0 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(marker_data, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_set_time_markers(this.__wbg_ptr, series_id, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Set visible bar range using fractional bar indices.
     * @param {number} start
     * @param {number} end
     */
    set_visible_range(start, end) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_visible_range(this.__wbg_ptr, start, end);
    }
    /**
     * Set volume bar colors: bullish and bearish.
     * @param {number} up_r
     * @param {number} up_g
     * @param {number} up_b
     * @param {number} up_a
     * @param {number} down_r
     * @param {number} down_g
     * @param {number} down_b
     * @param {number} down_a
     */
    set_volume_colors(up_r, up_g, up_b, up_a, down_r, down_g, down_b, down_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_set_volume_colors(this.__wbg_ptr, up_r, up_g, up_b, up_a, down_r, down_g, down_b, down_a);
    }
    /**
     * Show/hide volume bars in the main pane.
     * @param {boolean} visible
     */
    set_volume_visible(visible) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(visible);
        wasm.aion_charts_set_volume_visible(this.__wbg_ptr, visible);
    }
    /**
     * Start the auto-render RAF loop.
     */
    start_auto_render() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_start_auto_render(this.__wbg_ptr);
    }
    /**
     * Stop the auto-render RAF loop. Caller must manually call render().
     */
    stop_auto_render() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.aion_charts_stop_auto_render(this.__wbg_ptr);
    }
    /**
     * Get the number of studies.
     * @returns {number}
     */
    study_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_study_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    symbol() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_symbol(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the current theme preset name ("dark", "light", or "custom").
     * @returns {string}
     */
    theme() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.aion_charts_theme(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Advance the text-edit caret blink phase. The host should call this on
     * each animation frame (e.g. inside the rAF loop) passing `performance.now()`
     * in milliseconds. Returns true when the caret visibility flipped, in which
     * case the canvas is automatically marked dirty for repaint. When no text
     * edit is active this is a cheap no-op.
     * @param {number} now_ms
     * @returns {boolean}
     */
    tick_drawing_caret_blink(now_ms) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_tick_drawing_caret_blink(this.__wbg_ptr, now_ms);
        return ret !== 0;
    }
    /**
     * Convert a timestamp (in milliseconds) to a bar index.
     * Returns -1 if the timestamp is before all bars.
     * @param {bigint} timestamp_ms
     * @returns {bigint}
     */
    timestamp_to_bar_index(timestamp_ms) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp_ms);
        const ret = wasm.aion_charts_timestamp_to_bar_index(this.__wbg_ptr, timestamp_ms);
        return ret;
    }
    /**
     * Update indicator sub-pane data from a study.
     * @param {number} pane_id
     * @param {number} study_id
     */
    update_indicator_pane(pane_id, study_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        _assertNum(study_id);
        wasm.aion_charts_update_indicator_pane(this.__wbg_ptr, pane_id, study_id);
    }
    /**
     * Update the last bar in the data array. Used for real-time tick updates.
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     * @param {number} volume
     */
    update_last_bar(timestamp, open, high, low, close, volume) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_update_last_bar(this.__wbg_ptr, timestamp, open, high, low, close, volume);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Update the last point in a bar (OHLC) overlay series.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     */
    update_last_bar_series_point(id, timestamp, open, high, low, close) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_update_last_bar_series_point(this.__wbg_ptr, id, timestamp, open, high, low, close);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Update the last point in a histogram overlay series.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} value
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     */
    update_last_histogram_point(id, timestamp, value, color_r, color_g, color_b, color_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_update_last_histogram_point(this.__wbg_ptr, id, timestamp, value, color_r, color_g, color_b, color_a);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Update the last point in a line/area/baseline overlay series.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} value
     */
    update_last_series_point(id, timestamp, value) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_update_last_series_point(this.__wbg_ptr, id, timestamp, value);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * compatibility-style main series update semantics:
     * update last bar if timestamp matches, append if timestamp is newer.
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     * @param {number} volume
     */
    upsert_bar(timestamp, open, high, low, close, volume) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_upsert_bar(this.__wbg_ptr, timestamp, open, high, low, close, volume);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * compatibility-style update semantics for OHLC bar overlays:
     * update last point if timestamp matches, append if timestamp is newer.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     */
    upsert_bar_series_point(id, timestamp, open, high, low, close) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_upsert_bar_series_point(this.__wbg_ptr, id, timestamp, open, high, low, close);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Upsert a main bar and atomically set its footprint levels.
     *
     * This is the preferred real-time API for external order-flow feeds:
     * one call updates OHLCV + footprint for the same logical bar.
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     * @param {number} volume
     * @param {Float64Array} prices
     * @param {Float64Array} bid_volumes
     * @param {Float64Array} ask_volumes
     */
    upsert_bar_with_footprint(timestamp, open, high, low, close, volume, prices, bid_volumes, ask_volumes) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp);
        const ptr0 = passArrayF64ToWasm0(prices, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(bid_volumes, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(ask_volumes, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_upsert_bar_with_footprint(this.__wbg_ptr, timestamp, open, high, low, close, volume, ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Upsert multiple bars in one call. Existing latest timestamp is updated; newer bars append.
     * @param {Float64Array} open
     * @param {Float64Array} high
     * @param {Float64Array} low
     * @param {Float64Array} close
     * @param {Float64Array} volume
     * @param {BigUint64Array} timestamps
     */
    upsert_bars(open, high, low, close, volume, timestamps) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArrayF64ToWasm0(open, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(high, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(low, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(close, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(volume, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len5 = WASM_VECTOR_LEN;
        const ret = wasm.aion_charts_upsert_bars(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * compatibility-style update semantics for histogram overlays:
     * update last point if timestamp matches, append if timestamp is newer.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} value
     * @param {number} color_r
     * @param {number} color_g
     * @param {number} color_b
     * @param {number} color_a
     */
    upsert_histogram_point(id, timestamp, value, color_r, color_g, color_b, color_a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_upsert_histogram_point(this.__wbg_ptr, id, timestamp, value, color_r, color_g, color_b, color_a);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * compatibility-style update semantics for line/area/baseline overlays:
     * update last point if timestamp matches, append if timestamp is newer.
     * @param {number} id
     * @param {bigint} timestamp
     * @param {number} value
     */
    upsert_series_point(id, timestamp, value) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(id);
        _assertBigInt(timestamp);
        const ret = wasm.aion_charts_upsert_series_point(this.__wbg_ptr, id, timestamp, value);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {Float64Array}
     */
    visible_range() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.aion_charts_visible_range(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @param {bigint} start
     * @param {bigint} end
     */
    zoom_to_range(start, end) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(start);
        _assertBigInt(end);
        wasm.aion_charts_zoom_to_range(this.__wbg_ptr, start, end);
    }
}
if (Symbol.dispose) Aion_charts.prototype[Symbol.dispose] = Aion_charts.prototype.free;

export class ChartGroup {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ChartGroupFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_chartgroup_free(ptr, 0);
    }
    /**
     * @param {string} symbol
     * @param {string} interval
     * @returns {number}
     */
    add_pane(symbol, interval) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(symbol, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.chartgroup_add_pane(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {number} a
     * @param {number} b
     * @returns {boolean}
     */
    link_panes(a, b) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(a);
        _assertNum(b);
        const ret = wasm.chartgroup_link_panes(this.__wbg_ptr, a, b);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.chartgroup_new();
        this.__wbg_ptr = ret >>> 0;
        ChartGroupFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {number}
     */
    pane_count() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartgroup_pane_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Returns `[from_timestamp, to_timestamp]`, or empty if unavailable.
     * @param {number} pane_id
     * @returns {Float64Array}
     */
    pane_data_range(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.chartgroup_pane_data_range(this.__wbg_ptr, pane_id);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @param {number} pane_id
     * @returns {string}
     */
    pane_interval(pane_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            _assertNum(pane_id);
            const ret = wasm.chartgroup_pane_interval(this.__wbg_ptr, pane_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} pane_id
     * @returns {string}
     */
    pane_symbol(pane_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            _assertNum(pane_id);
            const ret = wasm.chartgroup_pane_symbol(this.__wbg_ptr, pane_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Returns `[start_bar, end_bar]`, or empty if pane is missing.
     * @param {number} pane_id
     * @returns {Float64Array}
     */
    pane_time_range(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.chartgroup_pane_time_range(this.__wbg_ptr, pane_id);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @param {number} pane_id
     * @returns {boolean}
     */
    remove_pane(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.chartgroup_remove_pane(this.__wbg_ptr, pane_id);
        return ret !== 0;
    }
    /**
     * @param {boolean} enabled
     */
    set_auto_link(enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBoolean(enabled);
        wasm.chartgroup_set_auto_link(this.__wbg_ptr, enabled);
    }
    /**
     * @param {string} feature
     * @param {boolean} enabled
     */
    set_sync(feature, enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(feature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(enabled);
        const ret = wasm.chartgroup_set_sync(this.__wbg_ptr, ptr0, len0, enabled);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} pane_a
     * @param {number} pane_b
     * @param {string} feature
     * @param {boolean} enabled
     */
    set_sync_for_link(pane_a, pane_b, feature, enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_a);
        _assertNum(pane_b);
        const ptr0 = passStringToWasm0(feature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(enabled);
        const ret = wasm.chartgroup_set_sync_for_link(this.__wbg_ptr, pane_a, pane_b, ptr0, len0, enabled);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} pane_id
     * @param {string} feature
     * @param {boolean} enabled
     */
    set_sync_for_pane(pane_id, feature, enabled) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ptr0 = passStringToWasm0(feature, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        _assertBoolean(enabled);
        const ret = wasm.chartgroup_set_sync_for_pane(this.__wbg_ptr, pane_id, ptr0, len0, enabled);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} a
     * @param {number} b
     * @returns {boolean}
     */
    unlink_panes(a, b) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(a);
        _assertNum(b);
        const ret = wasm.chartgroup_unlink_panes(this.__wbg_ptr, a, b);
        return ret !== 0;
    }
    /**
     * `crosshair` format: `[active, x, y, bar_index, price, magnet]`.
     * `magnet`: 0 = normal, 1 = OHLC magnet.
     * @param {number} source
     * @param {Float64Array} crosshair
     * @returns {Array<any>}
     */
    update_crosshair(source, crosshair) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(source);
        const ptr0 = passArrayF64ToWasm0(crosshair, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chartgroup_update_crosshair(this.__wbg_ptr, source, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} source
     * @param {number} from_timestamp
     * @param {number} to_timestamp
     * @returns {Array<any>}
     */
    update_data_range(source, from_timestamp, to_timestamp) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(source);
        const ret = wasm.chartgroup_update_data_range(this.__wbg_ptr, source, from_timestamp, to_timestamp);
        return ret;
    }
    /**
     * @param {number} source
     * @param {string} interval
     * @returns {Array<any>}
     */
    update_interval(source, interval) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(source);
        const ptr0 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chartgroup_update_interval(this.__wbg_ptr, source, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} source
     * @param {string} symbol
     * @returns {Array<any>}
     */
    update_symbol(source, symbol) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(source);
        const ptr0 = passStringToWasm0(symbol, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chartgroup_update_symbol(this.__wbg_ptr, source, ptr0, len0);
        return ret;
    }
    /**
     * @param {number} source
     * @param {number} start_bar
     * @param {number} end_bar
     * @returns {Array<any>}
     */
    update_time_range(source, start_bar, end_bar) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(source);
        const ret = wasm.chartgroup_update_time_range(this.__wbg_ptr, source, start_bar, end_bar);
        return ret;
    }
}
if (Symbol.dispose) ChartGroup.prototype[Symbol.dispose] = ChartGroup.prototype.free;

export class ChartWorkspace {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ChartWorkspaceFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_chartworkspace_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    active_pane_id() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_active_pane_id(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {boolean}
     */
    can_split_active() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_can_split_active(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} pane_id
     * @returns {boolean}
     */
    can_split_pane(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.chartworkspace_can_split_pane(this.__wbg_ptr, pane_id);
        return ret !== 0;
    }
    clear_on_active_pane_change() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_clear_on_active_pane_change(this.__wbg_ptr);
    }
    /**
     * @returns {boolean}
     */
    clear_pane_fullscreen() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_clear_pane_fullscreen(this.__wbg_ptr);
        return ret !== 0;
    }
    dispose() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_dispose(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    fullscreen_pane_id() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_fullscreen_pane_id(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {boolean}
     */
    is_pane_fullscreen() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_is_pane_fullscreen(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    max_panes() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_max_panes(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {string} container_id
     */
    constructor(container_id) {
        const ptr0 = passStringToWasm0(container_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chartworkspace_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        ChartWorkspaceFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} pane_id
     * @returns {string}
     */
    pane_host_id(pane_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            _assertNum(pane_id);
            const ret = wasm.chartworkspace_pane_host_id(this.__wbg_ptr, pane_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Array<any>}
     */
    pane_ids() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_pane_ids(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    root_pane_id() {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ret = wasm.chartworkspace_root_pane_id(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {number} pane_id
     * @returns {boolean}
     */
    set_active_pane(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.chartworkspace_set_active_pane(this.__wbg_ptr, pane_id);
        return ret !== 0;
    }
    /**
     * @param {number} max_panes
     */
    set_max_panes(max_panes) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(max_panes);
        wasm.chartworkspace_set_max_panes(this.__wbg_ptr, max_panes);
    }
    /**
     * @param {Function} callback
     */
    set_on_active_pane_change(callback) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_on_active_pane_change(this.__wbg_ptr, callback);
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_split_divider_active_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_split_divider_active_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_split_divider_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_split_divider_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * @param {number} hit_area_css
     */
    set_split_divider_hit_area(hit_area_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_split_divider_hit_area(this.__wbg_ptr, hit_area_css);
    }
    /**
     * @param {number} thickness_css
     */
    set_split_divider_thickness(thickness_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_split_divider_thickness(this.__wbg_ptr, thickness_css);
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_workspace_active_pane_border_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_workspace_active_pane_border_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * @param {number} width_css
     */
    set_workspace_active_pane_border_width(width_css) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_workspace_active_pane_border_width(this.__wbg_ptr, width_css);
    }
    /**
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    set_workspace_pane_background_color(r, g, b, a) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        wasm.chartworkspace_set_workspace_pane_background_color(this.__wbg_ptr, r, g, b, a);
    }
    /**
     * @param {string} direction
     * @returns {number}
     */
    split_active(direction) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(direction, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chartworkspace_split_active(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} pane_id
     * @param {string} direction
     * @returns {number}
     */
    split_pane(pane_id, direction) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ptr0 = passStringToWasm0(direction, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.chartworkspace_split_pane(this.__wbg_ptr, pane_id, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} pane_id
     * @returns {boolean}
     */
    toggle_pane_fullscreen(pane_id) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(pane_id);
        const ret = wasm.chartworkspace_toggle_pane_fullscreen(this.__wbg_ptr, pane_id);
        return ret !== 0;
    }
}
if (Symbol.dispose) ChartWorkspace.prototype[Symbol.dispose] = ChartWorkspace.prototype.free;

export class IndicatorWorkerRuntime {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        IndicatorWorkerRuntimeFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_indicatorworkerruntime_free(ptr, 0);
    }
    /**
     * @param {number} indicator_id
     * @param {string} opts_json
     * @returns {number}
     */
    attach(indicator_id, opts_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertNum(indicator_id);
        const ptr0 = passStringToWasm0(opts_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.indicatorworkerruntime_attach(this.__wbg_ptr, indicator_id, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {string} source
     * @param {string} meta_json
     * @returns {any}
     */
    compile(source, meta_json) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(meta_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.indicatorworkerruntime_compile(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret;
    }
    /**
     * @returns {string}
     */
    drain_events_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.indicatorworkerruntime_drain_events_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    draw_instructions_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
            _assertNum(this.__wbg_ptr);
            const ret = wasm.indicatorworkerruntime_draw_instructions_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    constructor() {
        const ret = wasm.indicatorworkerruntime_new();
        this.__wbg_ptr = ret >>> 0;
        IndicatorWorkerRuntimeFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {string} symbol
     * @param {string} interval
     */
    set_context(symbol, interval) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passStringToWasm0(symbol, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(interval, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.indicatorworkerruntime_set_context(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {Float64Array} open
     * @param {Float64Array} high
     * @param {Float64Array} low
     * @param {Float64Array} close
     * @param {Float64Array} volume
     * @param {BigUint64Array} timestamps
     */
    set_data_arrays(open, high, low, close, volume, timestamps) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        const ptr0 = passArrayF64ToWasm0(open, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(high, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(low, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ptr3 = passArrayF64ToWasm0(close, wasm.__wbindgen_malloc);
        const len3 = WASM_VECTOR_LEN;
        const ptr4 = passArrayF64ToWasm0(volume, wasm.__wbindgen_malloc);
        const len4 = WASM_VECTOR_LEN;
        const ptr5 = passArray64ToWasm0(timestamps, wasm.__wbindgen_malloc);
        const len5 = WASM_VECTOR_LEN;
        const ret = wasm.indicatorworkerruntime_set_data_arrays(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, ptr5, len5);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {bigint} timestamp
     * @param {number} open
     * @param {number} high
     * @param {number} low
     * @param {number} close
     * @param {number} volume
     */
    upsert_bar(timestamp, open, high, low, close, volume) {
        if (this.__wbg_ptr == 0) throw new Error('Attempt to use a moved value');
        _assertNum(this.__wbg_ptr);
        _assertBigInt(timestamp);
        const ret = wasm.indicatorworkerruntime_upsert_bar(this.__wbg_ptr, timestamp, open, high, low, close, volume);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
}
if (Symbol.dispose) IndicatorWorkerRuntime.prototype[Symbol.dispose] = IndicatorWorkerRuntime.prototype.free;

//#endregion

//#region wasm imports

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Window_7b2011a6368164ef: function() { return logError(function (arg0) {
            const ret = arg0.Window;
            return ret;
        }, arguments); },
        __wbg_WorkerGlobalScope_4bddbcb12b3f5a28: function() { return logError(function (arg0) {
            const ret = arg0.WorkerGlobalScope;
            return ret;
        }, arguments); },
        __wbg___wbindgen_boolean_get_bbbb1c18aa2f5e25: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            if (!isLikeNone(ret)) {
                _assertBoolean(ret);
            }
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_0bc8482c6e3508ae: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_0095a73b8b156f76: function(arg0) {
            const ret = typeof(arg0) === 'function';
            _assertBoolean(ret);
            return ret;
        },
        __wbg___wbindgen_is_null_ac34f5003991759a: function(arg0) {
            const ret = arg0 === null;
            _assertBoolean(ret);
            return ret;
        },
        __wbg___wbindgen_is_object_5ae8e5880f2c1fbd: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            _assertBoolean(ret);
            return ret;
        },
        __wbg___wbindgen_is_string_cd444516edc5b180: function(arg0) {
            const ret = typeof(arg0) === 'string';
            _assertBoolean(ret);
            return ret;
        },
        __wbg___wbindgen_is_undefined_9e4d92534c42d778: function(arg0) {
            const ret = arg0 === undefined;
            _assertBoolean(ret);
            return ret;
        },
        __wbg___wbindgen_number_get_8ff4255516ccad3e: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            if (!isLikeNone(ret)) {
                _assertNum(ret);
            }
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_72fb696202c56729: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_be289d5034ed271b: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_d9b87ff7982e3b21: function() { return logError(function (arg0) {
            arg0._wbg_cb_unref();
        }, arguments); },
        __wbg_actualBoundingBoxAscent_c53eadfc1424b1ea: function() { return logError(function (arg0) {
            const ret = arg0.actualBoundingBoxAscent;
            return ret;
        }, arguments); },
        __wbg_actualBoundingBoxDescent_f30ccd05a7e262e3: function() { return logError(function (arg0) {
            const ret = arg0.actualBoundingBoxDescent;
            return ret;
        }, arguments); },
        __wbg_addColorStop_2f80f11dfad35dec: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.addColorStop(arg1, getStringFromWasm0(arg2, arg3));
        }, arguments); },
        __wbg_addEventListener_3acb0aad4483804c: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_addEventListener_c917b5aafbcf493f: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.addEventListener(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_aion_charts_new: function() { return logError(function (arg0) {
            const ret = Aion_charts.__wrap(arg0);
            return ret;
        }, arguments); },
        __wbg_appendChild_dea38765a26d346d: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.appendChild(arg1);
            return ret;
        }, arguments); },
        __wbg_arcTo_ddf6b8adf3bf5084: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.arcTo(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_arc_60bf829e1bd2add5: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.arc(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_beginComputePass_8971ad8382254094: function() { return logError(function (arg0, arg1) {
            const ret = arg0.beginComputePass(arg1);
            return ret;
        }, arguments); },
        __wbg_beginPath_9873f939d695759c: function() { return logError(function (arg0) {
            arg0.beginPath();
        }, arguments); },
        __wbg_beginRenderPass_599b98d9a6ba5692: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.beginRenderPass(arg1);
            return ret;
        }, arguments); },
        __wbg_body_f67922363a220026: function() { return logError(function (arg0) {
            const ret = arg0.body;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_buffer_26d0910f3a5bc899: function() { return logError(function (arg0) {
            const ret = arg0.buffer;
            return ret;
        }, arguments); },
        __wbg_button_d86841d0a03adc44: function() { return logError(function (arg0) {
            const ret = arg0.button;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_buttons_a158a0cad3175f24: function() { return logError(function (arg0) {
            const ret = arg0.buttons;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_call_389efe28435a9388: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_call_4708e0c13bdc8e95: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_call_812d25f1510c13c8: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.call(arg1, arg2, arg3);
            return ret;
        }, arguments); },
        __wbg_cancelAnimationFrame_cd35895d78cf4510: function() { return handleError(function (arg0, arg1) {
            arg0.cancelAnimationFrame(arg1);
        }, arguments); },
        __wbg_clearBuffer_90dd5c24d3374f2d: function() { return logError(function (arg0, arg1, arg2, arg3) {
            arg0.clearBuffer(arg1, arg2, arg3);
        }, arguments); },
        __wbg_clearBuffer_ecde8985ebb316ea: function() { return logError(function (arg0, arg1, arg2) {
            arg0.clearBuffer(arg1, arg2);
        }, arguments); },
        __wbg_clearRect_1eed255045515c55: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.clearRect(arg1, arg2, arg3, arg4);
        }, arguments); },
        __wbg_clearTimeout_df03cf00269bc442: function() { return logError(function (arg0, arg1) {
            arg0.clearTimeout(arg1);
        }, arguments); },
        __wbg_clientHeight_6432ff0d61ccfe7d: function() { return logError(function (arg0) {
            const ret = arg0.clientHeight;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_clientWidth_dcf89c40d88df4a3: function() { return logError(function (arg0) {
            const ret = arg0.clientWidth;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_clientX_a3c5f4ff30e91264: function() { return logError(function (arg0) {
            const ret = arg0.clientX;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_clientX_ed7d2827ca30c165: function() { return logError(function (arg0) {
            const ret = arg0.clientX;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_clientY_79ab4711d0597b2c: function() { return logError(function (arg0) {
            const ret = arg0.clientY;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_clientY_e28509acb9b4a42a: function() { return logError(function (arg0) {
            const ret = arg0.clientY;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_closePath_de4e48859360b1b1: function() { return logError(function (arg0) {
            arg0.closePath();
        }, arguments); },
        __wbg_configure_bee5e0250d8526d5: function() { return handleError(function (arg0, arg1) {
            arg0.configure(arg1);
        }, arguments); },
        __wbg_copyBufferToBuffer_3e2b8d1e524281f5: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.copyBufferToBuffer(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_copyBufferToBuffer_6a6449c0e5793f4c: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.copyBufferToBuffer(arg1, arg2, arg3, arg4);
        }, arguments); },
        __wbg_copyBufferToTexture_1a28136f43dc6ddd: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.copyBufferToTexture(arg1, arg2, arg3);
        }, arguments); },
        __wbg_copyExternalImageToTexture_27cc97955849d4dc: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.copyExternalImageToTexture(arg1, arg2, arg3);
        }, arguments); },
        __wbg_copyTextureToBuffer_d24dda6fabc7ee56: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.copyTextureToBuffer(arg1, arg2, arg3);
        }, arguments); },
        __wbg_copyTextureToTexture_bf93074b99536fcf: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.copyTextureToTexture(arg1, arg2, arg3);
        }, arguments); },
        __wbg_createBindGroupLayout_f543b79f894eed2e: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBindGroupLayout(arg1);
            return ret;
        }, arguments); },
        __wbg_createBindGroup_06db01d96df151a7: function() { return logError(function (arg0, arg1) {
            const ret = arg0.createBindGroup(arg1);
            return ret;
        }, arguments); },
        __wbg_createBuffer_6e69283608e8f98f: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBuffer(arg1);
            return ret;
        }, arguments); },
        __wbg_createCommandEncoder_88e8ef64b19cdb2c: function() { return logError(function (arg0, arg1) {
            const ret = arg0.createCommandEncoder(arg1);
            return ret;
        }, arguments); },
        __wbg_createComputePipeline_d24ca7b211444394: function() { return logError(function (arg0, arg1) {
            const ret = arg0.createComputePipeline(arg1);
            return ret;
        }, arguments); },
        __wbg_createElement_49f60fdcaae809c8: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.createElement(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_createLinearGradient_b3d3d1a53abe5362: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            const ret = arg0.createLinearGradient(arg1, arg2, arg3, arg4);
            return ret;
        }, arguments); },
        __wbg_createPipelineLayout_0f960a922b66be56: function() { return logError(function (arg0, arg1) {
            const ret = arg0.createPipelineLayout(arg1);
            return ret;
        }, arguments); },
        __wbg_createQuerySet_23e578b66db97e49: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createQuerySet(arg1);
            return ret;
        }, arguments); },
        __wbg_createRenderBundleEncoder_a372ee6ba86577dc: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createRenderBundleEncoder(arg1);
            return ret;
        }, arguments); },
        __wbg_createRenderPipeline_725209221f17f288: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createRenderPipeline(arg1);
            return ret;
        }, arguments); },
        __wbg_createSampler_36aca895fb724d8b: function() { return logError(function (arg0, arg1) {
            const ret = arg0.createSampler(arg1);
            return ret;
        }, arguments); },
        __wbg_createShaderModule_714b17aece65828e: function() { return logError(function (arg0, arg1) {
            const ret = arg0.createShaderModule(arg1);
            return ret;
        }, arguments); },
        __wbg_createTask_deeb88a68fc97c9d: function() { return handleError(function (arg0, arg1) {
            const ret = console.createTask(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_createTexture_63195fd0d63c3a24: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createTexture(arg1);
            return ret;
        }, arguments); },
        __wbg_createView_79f49fbd3fb5f94f: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createView(arg1);
            return ret;
        }, arguments); },
        __wbg_ctrlKey_96ff94f8b18636a3: function() { return logError(function (arg0) {
            const ret = arg0.ctrlKey;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_debug_a4099fa12db6cd61: function() { return logError(function (arg0) {
            console.debug(arg0);
        }, arguments); },
        __wbg_deltaMode_a1d1df711e44cefc: function() { return logError(function (arg0) {
            const ret = arg0.deltaMode;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_deltaX_f0ca9116db5f7bc1: function() { return logError(function (arg0) {
            const ret = arg0.deltaX;
            return ret;
        }, arguments); },
        __wbg_deltaY_eb94120160ac821c: function() { return logError(function (arg0) {
            const ret = arg0.deltaY;
            return ret;
        }, arguments); },
        __wbg_destroy_7602e890b930bb90: function() { return logError(function (arg0) {
            arg0.destroy();
        }, arguments); },
        __wbg_destroy_9155d0887cf67205: function() { return logError(function (arg0) {
            arg0.destroy();
        }, arguments); },
        __wbg_destroy_aae96ad45238cff2: function() { return logError(function (arg0) {
            arg0.destroy();
        }, arguments); },
        __wbg_devicePixelRatio_5c458affc89fc209: function() { return logError(function (arg0) {
            const ret = arg0.devicePixelRatio;
            return ret;
        }, arguments); },
        __wbg_disconnect_5202f399852258c0: function() { return logError(function (arg0) {
            arg0.disconnect();
        }, arguments); },
        __wbg_document_ee35a3d3ae34ef6c: function() { return logError(function (arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_done_57b39ecd9addfe81: function() { return logError(function (arg0) {
            const ret = arg0.done;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_drawImage_8fd92dab0320a117: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.drawImage(arg1, arg2, arg3);
        }, arguments); },
        __wbg_drawIndexedIndirect_5502f87d16fa681a: function() { return logError(function (arg0, arg1, arg2) {
            arg0.drawIndexedIndirect(arg1, arg2);
        }, arguments); },
        __wbg_drawIndexed_c47b56e3bafadecb: function() { return logError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawIndexed(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5 >>> 0);
        }, arguments); },
        __wbg_drawIndirect_fb473a1c2f258da2: function() { return logError(function (arg0, arg1, arg2) {
            arg0.drawIndirect(arg1, arg2);
        }, arguments); },
        __wbg_draw_3f782f0d09a907da: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.draw(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        }, arguments); },
        __wbg_end_8bb194afb9988691: function() { return logError(function (arg0) {
            arg0.end();
        }, arguments); },
        __wbg_error_7534b8e9a36f1ab4: function() { return logError(function (arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        }, arguments); },
        __wbg_error_9a7fe3f932034cde: function() { return logError(function (arg0) {
            console.error(arg0);
        }, arguments); },
        __wbg_error_c1c426d1ef02ccf6: function() { return logError(function (arg0) {
            const ret = arg0.error;
            return ret;
        }, arguments); },
        __wbg_error_f852e41c69b0bd84: function() { return logError(function (arg0, arg1) {
            console.error(arg0, arg1);
        }, arguments); },
        __wbg_executeBundles_de62b9f5a1376f4b: function() { return logError(function (arg0, arg1) {
            arg0.executeBundles(arg1);
        }, arguments); },
        __wbg_features_2dff276169fd5138: function() { return logError(function (arg0) {
            const ret = arg0.features;
            return ret;
        }, arguments); },
        __wbg_features_5d2c2677affa352d: function() { return logError(function (arg0) {
            const ret = arg0.features;
            return ret;
        }, arguments); },
        __wbg_fillRect_d44afec47e3a3fab: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.fillRect(arg1, arg2, arg3, arg4);
        }, arguments); },
        __wbg_fillText_4a931850b976cc62: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.fillText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_fill_1eb35c386c8676aa: function() { return logError(function (arg0) {
            arg0.fill();
        }, arguments); },
        __wbg_finish_08e2d7b08c066b25: function() { return logError(function (arg0, arg1) {
            const ret = arg0.finish(arg1);
            return ret;
        }, arguments); },
        __wbg_finish_5ebfba3167b3092c: function() { return logError(function (arg0) {
            const ret = arg0.finish();
            return ret;
        }, arguments); },
        __wbg_firstChild_2950111f6da7246c: function() { return logError(function (arg0) {
            const ret = arg0.firstChild;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_from_bddd64e7d5ff6941: function() { return logError(function (arg0) {
            const ret = Array.from(arg0);
            return ret;
        }, arguments); },
        __wbg_getAttribute_b9f6fc4b689c71b0: function() { return logError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.getAttribute(getStringFromWasm0(arg2, arg3));
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_getBoundingClientRect_b5c8c34d07878818: function() { return logError(function (arg0) {
            const ret = arg0.getBoundingClientRect();
            return ret;
        }, arguments); },
        __wbg_getContext_2966500392030d63: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_2a5764d48600bc43: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getCurrentTexture_6dc4d0ea8555e374: function() { return handleError(function (arg0) {
            const ret = arg0.getCurrentTexture();
            return ret;
        }, arguments); },
        __wbg_getElementById_e34377b79d7285f6: function() { return logError(function (arg0, arg1, arg2) {
            const ret = arg0.getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getMappedRange_3cb6354f7963e27e: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getMappedRange(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_getPreferredCanvasFormat_06854455b835cf40: function() { return logError(function (arg0) {
            const ret = arg0.getPreferredCanvasFormat();
            return (__wbindgen_enum_GpuTextureFormat.indexOf(ret) + 1 || 96) - 1;
        }, arguments); },
        __wbg_getPropertyValue_d6911b2a1f9acba9: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.getPropertyValue(getStringFromWasm0(arg2, arg3));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_getUTCDate_aad14cab5ce3b408: function() { return logError(function (arg0) {
            const ret = arg0.getUTCDate();
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_getUTCFullYear_e2ef808de49a659f: function() { return logError(function (arg0) {
            const ret = arg0.getUTCFullYear();
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_getUTCHours_35ca437eb5eea37f: function() { return logError(function (arg0) {
            const ret = arg0.getUTCHours();
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_getUTCMinutes_f7f7e50da0efa786: function() { return logError(function (arg0) {
            const ret = arg0.getUTCMinutes();
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_getUTCMonth_1225344f80ac9874: function() { return logError(function (arg0) {
            const ret = arg0.getUTCMonth();
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_getUTCSeconds_0974d30103b4f4d9: function() { return logError(function (arg0) {
            const ret = arg0.getUTCSeconds();
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_get_4fe487fe39ff3573: function() { return logError(function (arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_get_9b94d73e6221f75c: function() { return logError(function (arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        }, arguments); },
        __wbg_get_b3ed3ad4be2bc8ac: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_d8db2ad31d529ff8: function() { return logError(function (arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_gpu_653e59c6ae8028a8: function() { return logError(function (arg0) {
            const ret = arg0.gpu;
            return ret;
        }, arguments); },
        __wbg_has_f1efef5b257eade8: function() { return logError(function (arg0, arg1, arg2) {
            const ret = arg0.has(getStringFromWasm0(arg1, arg2));
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_height_38750dc6de41ee75: function() { return logError(function (arg0) {
            const ret = arg0.height;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_height_45209601b4c4ede6: function() { return logError(function (arg0) {
            const ret = arg0.height;
            return ret;
        }, arguments); },
        __wbg_id_ff64a5892a30d4e9: function() { return logError(function (arg0, arg1) {
            const ret = arg1.id;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_info_148d043840582012: function() { return logError(function (arg0) {
            console.info(arg0);
        }, arguments); },
        __wbg_instanceof_CanvasRenderingContext2d_4bb052fd1c3d134d: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof CanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_Element_9e662f49ab6c6beb: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof Element;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_GpuAdapter_b2c1300e425af95c: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof GPUAdapter;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_GpuCanvasContext_c9b75b4b7dc7555e: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof GPUCanvasContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_GpuDeviceLostInfo_d07227c7621bedb8: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof GPUDeviceLostInfo;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_GpuOutOfMemoryError_36be198c7584d724: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof GPUOutOfMemoryError;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_GpuValidationError_9f5a409dc19b2a44: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof GPUValidationError;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_HtmlCanvasElement_3f2f6e1edb1c9792: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_HtmlDivElement_df0f494aea0b26b4: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLDivElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_HtmlElement_5abfac207260fd6f: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_Object_1c6af87502b733ed: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof Object;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_instanceof_Window_ed49b2db8df90359: function() { return logError(function (arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_isArray_d314bb98fcf08331: function() { return logError(function (arg0) {
            const ret = Array.isArray(arg0);
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_is_f29129f676e5410c: function() { return logError(function (arg0, arg1) {
            const ret = Object.is(arg0, arg1);
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_keys_f0752df54b3ecc47: function() { return logError(function (arg0) {
            const ret = arg0.keys();
            return ret;
        }, arguments); },
        __wbg_label_f279af9fe090b53f: function() { return logError(function (arg0, arg1) {
            const ret = arg1.label;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_left_3b7c3c1030d5ca7a: function() { return logError(function (arg0) {
            const ret = arg0.left;
            return ret;
        }, arguments); },
        __wbg_length_25b2ccd77d48ecb1: function() { return logError(function (arg0) {
            const ret = arg0.length;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_length_35a7bace40f36eac: function() { return logError(function (arg0) {
            const ret = arg0.length;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_limits_486026e4aa69b9b2: function() { return logError(function (arg0) {
            const ret = arg0.limits;
            return ret;
        }, arguments); },
        __wbg_limits_59402e6db2c6b230: function() { return logError(function (arg0) {
            const ret = arg0.limits;
            return ret;
        }, arguments); },
        __wbg_lineTo_c584cff6c760c4a5: function() { return logError(function (arg0, arg1, arg2) {
            arg0.lineTo(arg1, arg2);
        }, arguments); },
        __wbg_log_6b5ca2e6124b2808: function() { return logError(function (arg0) {
            console.log(arg0);
        }, arguments); },
        __wbg_lost_494e98e7ee6d8da8: function() { return logError(function (arg0) {
            const ret = arg0.lost;
            return ret;
        }, arguments); },
        __wbg_mapAsync_e89ffbd0722e6025: function() { return logError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.mapAsync(arg1 >>> 0, arg2, arg3);
            return ret;
        }, arguments); },
        __wbg_maxBindGroups_52e3144d1d4f3951: function() { return logError(function (arg0) {
            const ret = arg0.maxBindGroups;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxBindingsPerBindGroup_8e383157db4cfd9d: function() { return logError(function (arg0) {
            const ret = arg0.maxBindingsPerBindGroup;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxBufferSize_4bed0deb2b5570bc: function() { return logError(function (arg0) {
            const ret = arg0.maxBufferSize;
            return ret;
        }, arguments); },
        __wbg_maxColorAttachmentBytesPerSample_2ded1d176129b49e: function() { return logError(function (arg0) {
            const ret = arg0.maxColorAttachmentBytesPerSample;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxColorAttachments_a363e1f84136b445: function() { return logError(function (arg0) {
            const ret = arg0.maxColorAttachments;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxComputeInvocationsPerWorkgroup_8c8259a34a467300: function() { return logError(function (arg0) {
            const ret = arg0.maxComputeInvocationsPerWorkgroup;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxComputeWorkgroupSizeX_6a123a5258a37c70: function() { return logError(function (arg0) {
            const ret = arg0.maxComputeWorkgroupSizeX;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxComputeWorkgroupSizeY_212a6e863b315f06: function() { return logError(function (arg0) {
            const ret = arg0.maxComputeWorkgroupSizeY;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxComputeWorkgroupSizeZ_53a8c06a42e0daa4: function() { return logError(function (arg0) {
            const ret = arg0.maxComputeWorkgroupSizeZ;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxComputeWorkgroupStorageSize_0940bd6b70d5ee03: function() { return logError(function (arg0) {
            const ret = arg0.maxComputeWorkgroupStorageSize;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxComputeWorkgroupsPerDimension_155968404880d2bc: function() { return logError(function (arg0) {
            const ret = arg0.maxComputeWorkgroupsPerDimension;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxDynamicStorageBuffersPerPipelineLayout_7d88fb9026cd8af3: function() { return logError(function (arg0) {
            const ret = arg0.maxDynamicStorageBuffersPerPipelineLayout;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxDynamicUniformBuffersPerPipelineLayout_146ac1a721fbca9b: function() { return logError(function (arg0) {
            const ret = arg0.maxDynamicUniformBuffersPerPipelineLayout;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxSampledTexturesPerShaderStage_10ee96b97a701e05: function() { return logError(function (arg0) {
            const ret = arg0.maxSampledTexturesPerShaderStage;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxSamplersPerShaderStage_7546a712e69839d0: function() { return logError(function (arg0) {
            const ret = arg0.maxSamplersPerShaderStage;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxStorageBufferBindingSize_6f36ebfc9d4874d1: function() { return logError(function (arg0) {
            const ret = arg0.maxStorageBufferBindingSize;
            return ret;
        }, arguments); },
        __wbg_maxStorageBuffersPerShaderStage_ad3988a66894ccd8: function() { return logError(function (arg0) {
            const ret = arg0.maxStorageBuffersPerShaderStage;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxStorageTexturesPerShaderStage_3c4b0fd6cdb25d2f: function() { return logError(function (arg0) {
            const ret = arg0.maxStorageTexturesPerShaderStage;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxTextureArrayLayers_596c959454186b7e: function() { return logError(function (arg0) {
            const ret = arg0.maxTextureArrayLayers;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxTextureDimension1D_395c7225194787e6: function() { return logError(function (arg0) {
            const ret = arg0.maxTextureDimension1D;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxTextureDimension2D_1c70c07372595733: function() { return logError(function (arg0) {
            const ret = arg0.maxTextureDimension2D;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxTextureDimension3D_c2c0b973db2f7087: function() { return logError(function (arg0) {
            const ret = arg0.maxTextureDimension3D;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxUniformBufferBindingSize_18e95cb371149021: function() { return logError(function (arg0) {
            const ret = arg0.maxUniformBufferBindingSize;
            return ret;
        }, arguments); },
        __wbg_maxUniformBuffersPerShaderStage_e21721df6407d356: function() { return logError(function (arg0) {
            const ret = arg0.maxUniformBuffersPerShaderStage;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxVertexAttributes_3685d049fb4b9557: function() { return logError(function (arg0) {
            const ret = arg0.maxVertexAttributes;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxVertexBufferArrayStride_799ce7d416969442: function() { return logError(function (arg0) {
            const ret = arg0.maxVertexBufferArrayStride;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_maxVertexBuffers_9e36c1cf99fac3d6: function() { return logError(function (arg0) {
            const ret = arg0.maxVertexBuffers;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_measureText_9d64a92333bd05ee: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.measureText(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_message_622af13c44fafefe: function() { return logError(function (arg0, arg1) {
            const ret = arg1.message;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_message_b173dc74ecacb5d2: function() { return logError(function (arg0, arg1) {
            const ret = arg1.message;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_metaKey_374999c340f70626: function() { return logError(function (arg0) {
            const ret = arg0.metaKey;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_minStorageBufferOffsetAlignment_04598b6c2361de5d: function() { return logError(function (arg0) {
            const ret = arg0.minStorageBufferOffsetAlignment;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_minUniformBufferOffsetAlignment_0743900952f2cbce: function() { return logError(function (arg0) {
            const ret = arg0.minUniformBufferOffsetAlignment;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_moveTo_e9190fc700d55b40: function() { return logError(function (arg0, arg1, arg2) {
            arg0.moveTo(arg1, arg2);
        }, arguments); },
        __wbg_navigator_43be698ba96fc088: function() { return logError(function (arg0) {
            const ret = arg0.navigator;
            return ret;
        }, arguments); },
        __wbg_navigator_4478931f32ebca57: function() { return logError(function (arg0) {
            const ret = arg0.navigator;
            return ret;
        }, arguments); },
        __wbg_new_245cd5c49157e602: function() { return logError(function (arg0) {
            const ret = new Date(arg0);
            return ret;
        }, arguments); },
        __wbg_new_2e2be9617c4407d5: function() { return handleError(function (arg0) {
            const ret = new ResizeObserver(arg0);
            return ret;
        }, arguments); },
        __wbg_new_361308b2356cecd0: function() { return logError(function () {
            const ret = new Object();
            return ret;
        }, arguments); },
        __wbg_new_3eb36ae241fe6f44: function() { return logError(function () {
            const ret = new Array();
            return ret;
        }, arguments); },
        __wbg_new_8a6f238a6ece86ea: function() { return logError(function () {
            const ret = new Error();
            return ret;
        }, arguments); },
        __wbg_new_b5d9e2fb389fef91: function() { return logError(function (arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen__convert__closures_____invoke__h2e4f7e4bbddf86e9(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = state0.b = 0;
            }
        }, arguments); },
        __wbg_new_from_slice_a3d2629dc1826784: function() { return logError(function (arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_no_args_1c7c842f08d00ebb: function() { return logError(function (arg0, arg1) {
            const ret = new Function(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_new_with_byte_offset_and_length_aa261d9c9da49eb1: function() { return logError(function (arg0, arg1, arg2) {
            const ret = new Uint8Array(arg0, arg1 >>> 0, arg2 >>> 0);
            return ret;
        }, arguments); },
        __wbg_new_with_length_1763c527b2923202: function() { return logError(function (arg0) {
            const ret = new Array(arg0 >>> 0);
            return ret;
        }, arguments); },
        __wbg_new_with_length_6523745c0bd32809: function() { return logError(function (arg0) {
            const ret = new Float64Array(arg0 >>> 0);
            return ret;
        }, arguments); },
        __wbg_new_with_length_68f01b2100133ebd: function() { return logError(function (arg0) {
            const ret = new BigUint64Array(arg0 >>> 0);
            return ret;
        }, arguments); },
        __wbg_next_3482f54c49e8af19: function() { return handleError(function (arg0) {
            const ret = arg0.next();
            return ret;
        }, arguments); },
        __wbg_now_a3af9a2f4bbaa4d1: function() { return logError(function () {
            const ret = Date.now();
            return ret;
        }, arguments); },
        __wbg_observe_b9abc08d6d829e56: function() { return logError(function (arg0, arg1) {
            arg0.observe(arg1);
        }, arguments); },
        __wbg_of_9ab14f9d4bfb5040: function() { return logError(function (arg0, arg1) {
            const ret = Array.of(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_onSubmittedWorkDone_babe5ab237e856ff: function() { return logError(function (arg0) {
            const ret = arg0.onSubmittedWorkDone();
            return ret;
        }, arguments); },
        __wbg_pageY_5653bbc6f8a6f28d: function() { return logError(function (arg0) {
            const ret = arg0.pageY;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_parentElement_75863410a8617953: function() { return logError(function (arg0) {
            const ret = arg0.parentElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_parse_708461a1feddfb38: function() { return handleError(function (arg0, arg1) {
            const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_pointerId_466b1bdcaf2fe835: function() { return logError(function (arg0) {
            const ret = arg0.pointerId;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_pointerType_ba53c6f18634a26d: function() { return logError(function (arg0, arg1) {
            const ret = arg1.pointerType;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_popErrorScope_e824ee97fc4191f3: function() { return logError(function (arg0) {
            const ret = arg0.popErrorScope();
            return ret;
        }, arguments); },
        __wbg_preventDefault_cdcfcd7e301b9702: function() { return logError(function (arg0) {
            arg0.preventDefault();
        }, arguments); },
        __wbg_pushErrorScope_6ed69f7e8013c9c8: function() { return logError(function (arg0, arg1) {
            arg0.pushErrorScope(__wbindgen_enum_GpuErrorFilter[arg1]);
        }, arguments); },
        __wbg_push_8ffdcb2063340ba5: function() { return logError(function (arg0, arg1) {
            const ret = arg0.push(arg1);
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_quadraticCurveTo_b39b7adc73767cc0: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.quadraticCurveTo(arg1, arg2, arg3, arg4);
        }, arguments); },
        __wbg_querySelectorAll_1283aae52043a951: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelectorAll(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_queueMicrotask_0aa0a927f78f5d98: function() { return logError(function (arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        }, arguments); },
        __wbg_queueMicrotask_5bb536982f78a56f: function() { return logError(function (arg0) {
            queueMicrotask(arg0);
        }, arguments); },
        __wbg_queue_13a5c48e3c54a28c: function() { return logError(function (arg0) {
            const ret = arg0.queue;
            return ret;
        }, arguments); },
        __wbg_reason_70f510afd8774d84: function() { return logError(function (arg0) {
            const ret = arg0.reason;
            return (__wbindgen_enum_GpuDeviceLostReason.indexOf(ret) + 1 || 3) - 1;
        }, arguments); },
        __wbg_releasePointerCapture_420ef33c7c5fb6f4: function() { return handleError(function (arg0, arg1) {
            arg0.releasePointerCapture(arg1);
        }, arguments); },
        __wbg_removeChild_2f0b06213dbc49ca: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.removeChild(arg1);
            return ret;
        }, arguments); },
        __wbg_removeEventListener_e63328781a5b9af9: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.removeEventListener(getStringFromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_remove_31c39325eee968fc: function() { return logError(function (arg0) {
            arg0.remove();
        }, arguments); },
        __wbg_requestAdapter_cc9a9924f72519ab: function() { return logError(function (arg0, arg1) {
            const ret = arg0.requestAdapter(arg1);
            return ret;
        }, arguments); },
        __wbg_requestAnimationFrame_43682f8e1c5e5348: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.requestAnimationFrame(arg1);
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_requestDevice_295504649d1da14c: function() { return logError(function (arg0, arg1) {
            const ret = arg0.requestDevice(arg1);
            return ret;
        }, arguments); },
        __wbg_resolveQuerySet_58d78db4578ebdb5: function() { return logError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.resolveQuerySet(arg1, arg2 >>> 0, arg3 >>> 0, arg4, arg5 >>> 0);
        }, arguments); },
        __wbg_resolve_002c4b7d9d8f6b64: function() { return logError(function (arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        }, arguments); },
        __wbg_restore_0d233789d098ba64: function() { return logError(function (arg0) {
            arg0.restore();
        }, arguments); },
        __wbg_rotate_31f482965274db16: function() { return handleError(function (arg0, arg1) {
            arg0.rotate(arg1);
        }, arguments); },
        __wbg_run_bcde7ea43ea6ed7c: function() { return logError(function (arg0, arg1, arg2) {
            try {
                var state0 = {a: arg1, b: arg2};
                var cb0 = () => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen__convert__closures_____invoke__hf263c0e6d31821ed(a, state0.b, );
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = arg0.run(cb0);
                _assertBoolean(ret);
                return ret;
            } finally {
                state0.a = state0.b = 0;
            }
        }, arguments); },
        __wbg_save_e0cc2e58b36d33c9: function() { return logError(function (arg0) {
            arg0.save();
        }, arguments); },
        __wbg_setAttribute_cc8e4c8a2a008508: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setBindGroup_bf7233e51ee0fd56: function() { return logError(function (arg0, arg1, arg2) {
            arg0.setBindGroup(arg1 >>> 0, arg2);
        }, arguments); },
        __wbg_setBindGroup_c532d9e80c3b863a: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setBindGroup(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4), arg5, arg6 >>> 0);
        }, arguments); },
        __wbg_setBlendConstant_1c24115d90a69114: function() { return handleError(function (arg0, arg1) {
            arg0.setBlendConstant(arg1);
        }, arguments); },
        __wbg_setIndexBuffer_d5812b7c5ff15c50: function() { return logError(function (arg0, arg1, arg2, arg3) {
            arg0.setIndexBuffer(arg1, __wbindgen_enum_GpuIndexFormat[arg2], arg3);
        }, arguments); },
        __wbg_setIndexBuffer_d6851b016152211a: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setIndexBuffer(arg1, __wbindgen_enum_GpuIndexFormat[arg2], arg3, arg4);
        }, arguments); },
        __wbg_setLineDash_ecf27050368658c9: function() { return handleError(function (arg0, arg1) {
            arg0.setLineDash(arg1);
        }, arguments); },
        __wbg_setPipeline_b632e313f54b1cb1: function() { return logError(function (arg0, arg1) {
            arg0.setPipeline(arg1);
        }, arguments); },
        __wbg_setPointerCapture_420db6f6826eb74b: function() { return handleError(function (arg0, arg1) {
            arg0.setPointerCapture(arg1);
        }, arguments); },
        __wbg_setProperty_cbb25c4e74285b39: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setProperty(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setScissorRect_13be2665184d6e20: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setScissorRect(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        }, arguments); },
        __wbg_setStencilReference_0e9e96a76b035161: function() { return logError(function (arg0, arg1) {
            arg0.setStencilReference(arg1 >>> 0);
        }, arguments); },
        __wbg_setTimeout_eff32631ea138533: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.setTimeout(arg1, arg2);
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_setTransform_96b561b274a594ca: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setTransform(arg1, arg2, arg3, arg4, arg5, arg6);
        }, arguments); },
        __wbg_setVertexBuffer_71f6b6b9f7c32e99: function() { return logError(function (arg0, arg1, arg2, arg3) {
            arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3);
        }, arguments); },
        __wbg_setVertexBuffer_c8234139ead62a61: function() { return logError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3, arg4);
        }, arguments); },
        __wbg_setViewport_b25340c5cfc5e64f: function() { return logError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setViewport(arg1, arg2, arg3, arg4, arg5, arg6);
        }, arguments); },
        __wbg_set_6cb8631f80447a67: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_set_a_e87a2053d5fccb4c: function() { return logError(function (arg0, arg1) {
            arg0.a = arg1;
        }, arguments); },
        __wbg_set_access_69d91e9d4e4ceac2: function() { return logError(function (arg0, arg1) {
            arg0.access = __wbindgen_enum_GpuStorageTextureAccess[arg1];
        }, arguments); },
        __wbg_set_address_mode_u_17e91ba6701d7cdf: function() { return logError(function (arg0, arg1) {
            arg0.addressModeU = __wbindgen_enum_GpuAddressMode[arg1];
        }, arguments); },
        __wbg_set_address_mode_v_83cff33885b49fd0: function() { return logError(function (arg0, arg1) {
            arg0.addressModeV = __wbindgen_enum_GpuAddressMode[arg1];
        }, arguments); },
        __wbg_set_address_mode_w_2445963d0feae757: function() { return logError(function (arg0, arg1) {
            arg0.addressModeW = __wbindgen_enum_GpuAddressMode[arg1];
        }, arguments); },
        __wbg_set_alpha_a7a68e5ec04efe77: function() { return logError(function (arg0, arg1) {
            arg0.alpha = arg1;
        }, arguments); },
        __wbg_set_alpha_mode_60f87267fa3d95d0: function() { return logError(function (arg0, arg1) {
            arg0.alphaMode = __wbindgen_enum_GpuCanvasAlphaMode[arg1];
        }, arguments); },
        __wbg_set_alpha_to_coverage_enabled_67782b8fff854d06: function() { return logError(function (arg0, arg1) {
            arg0.alphaToCoverageEnabled = arg1 !== 0;
        }, arguments); },
        __wbg_set_array_layer_count_2bd74e56899b603a: function() { return logError(function (arg0, arg1) {
            arg0.arrayLayerCount = arg1 >>> 0;
        }, arguments); },
        __wbg_set_array_stride_acb85bd3848529a6: function() { return logError(function (arg0, arg1) {
            arg0.arrayStride = arg1;
        }, arguments); },
        __wbg_set_aspect_01abc5aa9afad261: function() { return logError(function (arg0, arg1) {
            arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
        }, arguments); },
        __wbg_set_aspect_82ca9caa27a4c533: function() { return logError(function (arg0, arg1) {
            arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
        }, arguments); },
        __wbg_set_aspect_b78bd0b34ebfe19b: function() { return logError(function (arg0, arg1) {
            arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
        }, arguments); },
        __wbg_set_attributes_4d5de6c80e3a7e73: function() { return logError(function (arg0, arg1) {
            arg0.attributes = arg1;
        }, arguments); },
        __wbg_set_b_87725d82ac69a631: function() { return logError(function (arg0, arg1) {
            arg0.b = arg1;
        }, arguments); },
        __wbg_set_base_array_layer_064977086530f2e7: function() { return logError(function (arg0, arg1) {
            arg0.baseArrayLayer = arg1 >>> 0;
        }, arguments); },
        __wbg_set_base_mip_level_845abe28a57bd901: function() { return logError(function (arg0, arg1) {
            arg0.baseMipLevel = arg1 >>> 0;
        }, arguments); },
        __wbg_set_beginning_of_pass_write_index_18bb7ab9fb16de02: function() { return logError(function (arg0, arg1) {
            arg0.beginningOfPassWriteIndex = arg1 >>> 0;
        }, arguments); },
        __wbg_set_beginning_of_pass_write_index_1d1dcdf984952e54: function() { return logError(function (arg0, arg1) {
            arg0.beginningOfPassWriteIndex = arg1 >>> 0;
        }, arguments); },
        __wbg_set_bind_group_layouts_db65f9787380e242: function() { return logError(function (arg0, arg1) {
            arg0.bindGroupLayouts = arg1;
        }, arguments); },
        __wbg_set_binding_35fa28beda49ff83: function() { return logError(function (arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        }, arguments); },
        __wbg_set_binding_3b4abee15b11f6ec: function() { return logError(function (arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        }, arguments); },
        __wbg_set_blend_21337ec514ad2280: function() { return logError(function (arg0, arg1) {
            arg0.blend = arg1;
        }, arguments); },
        __wbg_set_buffer_a9223dfcc0e34853: function() { return logError(function (arg0, arg1) {
            arg0.buffer = arg1;
        }, arguments); },
        __wbg_set_buffer_d49e95bb5349d827: function() { return logError(function (arg0, arg1) {
            arg0.buffer = arg1;
        }, arguments); },
        __wbg_set_buffer_f8967886328760f6: function() { return logError(function (arg0, arg1) {
            arg0.buffer = arg1;
        }, arguments); },
        __wbg_set_buffers_68609a5d48c31b27: function() { return logError(function (arg0, arg1) {
            arg0.buffers = arg1;
        }, arguments); },
        __wbg_set_bytes_per_row_1ee6dfa31a861d51: function() { return logError(function (arg0, arg1) {
            arg0.bytesPerRow = arg1 >>> 0;
        }, arguments); },
        __wbg_set_bytes_per_row_4a52bbf4cdbfe78b: function() { return logError(function (arg0, arg1) {
            arg0.bytesPerRow = arg1 >>> 0;
        }, arguments); },
        __wbg_set_className_c1d9e7362164af61: function() { return logError(function (arg0, arg1, arg2) {
            arg0.className = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_clear_value_8fc3623594df71b2: function() { return logError(function (arg0, arg1) {
            arg0.clearValue = arg1;
        }, arguments); },
        __wbg_set_code_20093e29960281f8: function() { return logError(function (arg0, arg1, arg2) {
            arg0.code = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_color_64a633bf7b4cf6fe: function() { return logError(function (arg0, arg1) {
            arg0.color = arg1;
        }, arguments); },
        __wbg_set_color_attachments_4d4c71d7eeba8e2f: function() { return logError(function (arg0, arg1) {
            arg0.colorAttachments = arg1;
        }, arguments); },
        __wbg_set_color_formats_124c2fc8ea5f658d: function() { return logError(function (arg0, arg1) {
            arg0.colorFormats = arg1;
        }, arguments); },
        __wbg_set_compare_0376672b0c0bbfd8: function() { return logError(function (arg0, arg1) {
            arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
        }, arguments); },
        __wbg_set_compare_f3fb77a9bf3f0f7e: function() { return logError(function (arg0, arg1) {
            arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
        }, arguments); },
        __wbg_set_compute_937f4ee700e465ff: function() { return logError(function (arg0, arg1) {
            arg0.compute = arg1;
        }, arguments); },
        __wbg_set_count_6f4f66c8eedc9bba: function() { return logError(function (arg0, arg1) {
            arg0.count = arg1 >>> 0;
        }, arguments); },
        __wbg_set_count_8cf9a3dd1ffc7b7d: function() { return logError(function (arg0, arg1) {
            arg0.count = arg1 >>> 0;
        }, arguments); },
        __wbg_set_cssText_18380c97092caefa: function() { return logError(function (arg0, arg1, arg2) {
            arg0.cssText = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_cull_mode_41c12526410d3e05: function() { return logError(function (arg0, arg1) {
            arg0.cullMode = __wbindgen_enum_GpuCullMode[arg1];
        }, arguments); },
        __wbg_set_depth_bias_31554aeaaa675954: function() { return logError(function (arg0, arg1) {
            arg0.depthBias = arg1;
        }, arguments); },
        __wbg_set_depth_bias_clamp_8cf5f4f0d80e8cba: function() { return logError(function (arg0, arg1) {
            arg0.depthBiasClamp = arg1;
        }, arguments); },
        __wbg_set_depth_bias_slope_scale_310ae406f2d3a055: function() { return logError(function (arg0, arg1) {
            arg0.depthBiasSlopeScale = arg1;
        }, arguments); },
        __wbg_set_depth_clear_value_8760aafb583d5312: function() { return logError(function (arg0, arg1) {
            arg0.depthClearValue = arg1;
        }, arguments); },
        __wbg_set_depth_compare_8831904ce3173063: function() { return logError(function (arg0, arg1) {
            arg0.depthCompare = __wbindgen_enum_GpuCompareFunction[arg1];
        }, arguments); },
        __wbg_set_depth_fail_op_62ec602580477afc: function() { return logError(function (arg0, arg1) {
            arg0.depthFailOp = __wbindgen_enum_GpuStencilOperation[arg1];
        }, arguments); },
        __wbg_set_depth_load_op_102d57f3ddf95461: function() { return logError(function (arg0, arg1) {
            arg0.depthLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        }, arguments); },
        __wbg_set_depth_or_array_layers_d7b93db07c5da69d: function() { return logError(function (arg0, arg1) {
            arg0.depthOrArrayLayers = arg1 >>> 0;
        }, arguments); },
        __wbg_set_depth_read_only_aebc24a542debafd: function() { return logError(function (arg0, arg1) {
            arg0.depthReadOnly = arg1 !== 0;
        }, arguments); },
        __wbg_set_depth_read_only_efe1c5933ff74d4e: function() { return logError(function (arg0, arg1) {
            arg0.depthReadOnly = arg1 !== 0;
        }, arguments); },
        __wbg_set_depth_stencil_5627e73aaf33912c: function() { return logError(function (arg0, arg1) {
            arg0.depthStencil = arg1;
        }, arguments); },
        __wbg_set_depth_stencil_attachment_04b936535778e362: function() { return logError(function (arg0, arg1) {
            arg0.depthStencilAttachment = arg1;
        }, arguments); },
        __wbg_set_depth_stencil_format_9206864898d88c62: function() { return logError(function (arg0, arg1) {
            arg0.depthStencilFormat = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_depth_store_op_610b0a50dbb00eb8: function() { return logError(function (arg0, arg1) {
            arg0.depthStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        }, arguments); },
        __wbg_set_depth_write_enabled_f94217df9ff2d60c: function() { return logError(function (arg0, arg1) {
            arg0.depthWriteEnabled = arg1 !== 0;
        }, arguments); },
        __wbg_set_device_dab18ead7bfc077b: function() { return logError(function (arg0, arg1) {
            arg0.device = arg1;
        }, arguments); },
        __wbg_set_dimension_2a75a794a0bfcc94: function() { return logError(function (arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        }, arguments); },
        __wbg_set_dimension_a3c50fb6d43f6cec: function() { return logError(function (arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureDimension[arg1];
        }, arguments); },
        __wbg_set_dst_factor_cf872fec841747ac: function() { return logError(function (arg0, arg1) {
            arg0.dstFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        }, arguments); },
        __wbg_set_end_of_pass_write_index_02ee5189026c1d3a: function() { return logError(function (arg0, arg1) {
            arg0.endOfPassWriteIndex = arg1 >>> 0;
        }, arguments); },
        __wbg_set_end_of_pass_write_index_12c25e0a48d5aa5c: function() { return logError(function (arg0, arg1) {
            arg0.endOfPassWriteIndex = arg1 >>> 0;
        }, arguments); },
        __wbg_set_entries_1472deaee7053fb7: function() { return logError(function (arg0, arg1) {
            arg0.entries = arg1;
        }, arguments); },
        __wbg_set_entries_b2258b5ef29810b0: function() { return logError(function (arg0, arg1) {
            arg0.entries = arg1;
        }, arguments); },
        __wbg_set_entry_point_11f912102ade99b1: function() { return logError(function (arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_entry_point_7f546bbf1e63e58d: function() { return logError(function (arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_entry_point_f9224cdb29cbe5df: function() { return logError(function (arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_external_texture_613e4434100d63ee: function() { return logError(function (arg0, arg1) {
            arg0.externalTexture = arg1;
        }, arguments); },
        __wbg_set_f43e577aea94465b: function() { return logError(function (arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
        }, arguments); },
        __wbg_set_fail_op_73a4e194f4bc914a: function() { return logError(function (arg0, arg1) {
            arg0.failOp = __wbindgen_enum_GpuStencilOperation[arg1];
        }, arguments); },
        __wbg_set_fillStyle_783d3f7489475421: function() { return logError(function (arg0, arg1, arg2) {
            arg0.fillStyle = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_fillStyle_9bd3ccbe7ecf6c2a: function() { return logError(function (arg0, arg1) {
            arg0.fillStyle = arg1;
        }, arguments); },
        __wbg_set_flip_y_330c483dc6f7916c: function() { return logError(function (arg0, arg1) {
            arg0.flipY = arg1 !== 0;
        }, arguments); },
        __wbg_set_font_575685c8f7e56957: function() { return logError(function (arg0, arg1, arg2) {
            arg0.font = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_format_1670e760e18ac001: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_format_2141a8a1fd36fb9c: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_format_25e4aacc74949e38: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_format_3f7008e9e568f0fc: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuVertexFormat[arg1];
        }, arguments); },
        __wbg_set_format_4a4fccdfc45bc409: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_format_7696f8290da8a36b: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_format_974a01725f579c5d: function() { return logError(function (arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        }, arguments); },
        __wbg_set_fragment_f7ce64feaf1cd7dc: function() { return logError(function (arg0, arg1) {
            arg0.fragment = arg1;
        }, arguments); },
        __wbg_set_front_face_09e32557f8852301: function() { return logError(function (arg0, arg1) {
            arg0.frontFace = __wbindgen_enum_GpuFrontFace[arg1];
        }, arguments); },
        __wbg_set_g_c31c959457596456: function() { return logError(function (arg0, arg1) {
            arg0.g = arg1;
        }, arguments); },
        __wbg_set_has_dynamic_offset_fbc1bb343939ed0b: function() { return logError(function (arg0, arg1) {
            arg0.hasDynamicOffset = arg1 !== 0;
        }, arguments); },
        __wbg_set_height_710b87344b3d6748: function() { return logError(function (arg0, arg1) {
            arg0.height = arg1 >>> 0;
        }, arguments); },
        __wbg_set_height_b386c0f603610637: function() { return logError(function (arg0, arg1) {
            arg0.height = arg1 >>> 0;
        }, arguments); },
        __wbg_set_height_f21f985387070100: function() { return logError(function (arg0, arg1) {
            arg0.height = arg1 >>> 0;
        }, arguments); },
        __wbg_set_id_9b8330f661385753: function() { return logError(function (arg0, arg1, arg2) {
            arg0.id = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_imageSmoothingEnabled_85c30565ebbfba4f: function() { return logError(function (arg0, arg1) {
            arg0.imageSmoothingEnabled = arg1 !== 0;
        }, arguments); },
        __wbg_set_index_77f6ba43cebcf275: function() { return logError(function (arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = BigInt.asUintN(64, arg2);
        }, arguments); },
        __wbg_set_index_78a85f2e336ce120: function() { return logError(function (arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
        }, arguments); },
        __wbg_set_innerHTML_edd39677e3460291: function() { return logError(function (arg0, arg1, arg2) {
            arg0.innerHTML = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_026fd015857827ae: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_0ec13ba975f77124: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_3b658d9ce970552c: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_48883f5f49e4ec47: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_4bbbc289ddddebd7: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_4d609666f09cfdfb: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_4f4264b0041180e2: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_5b46e419b9e88c5e: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_95423cd2e1f4b5dd: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_9acf6c263479f46f: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_ad0f2c69b41c3483: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_c3fc0a66f4ecc82b: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_c857f45a8485236a: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_d0fd4d4810525bf2: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_dc8df9969898889c: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_e3709fe3e82429b5: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_label_fb5d28b3ba7af11f: function() { return logError(function (arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_layout_170ec6b8aa37178f: function() { return logError(function (arg0, arg1) {
            arg0.layout = arg1;
        }, arguments); },
        __wbg_set_layout_7f76289be3294b4a: function() { return logError(function (arg0, arg1) {
            arg0.layout = arg1;
        }, arguments); },
        __wbg_set_layout_c20d48b352b24c1b: function() { return logError(function (arg0, arg1) {
            arg0.layout = arg1;
        }, arguments); },
        __wbg_set_lineCap_59a017de1ad2b0be: function() { return logError(function (arg0, arg1, arg2) {
            arg0.lineCap = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_lineJoin_9b9f1aaa283be35a: function() { return logError(function (arg0, arg1, arg2) {
            arg0.lineJoin = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_lineWidth_89fa506592f5b994: function() { return logError(function (arg0, arg1) {
            arg0.lineWidth = arg1;
        }, arguments); },
        __wbg_set_load_op_c71d200e998908b0: function() { return logError(function (arg0, arg1) {
            arg0.loadOp = __wbindgen_enum_GpuLoadOp[arg1];
        }, arguments); },
        __wbg_set_lod_max_clamp_aaac5daaecca96d4: function() { return logError(function (arg0, arg1) {
            arg0.lodMaxClamp = arg1;
        }, arguments); },
        __wbg_set_lod_min_clamp_ed2162d4b198abba: function() { return logError(function (arg0, arg1) {
            arg0.lodMinClamp = arg1;
        }, arguments); },
        __wbg_set_mag_filter_c8a8c1218cd38da6: function() { return logError(function (arg0, arg1) {
            arg0.magFilter = __wbindgen_enum_GpuFilterMode[arg1];
        }, arguments); },
        __wbg_set_mapped_at_creation_2d003ce549611385: function() { return logError(function (arg0, arg1) {
            arg0.mappedAtCreation = arg1 !== 0;
        }, arguments); },
        __wbg_set_mask_a933ba2e61c7610a: function() { return logError(function (arg0, arg1) {
            arg0.mask = arg1 >>> 0;
        }, arguments); },
        __wbg_set_max_anisotropy_fb4bae64cb5acf57: function() { return logError(function (arg0, arg1) {
            arg0.maxAnisotropy = arg1;
        }, arguments); },
        __wbg_set_min_binding_size_308360802ae7a9ba: function() { return logError(function (arg0, arg1) {
            arg0.minBindingSize = arg1;
        }, arguments); },
        __wbg_set_min_filter_2dafbdeb188fd817: function() { return logError(function (arg0, arg1) {
            arg0.minFilter = __wbindgen_enum_GpuFilterMode[arg1];
        }, arguments); },
        __wbg_set_mip_level_4b256e2cda4a4c5c: function() { return logError(function (arg0, arg1) {
            arg0.mipLevel = arg1 >>> 0;
        }, arguments); },
        __wbg_set_mip_level_babe1ff64201f0ea: function() { return logError(function (arg0, arg1) {
            arg0.mipLevel = arg1 >>> 0;
        }, arguments); },
        __wbg_set_mip_level_count_cd3197411f4f2432: function() { return logError(function (arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        }, arguments); },
        __wbg_set_mip_level_count_fdc72450a94244ef: function() { return logError(function (arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        }, arguments); },
        __wbg_set_mipmap_filter_79f552c459e63aa6: function() { return logError(function (arg0, arg1) {
            arg0.mipmapFilter = __wbindgen_enum_GpuMipmapFilterMode[arg1];
        }, arguments); },
        __wbg_set_module_18d541838665d831: function() { return logError(function (arg0, arg1) {
            arg0.module = arg1;
        }, arguments); },
        __wbg_set_module_20641353ebb28712: function() { return logError(function (arg0, arg1) {
            arg0.module = arg1;
        }, arguments); },
        __wbg_set_module_6ece909be28666dd: function() { return logError(function (arg0, arg1) {
            arg0.module = arg1;
        }, arguments); },
        __wbg_set_multisample_e0f310ea9e40c2d9: function() { return logError(function (arg0, arg1) {
            arg0.multisample = arg1;
        }, arguments); },
        __wbg_set_multisampled_cd50d8f6709cea1a: function() { return logError(function (arg0, arg1) {
            arg0.multisampled = arg1 !== 0;
        }, arguments); },
        __wbg_set_offset_2e78915f5d65d704: function() { return logError(function (arg0, arg1) {
            arg0.offset = arg1;
        }, arguments); },
        __wbg_set_offset_405017033a936d89: function() { return logError(function (arg0, arg1) {
            arg0.offset = arg1;
        }, arguments); },
        __wbg_set_offset_e7ce8b8eaaf46b95: function() { return logError(function (arg0, arg1) {
            arg0.offset = arg1;
        }, arguments); },
        __wbg_set_offset_efe9880f803c2500: function() { return logError(function (arg0, arg1) {
            arg0.offset = arg1;
        }, arguments); },
        __wbg_set_onuncapturederror_87bababe367ddca7: function() { return logError(function (arg0, arg1) {
            arg0.onuncapturederror = arg1;
        }, arguments); },
        __wbg_set_operation_b96fabca3716aaa3: function() { return logError(function (arg0, arg1) {
            arg0.operation = __wbindgen_enum_GpuBlendOperation[arg1];
        }, arguments); },
        __wbg_set_origin_7561d69f0dc1ba08: function() { return logError(function (arg0, arg1) {
            arg0.origin = arg1;
        }, arguments); },
        __wbg_set_origin_c2a973e78d223dd6: function() { return logError(function (arg0, arg1) {
            arg0.origin = arg1;
        }, arguments); },
        __wbg_set_origin_c5f017d3f09ad7ff: function() { return logError(function (arg0, arg1) {
            arg0.origin = arg1;
        }, arguments); },
        __wbg_set_pass_op_765be90bb2f27220: function() { return logError(function (arg0, arg1) {
            arg0.passOp = __wbindgen_enum_GpuStencilOperation[arg1];
        }, arguments); },
        __wbg_set_passive_f411e67e6f28687b: function() { return logError(function (arg0, arg1) {
            arg0.passive = arg1 !== 0;
        }, arguments); },
        __wbg_set_power_preference_39b347bf0d236ce6: function() { return logError(function (arg0, arg1) {
            arg0.powerPreference = __wbindgen_enum_GpuPowerPreference[arg1];
        }, arguments); },
        __wbg_set_premultiplied_alpha_3664301ff462d8bc: function() { return logError(function (arg0, arg1) {
            arg0.premultipliedAlpha = arg1 !== 0;
        }, arguments); },
        __wbg_set_primitive_d6456d7efe6b4fe5: function() { return logError(function (arg0, arg1) {
            arg0.primitive = arg1;
        }, arguments); },
        __wbg_set_query_set_20ecd7f9a16f3ec6: function() { return logError(function (arg0, arg1) {
            arg0.querySet = arg1;
        }, arguments); },
        __wbg_set_query_set_3afc955600bc819a: function() { return logError(function (arg0, arg1) {
            arg0.querySet = arg1;
        }, arguments); },
        __wbg_set_r_07bd987697069496: function() { return logError(function (arg0, arg1) {
            arg0.r = arg1;
        }, arguments); },
        __wbg_set_required_features_650c9e5dafbaa395: function() { return logError(function (arg0, arg1) {
            arg0.requiredFeatures = arg1;
        }, arguments); },
        __wbg_set_resolve_target_c18cd4048765732a: function() { return logError(function (arg0, arg1) {
            arg0.resolveTarget = arg1;
        }, arguments); },
        __wbg_set_resource_8cea0fe2c8745c3e: function() { return logError(function (arg0, arg1) {
            arg0.resource = arg1;
        }, arguments); },
        __wbg_set_rows_per_image_2f7969031c71f0d8: function() { return logError(function (arg0, arg1) {
            arg0.rowsPerImage = arg1 >>> 0;
        }, arguments); },
        __wbg_set_rows_per_image_a5295fffedd9f061: function() { return logError(function (arg0, arg1) {
            arg0.rowsPerImage = arg1 >>> 0;
        }, arguments); },
        __wbg_set_sample_count_07aedd28692aeae8: function() { return logError(function (arg0, arg1) {
            arg0.sampleCount = arg1 >>> 0;
        }, arguments); },
        __wbg_set_sample_count_483b61e508f24e85: function() { return logError(function (arg0, arg1) {
            arg0.sampleCount = arg1 >>> 0;
        }, arguments); },
        __wbg_set_sample_type_601a744a4bd6ea07: function() { return logError(function (arg0, arg1) {
            arg0.sampleType = __wbindgen_enum_GpuTextureSampleType[arg1];
        }, arguments); },
        __wbg_set_sampler_1a2729c0aa194081: function() { return logError(function (arg0, arg1) {
            arg0.sampler = arg1;
        }, arguments); },
        __wbg_set_shader_location_bdcfdc1009d351b1: function() { return logError(function (arg0, arg1) {
            arg0.shaderLocation = arg1 >>> 0;
        }, arguments); },
        __wbg_set_size_7a392ee585f87da8: function() { return logError(function (arg0, arg1) {
            arg0.size = arg1;
        }, arguments); },
        __wbg_set_size_c6bf409f70f4420f: function() { return logError(function (arg0, arg1) {
            arg0.size = arg1;
        }, arguments); },
        __wbg_set_size_f902b266d636bf6e: function() { return logError(function (arg0, arg1) {
            arg0.size = arg1;
        }, arguments); },
        __wbg_set_source_e41ff077cbd9b133: function() { return logError(function (arg0, arg1) {
            arg0.source = arg1;
        }, arguments); },
        __wbg_set_src_factor_50cef27aa8aece91: function() { return logError(function (arg0, arg1) {
            arg0.srcFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        }, arguments); },
        __wbg_set_stencil_back_e740415a5c0b637a: function() { return logError(function (arg0, arg1) {
            arg0.stencilBack = arg1;
        }, arguments); },
        __wbg_set_stencil_clear_value_6be76b512040398d: function() { return logError(function (arg0, arg1) {
            arg0.stencilClearValue = arg1 >>> 0;
        }, arguments); },
        __wbg_set_stencil_front_03185e1c3bafa411: function() { return logError(function (arg0, arg1) {
            arg0.stencilFront = arg1;
        }, arguments); },
        __wbg_set_stencil_load_op_084f44352b978b3d: function() { return logError(function (arg0, arg1) {
            arg0.stencilLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        }, arguments); },
        __wbg_set_stencil_read_mask_e2736fc4af9399e4: function() { return logError(function (arg0, arg1) {
            arg0.stencilReadMask = arg1 >>> 0;
        }, arguments); },
        __wbg_set_stencil_read_only_31f3d99299373c12: function() { return logError(function (arg0, arg1) {
            arg0.stencilReadOnly = arg1 !== 0;
        }, arguments); },
        __wbg_set_stencil_read_only_cea23ff30262cbb2: function() { return logError(function (arg0, arg1) {
            arg0.stencilReadOnly = arg1 !== 0;
        }, arguments); },
        __wbg_set_stencil_store_op_428fb4955e4899d6: function() { return logError(function (arg0, arg1) {
            arg0.stencilStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        }, arguments); },
        __wbg_set_stencil_write_mask_b1d3e1655305a187: function() { return logError(function (arg0, arg1) {
            arg0.stencilWriteMask = arg1 >>> 0;
        }, arguments); },
        __wbg_set_step_mode_98e49f7877daf1c5: function() { return logError(function (arg0, arg1) {
            arg0.stepMode = __wbindgen_enum_GpuVertexStepMode[arg1];
        }, arguments); },
        __wbg_set_storage_texture_6ee0cbeb50698110: function() { return logError(function (arg0, arg1) {
            arg0.storageTexture = arg1;
        }, arguments); },
        __wbg_set_store_op_e761080d541a10cc: function() { return logError(function (arg0, arg1) {
            arg0.storeOp = __wbindgen_enum_GpuStoreOp[arg1];
        }, arguments); },
        __wbg_set_strip_index_format_16df9e33c7aa97e6: function() { return logError(function (arg0, arg1) {
            arg0.stripIndexFormat = __wbindgen_enum_GpuIndexFormat[arg1];
        }, arguments); },
        __wbg_set_strokeStyle_087121ed5350b038: function() { return logError(function (arg0, arg1, arg2) {
            arg0.strokeStyle = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_targets_9fd1ec0b8edc895c: function() { return logError(function (arg0, arg1) {
            arg0.targets = arg1;
        }, arguments); },
        __wbg_set_textAlign_cdfa5b9f1c14f5c6: function() { return logError(function (arg0, arg1, arg2) {
            arg0.textAlign = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_textBaseline_c7ec6538cc52b073: function() { return logError(function (arg0, arg1, arg2) {
            arg0.textBaseline = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_texture_b0ca3ffcb0c2688c: function() { return logError(function (arg0, arg1) {
            arg0.texture = arg1;
        }, arguments); },
        __wbg_set_texture_f03807916f70dcc6: function() { return logError(function (arg0, arg1) {
            arg0.texture = arg1;
        }, arguments); },
        __wbg_set_texture_f8ae0bb4bb159354: function() { return logError(function (arg0, arg1) {
            arg0.texture = arg1;
        }, arguments); },
        __wbg_set_timestamp_writes_3998dbfa21e48dbe: function() { return logError(function (arg0, arg1) {
            arg0.timestampWrites = arg1;
        }, arguments); },
        __wbg_set_timestamp_writes_de925214f236e575: function() { return logError(function (arg0, arg1) {
            arg0.timestampWrites = arg1;
        }, arguments); },
        __wbg_set_topology_036632318a24227d: function() { return logError(function (arg0, arg1) {
            arg0.topology = __wbindgen_enum_GpuPrimitiveTopology[arg1];
        }, arguments); },
        __wbg_set_type_0cb4cdb5eff87f31: function() { return logError(function (arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuBufferBindingType[arg1];
        }, arguments); },
        __wbg_set_type_3099d48161846862: function() { return logError(function (arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuQueryType[arg1];
        }, arguments); },
        __wbg_set_type_d05fa8415ad0761f: function() { return logError(function (arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuSamplerBindingType[arg1];
        }, arguments); },
        __wbg_set_unclipped_depth_17a5ab83d4e7cadc: function() { return logError(function (arg0, arg1) {
            arg0.unclippedDepth = arg1 !== 0;
        }, arguments); },
        __wbg_set_usage_3d569e7b02227032: function() { return logError(function (arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        }, arguments); },
        __wbg_set_usage_ac222ece73f994b7: function() { return logError(function (arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        }, arguments); },
        __wbg_set_usage_ca00520767c8a475: function() { return logError(function (arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        }, arguments); },
        __wbg_set_usage_fe13088353b65bee: function() { return logError(function (arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        }, arguments); },
        __wbg_set_vertex_76b7ac4bdfbb06f4: function() { return logError(function (arg0, arg1) {
            arg0.vertex = arg1;
        }, arguments); },
        __wbg_set_view_1ef41eeb26eaf718: function() { return logError(function (arg0, arg1) {
            arg0.view = arg1;
        }, arguments); },
        __wbg_set_view_46b654a12649c6f6: function() { return logError(function (arg0, arg1) {
            arg0.view = arg1;
        }, arguments); },
        __wbg_set_view_dimension_12c332494a2697dc: function() { return logError(function (arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        }, arguments); },
        __wbg_set_view_dimension_31b9fd7126132e82: function() { return logError(function (arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        }, arguments); },
        __wbg_set_view_formats_152cb995add2ee4e: function() { return logError(function (arg0, arg1) {
            arg0.viewFormats = arg1;
        }, arguments); },
        __wbg_set_view_formats_cc77650da6c3b25b: function() { return logError(function (arg0, arg1) {
            arg0.viewFormats = arg1;
        }, arguments); },
        __wbg_set_visibility_6d1fc94552f22ac3: function() { return logError(function (arg0, arg1) {
            arg0.visibility = arg1 >>> 0;
        }, arguments); },
        __wbg_set_width_5ee1e2d4a0fd929b: function() { return logError(function (arg0, arg1) {
            arg0.width = arg1 >>> 0;
        }, arguments); },
        __wbg_set_width_7f07715a20503914: function() { return logError(function (arg0, arg1) {
            arg0.width = arg1 >>> 0;
        }, arguments); },
        __wbg_set_width_d60bc4f2f20c56a4: function() { return logError(function (arg0, arg1) {
            arg0.width = arg1 >>> 0;
        }, arguments); },
        __wbg_set_write_mask_c92743022356850e: function() { return logError(function (arg0, arg1) {
            arg0.writeMask = arg1 >>> 0;
        }, arguments); },
        __wbg_set_x_0771b0f86d56cdf9: function() { return logError(function (arg0, arg1) {
            arg0.x = arg1 >>> 0;
        }, arguments); },
        __wbg_set_x_7bd1a7929fa138eb: function() { return logError(function (arg0, arg1) {
            arg0.x = arg1 >>> 0;
        }, arguments); },
        __wbg_set_y_37d5f6d9b550b4ad: function() { return logError(function (arg0, arg1) {
            arg0.y = arg1 >>> 0;
        }, arguments); },
        __wbg_set_y_668d1578881576dd: function() { return logError(function (arg0, arg1) {
            arg0.y = arg1 >>> 0;
        }, arguments); },
        __wbg_set_z_3e24a918a76c816d: function() { return logError(function (arg0, arg1) {
            arg0.z = arg1 >>> 0;
        }, arguments); },
        __wbg_shiftKey_5558a3288542c985: function() { return logError(function (arg0) {
            const ret = arg0.shiftKey;
            _assertBoolean(ret);
            return ret;
        }, arguments); },
        __wbg_size_0ee2999debd2b5d2: function() { return logError(function (arg0) {
            const ret = arg0.size;
            return ret;
        }, arguments); },
        __wbg_stack_0ed75d68575b0f3c: function() { return logError(function (arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_static_accessor_GLOBAL_12837167ad935116: function() { return logError(function () {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_static_accessor_GLOBAL_THIS_e628e89ab3b1c95f: function() { return logError(function () {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_static_accessor_SELF_a621d3dfbb60d0ce: function() { return logError(function () {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_static_accessor_WINDOW_f8727f0cf888e0bd: function() { return logError(function () {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_stroke_240ea7f2407d73c0: function() { return logError(function (arg0) {
            arg0.stroke();
        }, arguments); },
        __wbg_style_0b7c9bd318f8b807: function() { return logError(function (arg0) {
            const ret = arg0.style;
            return ret;
        }, arguments); },
        __wbg_submit_a1850a1cb6baf64a: function() { return logError(function (arg0, arg1) {
            arg0.submit(arg1);
        }, arguments); },
        __wbg_target_0448c1b49e7df279: function() { return logError(function (arg0) {
            const ret = arg0.target;
            return ret;
        }, arguments); },
        __wbg_target_521be630ab05b11e: function() { return logError(function (arg0) {
            const ret = arg0.target;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_then_0d9fe2c7b1857d32: function() { return logError(function (arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_then_b9e7b3b5f1a9e1b5: function() { return logError(function (arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        }, arguments); },
        __wbg_toDataURL_69906a7a2b57e270: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.toDataURL(getStringFromWasm0(arg2, arg3));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_top_3d27ff6f468cf3fc: function() { return logError(function (arg0) {
            const ret = arg0.top;
            return ret;
        }, arguments); },
        __wbg_touches_55ce167b42bcdf52: function() { return logError(function (arg0) {
            const ret = arg0.touches;
            return ret;
        }, arguments); },
        __wbg_translate_3aa10730376a8c06: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.translate(arg1, arg2);
        }, arguments); },
        __wbg_unmap_ab94ab04cfb14bee: function() { return logError(function (arg0) {
            arg0.unmap();
        }, arguments); },
        __wbg_usage_6fec626a30cc0aff: function() { return logError(function (arg0) {
            const ret = arg0.usage;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_valueOf_3c28600026e653c4: function() { return logError(function (arg0) {
            const ret = arg0.valueOf();
            return ret;
        }, arguments); },
        __wbg_value_0546255b415e96c1: function() { return logError(function (arg0) {
            const ret = arg0.value;
            return ret;
        }, arguments); },
        __wbg_warn_f7ae1b2e66ccb930: function() { return logError(function (arg0) {
            console.warn(arg0);
        }, arguments); },
        __wbg_wgslLanguageFeatures_a6dc1ac6bcdcb1ca: function() { return logError(function (arg0) {
            const ret = arg0.wgslLanguageFeatures;
            return ret;
        }, arguments); },
        __wbg_width_5f66bde2e810fbde: function() { return logError(function (arg0) {
            const ret = arg0.width;
            _assertNum(ret);
            return ret;
        }, arguments); },
        __wbg_width_9bbf873307a2ac4e: function() { return logError(function (arg0) {
            const ret = arg0.width;
            return ret;
        }, arguments); },
        __wbg_width_ae46cb8e98ee102f: function() { return logError(function (arg0) {
            const ret = arg0.width;
            return ret;
        }, arguments); },
        __wbg_writeBuffer_b203cf79b98d6dd8: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.writeBuffer(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_writeTexture_0466bf7d7d35e04e: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.writeTexture(arg1, arg2, arg3, arg4);
        }, arguments); },
        __wbindgen_cast_0000000000000001: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 1114, function: Function { arguments: [Externref], shim_idx: 1115, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h09ff9d2157ef0cac, wasm_bindgen__convert__closures_____invoke__h18aee172ef939116);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000002: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 180, function: Function { arguments: [], shim_idx: 71, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h7189099d4ae87eba, wasm_bindgen__convert__closures_____invoke__hffdacf2875d1a35e);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000003: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 181, function: Function { arguments: [NamedExternref("TouchEvent")], shim_idx: 76, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h7786fa800d11f20a, wasm_bindgen__convert__closures_____invoke__h5b0fed36bfe92046);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000004: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 182, function: Function { arguments: [NamedExternref("WheelEvent")], shim_idx: 73, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h92d277a9e73a8830, wasm_bindgen__convert__closures_____invoke__h2a4c4588e55deacf);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000005: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 183, function: Function { arguments: [NamedExternref("Event")], shim_idx: 77, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__ha5b9bd6f60d810f0, wasm_bindgen__convert__closures_____invoke__h0894089cb9abe05b);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000006: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 184, function: Function { arguments: [NamedExternref("PointerEvent")], shim_idx: 75, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h54b5ec4e5daa0037, wasm_bindgen__convert__closures_____invoke__hfb83aab43a7a8ff3);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000007: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 185, function: Function { arguments: [F64], shim_idx: 74, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h0711f49dfcf70ac6, wasm_bindgen__convert__closures_____invoke__h160af521f7969f79);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000008: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 186, function: Function { arguments: [NamedExternref("Array<any>")], shim_idx: 72, ret: Unit, inner_ret: Some(Unit) }, mutable: false }) -> Externref`.
            const ret = makeClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__hc9bffe6cb7397f36, wasm_bindgen__convert__closures_____invoke__h624940a946cabd68);
            return ret;
        }, arguments); },
        __wbindgen_cast_0000000000000009: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 948, function: Function { arguments: [NamedExternref("GPUUncapturedErrorEvent")], shim_idx: 1087, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.wasm_bindgen__closure__destroy__h244aa4cacbfb7934, wasm_bindgen__convert__closures_____invoke__h9c76fb0acc9e105b);
            return ret;
        }, arguments); },
        __wbindgen_cast_000000000000000a: function() { return logError(function (arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        }, arguments); },
        __wbindgen_cast_000000000000000b: function() { return logError(function (arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        }, arguments); },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./aion_charts_wasm_bg.js": import0,
    };
}


//#endregion
function wasm_bindgen__convert__closures_____invoke__hffdacf2875d1a35e(arg0, arg1) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__hffdacf2875d1a35e(arg0, arg1);
}

function wasm_bindgen__convert__closures_____invoke__hf263c0e6d31821ed(arg0, arg1) {
    _assertNum(arg0);
    _assertNum(arg1);
    const ret = wasm.wasm_bindgen__convert__closures_____invoke__hf263c0e6d31821ed(arg0, arg1);
    return ret !== 0;
}

function wasm_bindgen__convert__closures_____invoke__h18aee172ef939116(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h18aee172ef939116(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h5b0fed36bfe92046(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h5b0fed36bfe92046(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h2a4c4588e55deacf(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h2a4c4588e55deacf(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h0894089cb9abe05b(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h0894089cb9abe05b(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__hfb83aab43a7a8ff3(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__hfb83aab43a7a8ff3(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h624940a946cabd68(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h624940a946cabd68(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h9c76fb0acc9e105b(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h9c76fb0acc9e105b(arg0, arg1, arg2);
}

function wasm_bindgen__convert__closures_____invoke__h2e4f7e4bbddf86e9(arg0, arg1, arg2, arg3) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h2e4f7e4bbddf86e9(arg0, arg1, arg2, arg3);
}

function wasm_bindgen__convert__closures_____invoke__h160af521f7969f79(arg0, arg1, arg2) {
    _assertNum(arg0);
    _assertNum(arg1);
    wasm.wasm_bindgen__convert__closures_____invoke__h160af521f7969f79(arg0, arg1, arg2);
}


const __wbindgen_enum_GpuAddressMode = ["clamp-to-edge", "repeat", "mirror-repeat"];


const __wbindgen_enum_GpuBlendFactor = ["zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"];


const __wbindgen_enum_GpuBlendOperation = ["add", "subtract", "reverse-subtract", "min", "max"];


const __wbindgen_enum_GpuBufferBindingType = ["uniform", "storage", "read-only-storage"];


const __wbindgen_enum_GpuCanvasAlphaMode = ["opaque", "premultiplied"];


const __wbindgen_enum_GpuCompareFunction = ["never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"];


const __wbindgen_enum_GpuCullMode = ["none", "front", "back"];


const __wbindgen_enum_GpuDeviceLostReason = ["unknown", "destroyed"];


const __wbindgen_enum_GpuErrorFilter = ["validation", "out-of-memory", "internal"];


const __wbindgen_enum_GpuFilterMode = ["nearest", "linear"];


const __wbindgen_enum_GpuFrontFace = ["ccw", "cw"];


const __wbindgen_enum_GpuIndexFormat = ["uint16", "uint32"];


const __wbindgen_enum_GpuLoadOp = ["load", "clear"];


const __wbindgen_enum_GpuMipmapFilterMode = ["nearest", "linear"];


const __wbindgen_enum_GpuPowerPreference = ["low-power", "high-performance"];


const __wbindgen_enum_GpuPrimitiveTopology = ["point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"];


const __wbindgen_enum_GpuQueryType = ["occlusion", "timestamp"];


const __wbindgen_enum_GpuSamplerBindingType = ["filtering", "non-filtering", "comparison"];


const __wbindgen_enum_GpuStencilOperation = ["keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"];


const __wbindgen_enum_GpuStorageTextureAccess = ["write-only", "read-only", "read-write"];


const __wbindgen_enum_GpuStoreOp = ["store", "discard"];


const __wbindgen_enum_GpuTextureAspect = ["all", "stencil-only", "depth-only"];


const __wbindgen_enum_GpuTextureDimension = ["1d", "2d", "3d"];


const __wbindgen_enum_GpuTextureFormat = ["r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"];


const __wbindgen_enum_GpuTextureSampleType = ["float", "unfilterable-float", "depth", "sint", "uint"];


const __wbindgen_enum_GpuTextureViewDimension = ["1d", "2d", "2d-array", "cube", "cube-array", "3d"];


const __wbindgen_enum_GpuVertexFormat = ["uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"];


const __wbindgen_enum_GpuVertexStepMode = ["vertex", "instance"];
const Aion_chartsFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_aion_charts_free(ptr >>> 0, 1));
const ChartGroupFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_chartgroup_free(ptr >>> 0, 1));
const ChartWorkspaceFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_chartworkspace_free(ptr >>> 0, 1));
const IndicatorWorkerRuntimeFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_indicatorworkerruntime_free(ptr >>> 0, 1));


//#region intrinsics
function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function _assertBigInt(n) {
    if (typeof(n) !== 'bigint') throw new Error(`expected a bigint argument, found ${typeof(n)}`);
}

function _assertBoolean(n) {
    if (typeof(n) !== 'boolean') {
        throw new Error(`expected a boolean argument, found ${typeof(n)}`);
    }
}

function _assertNum(n) {
    if (typeof(n) !== 'number') throw new Error(`expected a number argument, found ${typeof(n)}`);
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => state.dtor(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedBigUint64ArrayMemory0 = null;
function getBigUint64ArrayMemory0() {
    if (cachedBigUint64ArrayMemory0 === null || cachedBigUint64ArrayMemory0.byteLength === 0) {
        cachedBigUint64ArrayMemory0 = new BigUint64Array(wasm.memory.buffer);
    }
    return cachedBigUint64ArrayMemory0;
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function logError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        let error = (function () {
            try {
                return e instanceof Error ? `${e.message}\n\nStack:\n${e.stack}` : e.toString();
            } catch(_) {
                return "<failed to stringify thrown value>";
            }
        }());
        console.error("wasm-bindgen: imported JS function that was not marked as `catch` threw an error:", error);
        throw e;
    }
}

function makeClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        try {
            return f(state.a, state.b, ...args);
        } finally {
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getBigUint64ArrayMemory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getFloat64ArrayMemory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (typeof(arg) !== 'string') throw new Error(`expected a string argument, found ${typeof(arg)}`);
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);
        if (ret.read !== arg.length) throw new Error('failed to pass whole string');
        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;


//#endregion

//#region wasm loading
let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedBigUint64ArrayMemory0 = null;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('aion_charts_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
//#endregion
export { wasm as __wasm }

/**
 * Handle implementations over the wasm engine: series, time scale, price scale, pane, chart.
 * Extracted from `index.ts`.
 */

// @ts-ignore -- pkg is a build artifact, present after build:wasm
import init, { AionChart } from "../pkg/aion_wasm.js";

import { install_gestures } from "./gestures.js";
import type {
  bars_info, chart_api, chart_options, data_changed_handler, dbl_click_handler,
  deep_partial, handle_scale_options, handle_scroll_options, kinetic_scroll_options,
  localization_options, logical_range,
  mismatch_direction, mouse_event_handler, mouse_event_params, ohlc_data, pane_api, price_line_api, price_line_options,
  price_range, price_scale_api, price_scale_options, series_api, series_data, series_kind,
  series_marker, series_marker_options, series_options, single_value_data, time, time_range,
  time_scale_api, time_scale_options, tracking_mode_options, visible_logical_range_handler, visible_time_range_handler,
} from "./types.js";
import { KIND_TO_U8, LINE_STYLE_TO_U8, LINE_TYPE_TO_U8 } from "./types.js";

// ---------------------------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------------------------

let init_promise: Promise<unknown> | null = null;
/** Instantiate the wasm module once per page. */
export function ensure_init(): Promise<unknown> {
  if (init_promise === null) {
    init_promise = init();
  }
  return init_promise;
}

function undef_to_null<T>(v: T | undefined): T | null {
  return v === undefined ? null : v;
}

function same_logical_range(a: logical_range | null, b: logical_range | null): boolean {
  return a === b || (a !== null && b !== null && a.from === b.from && a.to === b.to);
}

function same_time_range(a: time_range | null, b: time_range | null): boolean {
  return a === b || (a !== null && b !== null && a.from === b.from && a.to === b.to);
}

/** Pack a data array into the six Float64Arrays the engine expects (single-value → o=h=l=c). */
/**
 * Convert a `time` input to the engine's UTC-seconds form. Business days and `"YYYY-MM-DD"` strings
 * are taken at UTC midnight (matching LWC's `Date.UTC(...)/1000`). A malformed value yields `NaN`,
 * which the engine's sanitizer drops as an invalid row.
 */
function time_to_utc_seconds(t: time): number {
  if (typeof t === "number") return t;
  if (typeof t === "string") {
    const [y, m, d] = t.split("-").map(Number);
    return Date.UTC(y ?? NaN, (m ?? 1) - 1, d ?? 1) / 1000;
  }
  return Date.UTC(t.year, t.month - 1, t.day) / 1000;
}

function pack(data: readonly series_data[]): {
  times: Float64Array;
  open: Float64Array;
  high: Float64Array;
  low: Float64Array;
  close: Float64Array;
} {
  const n = data.length;
  const times = new Float64Array(n);
  const open = new Float64Array(n);
  const high = new Float64Array(n);
  const low = new Float64Array(n);
  const close = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const d = data[i] as series_data;
    times[i] = time_to_utc_seconds(d.time);
    if ("value" in d) {
      open[i] = high[i] = low[i] = close[i] = d.value;
    } else {
      open[i] = d.open;
      high[i] = d.high;
      low[i] = d.low;
      close[i] = d.close;
    }
  }
  return { times, open, high, low, close };
}

/** Parse a CSS hex or rgb()/rgba() color to 8-bit channels (mirrors the Rust `Color::parse_css`). */
function parse_rgb(css: string): [number, number, number] | null {
  const s = css.trim();
  if (s.startsWith("#")) {
    const h = s.slice(1);
    const expand = (c: string) => parseInt(c + c, 16);
    if (h.length === 3 || h.length === 4) {
      return [expand(h[0]!), expand(h[1]!), expand(h[2]!)];
    }
    if (h.length === 6 || h.length === 8) {
      return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
    }
    return null;
  }
  const m = s.match(/^rgba?\(([^)]+)\)$/i);
  if (m) {
    const parts = m[1]!.split(",").map((p) => parseFloat(p.trim()));
    if (parts.length >= 3 && parts.every((p) => !Number.isNaN(p))) {
      return [Math.round(parts[0]!), Math.round(parts[1]!), Math.round(parts[2]!)];
    }
  }
  return null;
}

class series_impl implements series_api {
  private readonly data_changed_subs = new Set<data_changed_handler>();
  private removed = false;

  constructor(
    readonly id: number,
    readonly kind: series_kind,
    private readonly chart: chart_impl,
  ) {}

  /** Called by `chart_impl.remove_series`; makes every subsequent method on this handle throw. */
  mark_removed(): void {
    this.removed = true;
    this.data_changed_subs.clear();
  }
  private assert_live(): void {
    if (this.removed) throw new Error("aion: this series has been removed from the chart");
  }

  set_data(data: readonly series_data[]): void {
    this.assert_live();
    const p = pack(data);
    this.chart.wasm.set_series_data_typed(this.id, p.times, p.open, p.high, p.low, p.close);
    this.chart.repaint();
    for (const handler of this.data_changed_subs) handler("full");
  }

  update(point: series_data): void {
    this.assert_live();
    const o = "value" in point ? point.value : point.open;
    const h = "value" in point ? point.value : point.high;
    const l = "value" in point ? point.value : point.low;
    const c = "value" in point ? point.value : point.close;
    // Series-scoped streaming: append a new time point or replace the last on this series.
    this.chart.wasm.update_series_bar(this.id, time_to_utc_seconds(point.time), o, h, l, c);
    this.chart.repaint();
    for (const handler of this.data_changed_subs) handler("update");
  }

  apply_options(options: Partial<series_options>): void {
    this.assert_live();
    if (options.color !== undefined) {
      const rgb = parse_rgb(options.color);
      if (rgb) {
        this.chart.wasm.set_series_color(this.id, rgb[0], rgb[1], rgb[2]);
      }
    }
    if (options.visible !== undefined) {
      this.chart.wasm.set_series_visible(this.id, options.visible);
    }
    if (options.up_color !== undefined || options.down_color !== undefined) {
      // CSS strings passed through so the engine keeps alpha; empty = leave unchanged.
      this.chart.wasm.set_series_updown_colors(this.id, options.up_color ?? "", options.down_color ?? "");
    }
    if (options.wick_up_color !== undefined || options.wick_down_color !== undefined) {
      // Pass each direction through unchanged: undefined = keep, "" = clear (follow body), CSS = pin.
      // (A plain `?? ""` here would wrongly clear the direction the caller left unspecified.)
      this.chart.wasm.set_series_wick_colors(this.id, options.wick_up_color, options.wick_down_color);
    }
    if (options.border_up_color !== undefined || options.border_down_color !== undefined) {
      this.chart.wasm.set_series_border_colors(this.id, options.border_up_color, options.border_down_color);
    }
    if (options.wick_visible !== undefined) {
      this.chart.wasm.set_series_wick_visible(this.id, options.wick_visible);
    }
    if (options.border_visible !== undefined) {
      this.chart.wasm.set_series_border_visible(this.id, options.border_visible);
    }
    if (options.line_width !== undefined) {
      this.chart.wasm.set_series_line_width(this.id, options.line_width);
    }
    if (options.area_top_color !== undefined || options.area_bottom_color !== undefined) {
      this.chart.wasm.set_series_area_colors(this.id, options.area_top_color ?? "", options.area_bottom_color ?? "");
    }
    if (options.histogram_updown !== undefined) {
      this.chart.wasm.set_series_histogram_updown(this.id, options.histogram_updown);
    }
    if (options.pane !== undefined && options.pane > 0) {
      this.chart.wasm.set_series_pane(this.id, options.pane, options.pane_stretch ?? 1);
    }
    if (options.line_type !== undefined) {
      this.chart.wasm.set_series_line_type(this.id, LINE_TYPE_TO_U8[options.line_type]);
    }
    if (options.point_markers !== undefined) {
      this.chart.wasm.set_series_point_markers(this.id, options.point_markers);
    }
    if (options.baseline_value !== undefined) {
      this.chart.wasm.set_series_baseline(this.id, options.baseline_value);
    }
    if (options.overlay) {
      const m = options.scale_margins ?? { top: 0.8, bottom: 0 };
      this.chart.wasm.set_series_overlay(this.id, m.top, m.bottom);
    } else if (options.price_scale_id !== undefined || options.priceScaleId !== undefined) {
      const id = options.priceScaleId ?? options.price_scale_id;
      const target = id === "left" ? 1 : id === "" ? 2 : 0;
      this.chart.wasm.set_series_price_scale(this.id, target);
      if (target === 2) {
        const m = options.scale_margins ?? { top: 0.8, bottom: 0 };
        this.chart.wasm.set_series_overlay(this.id, m.top, m.bottom);
      }
    }
    if (options.last_price_animation !== undefined) {
      this.chart.wasm.set_series_last_price_animation(this.id, options.last_price_animation);
      this.chart.sync_animation();
    }
    this.chart.repaint();
  }

  set_type(kind: series_kind): void {
    this.assert_live();
    if (this.id === 0) {
      this.chart.wasm.set_series_type(KIND_TO_U8[kind]);
    } else {
      console.warn("aion: set_type() currently supports the primary series only");
    }
    this.chart.repaint();
  }

  move_to_pane(pane_index: number, stretch = 1): void {
    this.assert_live();
    this.chart.wasm.set_series_pane(this.id, pane_index, stretch);
    this.chart.repaint();
  }

  create_price_line(options: price_line_options): price_line_api {
    this.assert_live();
    const rgb = parse_rgb(options.color ?? "#2196f3") ?? [0x21, 0x96, 0xf3];
    const style = LINE_STYLE_TO_U8[options.line_style ?? "solid"];
    const id = this.chart.wasm.create_price_line(
      this.id,
      options.price,
      rgb[0],
      rgb[1],
      rgb[2],
      options.line_width ?? 1,
      style,
      options.title ?? "",
    );
    this.chart.repaint();
    const chart = this.chart;
    return {
      id,
      remove() {
        chart.wasm.remove_price_line(id);
        chart.repaint();
      },
    };
  }

  set_markers(markers: readonly series_marker[], options?: Partial<series_marker_options>): void {
    this.assert_live();
    if (options?.auto_scale !== undefined) {
      this.chart.wasm.set_series_markers_auto_scale(this.id, options.auto_scale);
    }
    // Normalize marker times to UTC seconds so business-day/string forms match their data points
    // (the engine's marker JSON expects a numeric time).
    const normalized = markers.map((mk) => ({ ...mk, time: time_to_utc_seconds(mk.time) }));
    this.chart.wasm.set_series_markers(this.id, JSON.stringify(normalized));
    this.chart.repaint();
  }

  price_scale(): price_scale_api {
    const pane = undef_to_null(this.chart.wasm.series_pane_index(this.id)) ?? 0;
    const target = undef_to_null(this.chart.wasm.series_price_scale_id(this.id)) ?? 0;
    return new price_scale_impl(this.chart, pane, target);
  }
  price_to_coordinate(price: number): number | null {
    return undef_to_null(this.chart.wasm.series_price_to_coordinate(this.id, price));
  }
  coordinate_to_price(coordinate: number): number | null {
    return undef_to_null(this.chart.wasm.series_coordinate_to_price(this.id, coordinate));
  }
  bars_in_logical_range(range: logical_range): bars_info | null {
    const info = this.chart.wasm.series_bars_in_logical_range(this.id, range.from, range.to);
    if (info.length < 2) return null;
    return info.length === 4
      ? { bars_before: info[0]!, bars_after: info[1]!, from: info[2]!, to: info[3]! }
      : { bars_before: info[0]!, bars_after: info[1]! };
  }
  private unpack_point(values: Float64Array | number[], offset = 0): series_data | null {
    if (values.length < offset + 5) return null;
    const time = values[offset]!;
    if (this.series_type() === "candlestick" || this.series_type() === "bar") {
      return {
        time,
        open: values[offset + 1]!,
        high: values[offset + 2]!,
        low: values[offset + 3]!,
        close: values[offset + 4]!,
      };
    }
    return { time, value: values[offset + 4]! };
  }
  data_by_index(logical_index: number, mismatch_direction: mismatch_direction = 0): series_data | null {
    return this.unpack_point(
      this.chart.wasm.series_data_by_index(this.id, logical_index, mismatch_direction),
    );
  }
  data(): readonly series_data[] {
    const values = this.chart.wasm.series_data(this.id);
    const output: series_data[] = [];
    for (let offset = 0; offset + 4 < values.length; offset += 5) {
      const point = this.unpack_point(values, offset);
      if (point !== null) output.push(point);
    }
    return output;
  }
  series_type(): series_kind {
    const kind = this.chart.wasm.series_kind(this.id) ?? KIND_TO_U8[this.kind];
    return (["candlestick", "bar", "line", "area", "histogram", "baseline"] as const)[kind]
      ?? "candlestick";
  }
  subscribe_data_changed(handler: data_changed_handler): void {
    this.data_changed_subs.add(handler);
  }
  unsubscribe_data_changed(handler: data_changed_handler): void {
    this.data_changed_subs.delete(handler);
  }
}

class time_scale_impl implements time_scale_api {
  constructor(private readonly chart: chart_impl) {}

  scroll_position(): number {
    return this.chart.wasm.scroll_position();
  }
  scroll_to_position(position: number, _animated: boolean): void {
    this.chart.wasm.scroll_to_position(position);
    this.chart.repaint();
  }
  scroll_to_real_time(): void {
    this.chart.wasm.scroll_to_real_time();
    this.chart.repaint();
  }
  reset_time_scale(): void {
    this.chart.wasm.reset_time_scale();
    this.chart.repaint();
  }
  fit_content(): void {
    this.chart.wasm.fit_content();
    this.chart.repaint();
  }
  apply_options(options: Partial<time_scale_options>): void {
    if (options.bar_spacing !== undefined) this.chart.wasm.set_bar_spacing(options.bar_spacing);
    if (options.right_offset !== undefined) this.chart.wasm.set_right_offset(options.right_offset);
    if (options.min_bar_spacing !== undefined) this.chart.wasm.set_min_bar_spacing(options.min_bar_spacing);
    if (options.time_visible !== undefined) this.chart.wasm.set_time_visible(options.time_visible);
    if (options.seconds_visible !== undefined) this.chart.wasm.set_seconds_visible(options.seconds_visible);
    if (options.fix_left_edge !== undefined) this.chart.wasm.set_fix_left_edge(options.fix_left_edge);
    if (options.fix_right_edge !== undefined) this.chart.wasm.set_fix_right_edge(options.fix_right_edge);
    if (options.lock_visible_time_range_on_resize !== undefined)
      this.chart.wasm.set_lock_visible_time_range_on_resize(options.lock_visible_time_range_on_resize);
    if (options.right_bar_stays_on_scroll !== undefined)
      this.chart.wasm.set_right_bar_stays_on_scroll(options.right_bar_stays_on_scroll);
    if (options.tick_mark_formatter !== undefined)
      this.chart.wasm.set_tick_mark_formatter(options.tick_mark_formatter);
    this.chart.repaint();
  }
  options(): time_scale_options {
    return {
      bar_spacing: this.chart.wasm.bar_spacing(),
      right_offset: this.chart.wasm.right_offset(),
    };
  }
  get_visible_logical_range(): logical_range | null {
    const r = this.chart.wasm.visible_logical_range();
    return r.length === 2 ? { from: r[0]!, to: r[1]! } : null;
  }
  set_visible_logical_range(range: logical_range): void {
    this.chart.wasm.set_visible_logical_range(range.from, range.to);
    this.chart.repaint();
  }
  get_visible_range(): time_range | null {
    const r = this.chart.wasm.visible_time_range();
    return r.length === 2 ? { from: r[0]!, to: r[1]! } : null;
  }
  set_visible_range(range: time_range): void {
    this.chart.wasm.set_visible_time_range(range.from, range.to);
    this.chart.repaint();
  }
  subscribe_visible_logical_range_change(handler: visible_logical_range_handler): void {
    this.chart.subscribe_visible_logical_range_change(handler);
  }
  unsubscribe_visible_logical_range_change(handler: visible_logical_range_handler): void {
    this.chart.unsubscribe_visible_logical_range_change(handler);
  }
  subscribe_visible_time_range_change(handler: visible_time_range_handler): void {
    this.chart.subscribe_visible_time_range_change(handler);
  }
  unsubscribe_visible_time_range_change(handler: visible_time_range_handler): void {
    this.chart.unsubscribe_visible_time_range_change(handler);
  }
  time_to_coordinate(time: number): number | null {
    return undef_to_null(this.chart.wasm.time_to_coordinate(time));
  }
  coordinate_to_time(x: number): number | null {
    return undef_to_null(this.chart.wasm.coordinate_to_time(x));
  }
  logical_to_coordinate(logical: number): number | null {
    return undef_to_null(this.chart.wasm.logical_to_coordinate(logical));
  }
  coordinate_to_logical(x: number): number | null {
    return undef_to_null(this.chart.wasm.coordinate_to_logical(x));
  }
  time_to_index(time: number, find_nearest = false): number | null {
    const index = undef_to_null(this.chart.wasm.time_to_index(time, find_nearest));
    return index === null ? null : Number(index);
  }
  width(): number {
    return this.chart.wasm.time_scale_width();
  }
  height(): number {
    return this.chart.wasm.time_scale_height();
  }
}

class price_scale_impl implements price_scale_api {
  constructor(
    private readonly chart: chart_impl,
    private readonly pane: number,
    private readonly target: number,
  ) {}

  apply_options(options: deep_partial<price_scale_options>): void {
    if (options.mode !== undefined) {
      this.chart.wasm.set_price_scale_mode(this.pane, this.target, options.mode);
    }
    if (options.auto_scale !== undefined) {
      this.chart.wasm.set_price_scale_auto_scale(this.pane, this.target, options.auto_scale);
    }
    if (options.invert_scale !== undefined) {
      this.chart.wasm.set_price_scale_inverted(this.pane, this.target, options.invert_scale);
    }
    if (options.scale_margins !== undefined) {
      const current = this.options().scale_margins;
      this.chart.wasm.set_price_scale_margins(
        this.pane,
        this.target,
        options.scale_margins.top ?? current.top,
        options.scale_margins.bottom ?? current.bottom,
      );
    }
    this.chart.repaint();
  }

  options(): price_scale_options {
    const margins = this.chart.wasm.price_scale_margins(this.pane, this.target);
    return {
      mode: (this.chart.wasm.price_scale_mode(this.pane, this.target) ?? 0) as 0 | 1 | 2 | 3,
      auto_scale: this.chart.wasm.price_scale_auto_scale(this.pane, this.target) ?? false,
      invert_scale: this.chart.wasm.price_scale_inverted(this.pane, this.target) ?? false,
      scale_margins: {
        top: margins.length === 2 ? margins[0]! : 0,
        bottom: margins.length === 2 ? margins[1]! : 0,
      },
    };
  }

  width(): number {
    return this.chart.wasm.price_scale_width(this.pane, this.target);
  }

  set_visible_range(range: price_range): void {
    this.chart.wasm.set_price_scale_visible_range(this.pane, this.target, range.from, range.to);
    this.chart.repaint();
  }

  get_visible_range(): price_range | null {
    const range = this.chart.wasm.price_scale_visible_range(this.pane, this.target);
    return range.length === 2 ? { from: range[0]!, to: range[1]! } : null;
  }

  set_auto_scale(on: boolean): void {
    this.chart.wasm.set_price_scale_auto_scale(this.pane, this.target, on);
    this.chart.repaint();
  }
}

class pane_impl implements pane_api {
  constructor(private readonly chart: chart_impl, private readonly index: number) {}

  pane_index(): number {
    return this.index;
  }
  get_height(): number {
    return this.chart.wasm.pane_height(this.index);
  }
  set_height(height: number): void {
    this.chart.wasm.set_pane_height(this.index, height);
    this.chart.repaint();
  }
  get_stretch_factor(): number {
    return this.chart.wasm.pane_stretch(this.index);
  }
  set_stretch_factor(factor: number): void {
    this.chart.wasm.set_pane_stretch(this.index, factor);
    this.chart.repaint();
  }
}

/** Gesture toggles resolved to concrete values the recognizer reads on each event. */
export interface resolved_gestures {
  pan: boolean;
  pan_horz_touch: boolean;
  pan_vert_touch: boolean;
  wheel_scroll: boolean;
  wheel_zoom: boolean;
  pinch_zoom: boolean;
  axis_dblclick_reset_time: boolean;
  axis_dblclick_reset_price: boolean;
  axis_scale_price: boolean;
  axis_scale_time: boolean;
  kinetic_touch: boolean;
  kinetic_mouse: boolean;
  panes_resize: boolean;
  tracking_exit_mode: "on_next_tap" | "on_touch_end";
}

function apply_scroll(v: boolean | handle_scroll_options, cfg: resolved_gestures): void {
  if (typeof v === "boolean") {
    // LWC migrateHandleScaleScrollOptions: a boolean expands to all four scroll flags.
    cfg.pan = v;
    cfg.pan_horz_touch = v;
    cfg.pan_vert_touch = v;
    cfg.wheel_scroll = v;
    return;
  }
  // Object form merges over the current config; `pan` (the mouse-drag / generic pan) tracks
  // pressed_mouse_move, and the touch axes track their own flags.
  cfg.pan = v.pressed_mouse_move ?? cfg.pan;
  cfg.pan_horz_touch = v.horz_touch_drag ?? cfg.pan_horz_touch;
  cfg.pan_vert_touch = v.vert_touch_drag ?? cfg.pan_vert_touch;
  cfg.wheel_scroll = v.mouse_wheel ?? cfg.wheel_scroll;
}

function apply_scale(v: boolean | handle_scale_options, cfg: resolved_gestures): void {
  if (typeof v === "boolean") {
    // LWC migrateHandleScaleScrollOptions: a boolean expands to every scale flag.
    cfg.wheel_zoom = v;
    cfg.pinch_zoom = v;
    cfg.axis_dblclick_reset_time = v;
    cfg.axis_dblclick_reset_price = v;
    cfg.axis_scale_price = v;
    cfg.axis_scale_time = v;
    return;
  }
  // Object form merges over the current config (LWC applyOptions semantics).
  cfg.wheel_zoom = v.mouse_wheel ?? cfg.wheel_zoom;
  cfg.pinch_zoom = v.pinch ?? cfg.pinch_zoom;
  const adr = v.axis_double_click_reset;
  if (typeof adr === "boolean") {
    // LWC migrateHandleScaleScrollOptions: a boolean expands to both axes.
    cfg.axis_dblclick_reset_time = adr;
    cfg.axis_dblclick_reset_price = adr;
  } else if (adr) {
    cfg.axis_dblclick_reset_time = adr.time ?? cfg.axis_dblclick_reset_time;
    cfg.axis_dblclick_reset_price = adr.price ?? cfg.axis_dblclick_reset_price;
  }
  const apm = v.axis_pressed_mouse_move;
  if (typeof apm === "boolean") {
    cfg.axis_scale_price = apm;
    cfg.axis_scale_time = apm;
  } else if (apm) {
    cfg.axis_scale_price = apm.price ?? cfg.axis_scale_price;
    cfg.axis_scale_time = apm.time ?? cfg.axis_scale_time;
  }
}

function apply_kinetic(v: boolean | kinetic_scroll_options, cfg: resolved_gestures): void {
  if (typeof v === "boolean") {
    cfg.kinetic_touch = v;
    cfg.kinetic_mouse = v;
    return;
  }
  cfg.kinetic_touch = v.touch ?? cfg.kinetic_touch;
  cfg.kinetic_mouse = v.mouse ?? cfg.kinetic_mouse;
}

function apply_tracking(v: tracking_mode_options, cfg: resolved_gestures): void {
  cfg.tracking_exit_mode = v.exit_mode ?? cfg.tracking_exit_mode;
}

export class chart_impl implements chart_api {
  private next_extra_series = false;
  private readonly gestures_cfg: resolved_gestures = {
    pan: true,
    pan_horz_touch: true,
    pan_vert_touch: true,
    wheel_scroll: true,
    wheel_zoom: true,
    pinch_zoom: true,
    axis_dblclick_reset_time: true,
    axis_dblclick_reset_price: true,
    axis_scale_price: true,
    axis_scale_time: true,
    kinetic_touch: true,
    kinetic_mouse: false,
    panes_resize: true,
    tracking_exit_mode: "on_next_tap",
  };
  private a11y_live: HTMLElement | null = null;
  private readonly ts = new time_scale_impl(this);
  private observer: ResizeObserver | null = null;
  private detach_gestures: (() => void) | null = null;
  private removed = false;
  private readonly series_by_id = new Map<number, series_impl>();
  private readonly crosshair_subs = new Set<mouse_event_handler>();
  private readonly click_subs = new Set<mouse_event_handler>();
  private readonly dbl_click_subs = new Set<dbl_click_handler>();
  private readonly visible_logical_range_subs = new Set<visible_logical_range_handler>();
  private readonly visible_time_range_subs = new Set<visible_time_range_handler>();
  private last_visible_logical_range: logical_range | null;
  private last_visible_time_range: time_range | null;
  private readonly backend_runtime_id: number;
  private anim_frame: number | null = null;
  private readonly backend_loss_handler = (event: Event): void => {
    if ((event as CustomEvent<number>).detail !== this.backend_runtime_id || this.removed) return;
    // The wgpu callback may arrive from a promise microtask. Defer the repaint once more so the
    // callback stack is fully unwound before Rust drops GPU resources and paints the warm 2D pane.
    queueMicrotask(() => this.repaint());
  };

  constructor(
    readonly wasm: AionChart,
    private readonly container: HTMLElement,
    private readonly gpu_pane: HTMLCanvasElement,
    private readonly fallback_pane: HTMLCanvasElement,
    private readonly overlay: HTMLCanvasElement,
    auto_size: boolean,
  ) {
    this.backend_runtime_id = this.wasm.backend_runtime_id();
    window.addEventListener("aion-chart-backend-lost", this.backend_loss_handler);
    this.last_visible_logical_range = this.read_visible_logical_range();
    this.last_visible_time_range = this.read_visible_time_range();
    this.init_accessibility();
    this.detach_gestures = install_gestures(this);
    if (auto_size) {
      this.wasm.enable_auto_resize(container);
    }
  }

  /** Repaint unless torn down. Named distinctly from the public `render` for internal use. */
  repaint(): void {
    if (!this.removed) {
      this.wasm.render();
      this.emit_visible_range_changes();
    }
  }

  private read_visible_logical_range(): logical_range | null {
    const r = this.wasm.visible_logical_range();
    return r.length === 2 ? { from: r[0]!, to: r[1]! } : null;
  }

  private read_visible_time_range(): time_range | null {
    const r = this.wasm.visible_time_range();
    return r.length === 2 ? { from: r[0]!, to: r[1]! } : null;
  }

  private emit_visible_range_changes(): void {
    const logical = this.read_visible_logical_range();
    const time = this.read_visible_time_range();
    const logical_changed = !same_logical_range(this.last_visible_logical_range, logical);
    const time_changed = !same_time_range(this.last_visible_time_range, time);
    this.last_visible_logical_range = logical;
    this.last_visible_time_range = time;

    if (logical_changed) {
      for (const handler of this.visible_logical_range_subs) {
        handler(logical ? { ...logical } : null);
      }
    }
    if (time_changed) {
      for (const handler of this.visible_time_range_subs) {
        handler(time ? { ...time } : null);
      }
    }
  }

  /** Start or stop the animation rAF loop to match whether any series wants the last-price pulse. */
  sync_animation(): void {
    if (this.removed) return;
    if (this.wasm.wants_animation()) {
      this.start_animation();
    } else {
      this.stop_animation();
    }
  }
  private start_animation(): void {
    if (this.anim_frame !== null || this.removed) return;
    const tick = () => {
      if (this.removed || !this.wasm.wants_animation()) {
        this.anim_frame = null;
        return;
      }
      this.wasm.set_animation_time(performance.now());
      this.wasm.render();
      this.anim_frame = requestAnimationFrame(tick);
    };
    this.anim_frame = requestAnimationFrame(tick);
  }
  private stop_animation(): void {
    if (this.anim_frame !== null) {
      cancelAnimationFrame(this.anim_frame);
      this.anim_frame = null;
    }
  }

  add_series(kind: series_kind, options?: Partial<series_options>): series_api {
    // Series 0 is created by the engine at construction; the first add_series adopts it so the
    // common "one chart, one series" path matches LWC (add_series returns the primary series).
    let id: number;
    if (!this.next_extra_series) {
      this.next_extra_series = true;
      this.wasm.set_series_type(KIND_TO_U8[kind]);
      id = 0;
    } else {
      id = this.wasm.add_series(KIND_TO_U8[kind]);
    }
    const series = new series_impl(id, kind, this);
    this.series_by_id.set(id, series);
    if (options) {
      series.apply_options(options);
    }
    return series;
  }

  remove_series(series: series_api): void {
    const impl = this.series_by_id.get(series.id);
    // Ignore a foreign handle or one already removed (idempotent, matching LWC leniency).
    if (!impl) return;
    if (series.id === 0) {
      throw new Error("aion: the primary series cannot be removed");
    }
    if (!this.wasm.remove_series(series.id)) return;
    impl.mark_removed();
    this.series_by_id.delete(series.id);
    this.repaint();
  }

  private indicator_series(id: number, options?: Partial<series_options>): series_api {
    if (id === 0xffffffff) throw new Error("aion: invalid indicator configuration");
    const series = new series_impl(id, "line", this);
    this.series_by_id.set(id, series);
    if (options) series.apply_options(options);
    return series;
  }

  add_sma(source: series_api, period: number, options?: Partial<series_options>): series_api {
    return this.indicator_series(this.wasm.add_sma(source.id, Math.max(1, Math.floor(period))), options);
  }

  add_ema(source: series_api, period: number, options?: Partial<series_options>): series_api {
    return this.indicator_series(this.wasm.add_ema(source.id, Math.max(1, Math.floor(period))), options);
  }

  add_bollinger(source: series_api, period: number, deviation = 2, options?: Partial<series_options>): [series_api, series_api, series_api] {
    const ids = this.wasm.add_bollinger(source.id, Math.max(1, Math.floor(period)), deviation);
    if (ids.length !== 3) throw new Error("aion: invalid Bollinger configuration");
    return [this.indicator_series(ids[0]!, options), this.indicator_series(ids[1]!, options), this.indicator_series(ids[2]!, options)];
  }

  subscribe_crosshair_move(handler: mouse_event_handler): void {
    this.crosshair_subs.add(handler);
  }
  unsubscribe_crosshair_move(handler: mouse_event_handler): void {
    this.crosshair_subs.delete(handler);
  }
  subscribe_click(handler: mouse_event_handler): void {
    this.click_subs.add(handler);
  }
  unsubscribe_click(handler: mouse_event_handler): void {
    this.click_subs.delete(handler);
  }
  subscribe_dbl_click(handler: dbl_click_handler): void {
    this.dbl_click_subs.add(handler);
  }
  unsubscribe_dbl_click(handler: dbl_click_handler): void {
    this.dbl_click_subs.delete(handler);
  }
  subscribe_visible_logical_range_change(handler: visible_logical_range_handler): void {
    this.visible_logical_range_subs.add(handler);
  }
  unsubscribe_visible_logical_range_change(handler: visible_logical_range_handler): void {
    this.visible_logical_range_subs.delete(handler);
  }
  subscribe_visible_time_range_change(handler: visible_time_range_handler): void {
    this.visible_time_range_subs.add(handler);
  }
  unsubscribe_visible_time_range_change(handler: visible_time_range_handler): void {
    this.visible_time_range_subs.delete(handler);
  }

  /** Build event params for a cursor at (x, y) in pane CSS px. */
  private build_params(x: number, y: number): mouse_event_params {
    const time = undef_to_null(this.wasm.coordinate_to_time(x));
    const logical = undef_to_null(this.wasm.coordinate_to_logical(x));
    const flat = this.wasm.hover_data(x); // groups of [id, o, h, l, c]
    const series_data = new Map<series_api, ohlc_data | single_value_data>();
    const t = time ?? 0;
    for (let i = 0; i + 4 < flat.length; i += 5) {
      const s = this.series_by_id.get(flat[i]!);
      if (!s) continue;
      const [o, h, l, c] = [flat[i + 1]!, flat[i + 2]!, flat[i + 3]!, flat[i + 4]!];
      series_data.set(
        s,
        s.kind === "candlestick" || s.kind === "bar"
          ? { time: t, open: o, high: h, low: l, close: c }
          : { time: t, value: c },
      );
    }
    return { time, logical, point: { x, y }, series_data };
  }

  /** Emit a crosshair-move event (called by the gesture recognizer). */
  emit_crosshair(x: number, y: number): void {
    if (this.crosshair_subs.size === 0) return;
    const params = this.build_params(x, y);
    for (const h of this.crosshair_subs) h(params);
  }
  /** Emit the "cursor left the chart" crosshair event (empty params). */
  emit_crosshair_left(): void {
    if (this.crosshair_subs.size === 0) return;
    const params: mouse_event_params = { time: null, logical: null, point: null, series_data: new Map() };
    for (const h of this.crosshair_subs) h(params);
  }
  /** Emit a click event (called by the gesture recognizer). */
  emit_click(x: number, y: number): void {
    // A click/tap is a discrete, intentional action, so it is a good moment to announce the point
    // to assistive tech (unlike mouse hover, which would flood the live region).
    this.announce(x, y);
    if (this.click_subs.size === 0) return;
    const params = this.build_params(x, y);
    for (const h of this.click_subs) h(params);
  }

  /** Announce the current visible time range to assistive tech (used after keyboard navigation). */
  announce_view(): void {
    if (!this.a11y_live) return;
    const r = this.wasm.visible_time_range();
    this.a11y_live.textContent =
      r.length === 2 ? `Showing time ${r[0]} to ${r[1]}` : "No data";
  }

  emit_dbl_click(x: number, y: number): void {
    if (this.dbl_click_subs.size === 0) return;
    const params = this.build_params(x, y);
    for (const h of this.dbl_click_subs) h(params);
  }

  apply_options(options: deep_partial<chart_options>): void {
    // handle_scroll / handle_scale / kinetic_scroll / tracking_mode (gestures), the pane-resize
    // toggle, and localization (JS callbacks) are package-level; intercept and strip them so only
    // engine-owned, JSON-serializable options reach the wasm store.
    const { handle_scroll, handle_scale, kinetic_scroll, tracking_mode, localization, ...rest } =
      options as deep_partial<chart_options> & {
        handle_scroll?: boolean | handle_scroll_options;
        handle_scale?: boolean | handle_scale_options;
        kinetic_scroll?: boolean | kinetic_scroll_options;
        tracking_mode?: tracking_mode_options;
        localization?: localization_options;
      };
    let engine_options: Record<string, unknown> = rest;
    // layout.panes.enableResize (LWC) drives the separator drag here, not the engine; strip it
    // alongside the other gesture keys before forwarding.
    const panes = (rest.layout as { panes?: { enableResize?: boolean } } | undefined)?.panes;
    if (panes?.enableResize !== undefined) {
      const { enableResize, ...panes_rest } = panes;
      engine_options = { ...rest, layout: { ...(rest.layout as object), panes: panes_rest } };
      this.gestures_cfg.panes_resize = enableResize;
    }
    if (
      handle_scroll !== undefined || handle_scale !== undefined || kinetic_scroll !== undefined ||
      tracking_mode !== undefined
    ) {
      this.apply_gesture_options(handle_scroll, handle_scale, kinetic_scroll, tracking_mode);
    }
    if (localization !== undefined) this.apply_localization(localization);
    this.wasm.apply_options(JSON.stringify(engine_options));
    this.repaint();
  }

  /** Install the host price/time formatters (LWC `localization`). Callbacks cross into wasm. */
  apply_localization(loc: localization_options): void {
    if (loc.price_formatter !== undefined) this.wasm.set_price_formatter(loc.price_formatter);
    if (loc.time_formatter !== undefined) this.wasm.set_time_formatter(loc.time_formatter);
  }

  /** Resolve and store the gesture toggles; the recognizer reads the result live. */
  apply_gesture_options(
    scroll?: boolean | handle_scroll_options,
    scale?: boolean | handle_scale_options,
    kinetic?: boolean | kinetic_scroll_options,
    tracking?: tracking_mode_options,
  ): void {
    if (scroll !== undefined) apply_scroll(scroll, this.gestures_cfg);
    if (scale !== undefined) apply_scale(scale, this.gestures_cfg);
    if (kinetic !== undefined) apply_kinetic(kinetic, this.gestures_cfg);
    if (tracking !== undefined) apply_tracking(tracking, this.gestures_cfg);
    this.sync_interaction_disabled();
  }

  /** Resolve the LWC `layout.panes.enableResize` toggle (separator drag + hover cursor). */
  apply_panes_resize(enabled: boolean): void {
    this.gestures_cfg.panes_resize = enabled;
  }

  /** Mirror the engine's master interaction switch: off only when every scroll+scale flag is. */
  private sync_interaction_disabled(): void {
    const c = this.gestures_cfg;
    const all_off =
      !c.pan && !c.pan_horz_touch && !c.pan_vert_touch && !c.wheel_scroll && !c.wheel_zoom &&
      !c.pinch_zoom && !c.axis_dblclick_reset_time && !c.axis_dblclick_reset_price &&
      !c.axis_scale_time && !c.axis_scale_price;
    this.wasm.set_interaction_disabled(all_off);
  }

  /** Set up the accessible wrapper: focusable role on the overlay + an aria-live status region. */
  private init_accessibility(): void {
    this.container.setAttribute("role", "group");
    if (!this.container.hasAttribute("aria-label")) {
      this.container.setAttribute("aria-label", "Financial chart");
    }
    this.overlay.tabIndex = 0;
    this.overlay.setAttribute("role", "application");
    this.overlay.setAttribute(
      "aria-label",
      "Chart. Arrow keys pan, plus and minus zoom, Home fits content, Escape clears the crosshair.",
    );
    const live = this.container.ownerDocument.createElement("div");
    live.setAttribute("aria-live", "polite");
    live.setAttribute("role", "status");
    // Visually hidden but available to assistive tech.
    live.style.cssText =
      "position:absolute;width:1px;height:1px;margin:-1px;padding:0;overflow:hidden;clip:rect(0 0 0 0);white-space:nowrap;border:0;";
    this.container.appendChild(live);
    this.a11y_live = live;
  }

  /** Update the aria-live region with a compact description of the point under the cursor. */
  announce(x: number, y: number): void {
    if (!this.a11y_live) return;
    const params = this.build_params(x, y);
    if (params.time === null || params.series_data.size === 0) return;
    const parts = [];
    for (const [, point] of params.series_data) {
      parts.push("value" in point ? `${point.value}` : `O ${point.open} H ${point.high} L ${point.low} C ${point.close}`);
    }
    this.a11y_live.textContent = `Time ${params.time}: ${parts.join("; ")}`;
  }

  /** Whether the user has requested reduced motion (gates kinetic scroll). */
  prefers_reduced_motion(): boolean {
    return this.container.ownerDocument.defaultView?.matchMedia?.("(prefers-reduced-motion: reduce)")
      .matches === true;
  }

  /** Current resolved gesture toggles (read by the gesture recognizer). */
  gesture_config(): resolved_gestures {
    return this.gestures_cfg;
  }

  options(): unknown {
    return JSON.parse(this.wasm.options_json());
  }

  backend(): "webgpu" | "canvas2d" {
    return this.wasm.backend_kind() as "webgpu" | "canvas2d";
  }

  time_scale(): time_scale_api {
    return this.ts;
  }

  price_scale(price_scale_id: "left" | "right" | "" = "right", pane_index = 0): price_scale_api {
    const target = price_scale_id === "left" ? 1 : price_scale_id === "" ? 2 : 0;
    return new price_scale_impl(this, pane_index, target);
  }

  panes(): pane_api[] {
    const n = this.wasm.pane_count();
    const out: pane_api[] = [];
    for (let i = 0; i < n; i++) {
      out.push(new pane_impl(this, i));
    }
    return out;
  }

  price_to_coordinate(price: number): number | null {
    return undef_to_null(this.wasm.price_to_coordinate(price));
  }
  coordinate_to_price(y: number): number | null {
    return undef_to_null(this.wasm.coordinate_to_price(y));
  }

  resize(width: number, height: number, dpr?: number): void {
    this.wasm.resize(width, height, dpr ?? window.devicePixelRatio ?? 1);
    this.repaint();
  }

  render(): void {
    this.repaint();
  }

  take_screenshot(): HTMLCanvasElement {
    // Browser WebGPU canvases are presentable but are not synchronously readable through
    // CanvasRenderingContext2D.drawImage (Chromium returns transparent pixels). Repaint the current
    // engine frame, then execute that same retained frame through the already-warm Canvas2D pane.
    // This keeps the LWC-style synchronous API deterministic without duplicating chart state.
    this.repaint();
    this.wasm.render_canvas2d_snapshot();
    const output = document.createElement("canvas");
    output.width = this.overlay.width;
    output.height = this.overlay.height;
    const ctx = output.getContext("2d");
    if (ctx === null) throw new Error("aion: screenshot Canvas2D context is unavailable");
    ctx.drawImage(this.fallback_pane, 0, 0);
    ctx.drawImage(this.overlay, 0, 0);
    return output;
  }

  overlay_el(): HTMLCanvasElement {
    return this.overlay;
  }

  remove(): void {
    if (this.removed) return;
    this.removed = true;
    this.stop_animation();
    window.removeEventListener("aion-chart-backend-lost", this.backend_loss_handler);
    this.detach_gestures?.();
    this.observer?.disconnect();
    this.gpu_pane.remove();
    this.fallback_pane.remove();
    this.overlay.remove();
  }
}

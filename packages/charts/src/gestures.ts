/**
 * Pointer/wheel/keyboard gesture recognizer wired onto the axis/input overlay canvas.
 *
 * Beyond pan/zoom/crosshair it implements LWC-parity interactions:
 * - axis drag-to-scale (price axis = vertical, time axis = horizontal),
 * - kinetic (momentum) scroll after a flick,
 * - touch: drag pans without a crosshair, long-press enters a crosshair "tracking" mode,
 * - keyboard: arrows pan, +/- zoom, Home fit-content, Escape clear crosshair.
 * All behavior is gated by the resolved gesture config (`chart.gesture_config()`).
 */

import type { chart_impl } from "./impl.js";

const TAP_SLOP = 4; // css px before a press becomes a drag
const SEP_HIT = 4; // css px hit tolerance around a pane boundary
const LONGPRESS_MS = 250; // touch hold before entering crosshair tracking
const PRICE_SCALE_K = 0.01; // price-axis drag sensitivity (per css px)
const TIME_SCALE_K = 0.006; // time-axis drag sensitivity (per css px)
const KINETIC_MIN_V = 0.05; // px/ms flick velocity needed to coast
const KINETIC_STOP_V = 0.01; // px/ms at which coasting stops
const KINETIC_TAU = 325; // ms momentum time constant

type AxisDrag =
  | { kind: "price"; pane: number; target: number; last_y: number }
  | { kind: "time"; last_x: number };

export function install_gestures(chart: chart_impl): () => void {
  const overlay = chart.overlay_el();
  const wasm = chart.wasm;
  const pointers = new Map<number, { x: number; y: number }>();
  let dragging = false;
  let pinch_dist = 0;
  let sep_drag: { index: number; last_y: number } | null = null;
  let axis_drag: AxisDrag | null = null;
  let press_origin: { x: number; y: number } | null = null;
  let moved = false;
  let primary_pointer_type = "mouse";
  let touch_tracking = false;
  let longpress_timer: ReturnType<typeof setTimeout> | null = null;
  let last_pan_x: number | null = null;
  let vel_samples: { x: number; t: number }[] = [];
  let kinetic_raf: number | null = null;

  const local_xy = (e: { clientX: number; clientY: number }) => {
    const r = overlay.getBoundingClientRect();
    return { x: e.clientX - r.left - wasm.pane_left(), y: e.clientY - r.top };
  };

  const separator_at = (y: number): number => {
    const ys = wasm.pane_separator_ys();
    for (let i = 0; i < ys.length; i++) {
      if (Math.abs(y - ys[i]!) <= SEP_HIT) return i;
    }
    return -1;
  };

  /** Price-axis target under `p` (0 = right, 1 = left), or null if not over a price axis. */
  const price_axis_target_at = (p: { x: number; y: number }): number | null => {
    if (p.x < 0) return 1;
    if (p.x > wasm.time_scale_width()) return 0;
    return null;
  };
  const is_time_axis = (p: { x: number; y: number }): boolean =>
    p.y > overlay.getBoundingClientRect().height - wasm.time_scale_height();
  const pane_of = (y: number): number => {
    let pane = 0;
    for (const sy of wasm.pane_separator_ys()) if (y > sy) pane += 1;
    return pane;
  };

  const clear_longpress = () => {
    if (longpress_timer !== null) {
      clearTimeout(longpress_timer);
      longpress_timer = null;
    }
  };
  const record_velocity = (x: number) => {
    vel_samples.push({ x, t: performance.now() });
    if (vel_samples.length > 6) vel_samples.shift();
  };
  const current_velocity = (): number => {
    if (vel_samples.length < 2) return 0;
    const a = vel_samples[0]!;
    const b = vel_samples[vel_samples.length - 1]!;
    const dt = b.t - a.t;
    return dt > 0 ? (b.x - a.x) / dt : 0;
  };
  const stop_kinetic = () => {
    if (kinetic_raf !== null) {
      cancelAnimationFrame(kinetic_raf);
      kinetic_raf = null;
      wasm.scroll_end();
    }
  };
  const start_kinetic = (x0: number, v0: number) => {
    let x = x0;
    let v = v0;
    let last = performance.now();
    const step = () => {
      const now = performance.now();
      const dt = now - last;
      last = now;
      x += v * dt;
      v *= Math.exp(-dt / KINETIC_TAU);
      wasm.scroll_move(x);
      chart.repaint();
      if (Math.abs(v) < KINETIC_STOP_V) {
        wasm.scroll_end();
        kinetic_raf = null;
        return;
      }
      kinetic_raf = requestAnimationFrame(step);
    };
    kinetic_raf = requestAnimationFrame(step);
  };

  const on_wheel = (e: WheelEvent) => {
    if (!chart.gesture_config().wheel_zoom) return; // let the page scroll
    e.preventDefault();
    const delta_y = -(e.deltaY / 100);
    if (delta_y !== 0) {
      const zoom_scale = Math.sign(delta_y) * Math.min(1, Math.abs(delta_y));
      wasm.zoom(e.offsetX - wasm.pane_left(), zoom_scale);
      chart.repaint();
    }
  };

  const on_down = (e: PointerEvent) => {
    stop_kinetic();
    try {
      overlay.setPointerCapture(e.pointerId);
    } catch {
      // ignore synthetic events with no active pointer
    }
    const p = local_xy(e);
    pointers.set(e.pointerId, p);
    if (pointers.size === 1) {
      press_origin = p;
      moved = false;
      primary_pointer_type = e.pointerType || "mouse";
      touch_tracking = false;
      vel_samples = [];
      last_pan_x = null;
      const cfg = chart.gesture_config();

      // separator drag takes precedence over any pan/scale
      const si = separator_at(p.y);
      if (si >= 0) {
        sep_drag = { index: si, last_y: p.y };
        return;
      }
      // axis drag-to-scale: price axis (vertical) / time axis (horizontal)
      const price_target = price_axis_target_at(p);
      if (price_target !== null) {
        if (cfg.axis_scale_price) {
          axis_drag = { kind: "price", pane: pane_of(p.y), target: price_target, last_y: p.y };
        }
        return; // never pan from an axis strip
      }
      if (is_time_axis(p)) {
        if (cfg.axis_scale_time) axis_drag = { kind: "time", last_x: p.x };
        return;
      }
      // pane press: pan (mouse and touch); touch also arms long-press → crosshair tracking
      if (cfg.pan) {
        dragging = true;
        wasm.scroll_start(p.x);
        last_pan_x = p.x;
      }
      if (primary_pointer_type === "touch") {
        longpress_timer = setTimeout(() => {
          longpress_timer = null;
          if (moved || press_origin === null) return;
          touch_tracking = true;
          if (dragging) {
            dragging = false;
            wasm.scroll_end();
          }
          wasm.set_crosshair(press_origin.x, press_origin.y);
          chart.repaint();
          chart.emit_crosshair(press_origin.x, press_origin.y);
        }, LONGPRESS_MS);
      }
    } else if (pointers.size === 2) {
      clear_longpress();
      touch_tracking = false;
      if (dragging) {
        dragging = false;
        wasm.scroll_end();
      }
      const [a, b] = [...pointers.values()];
      pinch_dist = Math.hypot(a!.x - b!.x, a!.y - b!.y);
    }
  };

  const on_move = (e: PointerEvent) => {
    const p = local_xy(e);

    // active axis drag-to-scale
    if (axis_drag !== null) {
      if (axis_drag.kind === "price") {
        const dy = p.y - axis_drag.last_y;
        axis_drag.last_y = p.y;
        const range = wasm.price_scale_visible_range(axis_drag.pane, axis_drag.target);
        if (range.length === 2) {
          const mid = (range[0]! + range[1]!) / 2;
          const half = ((range[1]! - range[0]!) / 2) * Math.exp(dy * PRICE_SCALE_K);
          wasm.set_price_scale_visible_range(axis_drag.pane, axis_drag.target, mid - half, mid + half);
        }
      } else {
        const dx = p.x - axis_drag.last_x;
        axis_drag.last_x = p.x;
        wasm.set_bar_spacing(wasm.bar_spacing() * Math.exp(dx * TIME_SCALE_K));
      }
      chart.repaint();
      return;
    }

    if (sep_drag !== null) {
      const dy = p.y - sep_drag.last_y;
      sep_drag.last_y = p.y;
      wasm.drag_pane_separator(sep_drag.index, dy);
      chart.repaint();
      return;
    }

    // hover cursor feedback (no button pressed)
    if (pointers.size === 0) {
      overlay.style.cursor =
        separator_at(p.y) >= 0
          ? "row-resize"
          : price_axis_target_at(p) !== null
            ? "ns-resize"
            : is_time_axis(p)
              ? "ew-resize"
              : "crosshair";
    }

    if (pointers.has(e.pointerId)) pointers.set(e.pointerId, p);
    if (pointers.size > 0 && press_origin !== null && !moved) {
      moved = Math.hypot(p.x - press_origin.x, p.y - press_origin.y) > TAP_SLOP;
      if (moved) clear_longpress(); // a moving press is a drag, not a hold
    }

    if (pointers.size >= 2) {
      const [a, b] = [...pointers.values()];
      const dist = Math.hypot(a!.x - b!.x, a!.y - b!.y);
      const mid = (a!.x + b!.x) / 2;
      if (pinch_dist > 0 && dist > 0 && chart.gesture_config().pinch_zoom) {
        const zoom_scale = Math.max(-1, Math.min(1, (dist - pinch_dist) / 40));
        if (zoom_scale !== 0) wasm.zoom(mid, zoom_scale);
      }
      pinch_dist = dist;
      chart.repaint();
      return;
    }

    if (dragging) {
      wasm.scroll_move(p.x);
      last_pan_x = p.x;
      record_velocity(p.x);
    }
    // Crosshair: mouse always (hover + drag); touch only in tracking mode (a plain touch drag pans).
    if (primary_pointer_type !== "touch" || touch_tracking) {
      wasm.set_crosshair(p.x, p.y);
      chart.emit_crosshair(p.x, p.y);
    }
    chart.repaint();
  };

  const end_pointer = (e: PointerEvent) => {
    clear_longpress();
    pointers.delete(e.pointerId);
    if (pointers.size === 0 && sep_drag !== null) {
      sep_drag = null;
      return;
    }
    if (pointers.size === 0 && axis_drag !== null) {
      axis_drag = null;
      return;
    }
    if (pointers.size < 2) pinch_dist = 0;
    if (pointers.size !== 0) return;

    const was_dragging = dragging;
    dragging = false;
    touch_tracking = false;

    let kinetic = false;
    if (was_dragging) {
      const cfg = chart.gesture_config();
      const enabled =
        (primary_pointer_type === "touch" ? cfg.kinetic_touch : cfg.kinetic_mouse) &&
        !chart.prefers_reduced_motion();
      const v = current_velocity();
      if (enabled && last_pan_x !== null && Math.abs(v) > KINETIC_MIN_V) {
        start_kinetic(last_pan_x, v);
        kinetic = true;
      } else {
        wasm.scroll_end();
      }
    }
    wasm.clear_crosshair();
    if (!kinetic) chart.repaint();
    chart.emit_crosshair_left();
  };

  const on_dblclick = (e: MouseEvent) => {
    if (moved) return;
    const p = local_xy(e);
    chart.emit_dbl_click(p.x, p.y);
    if (!chart.gesture_config().axis_dblclick_reset) return;
    const rect = overlay.getBoundingClientRect();
    const on_time_axis = p.y > rect.height - wasm.time_scale_height();
    const on_price_axis = p.x < 0 || p.x > wasm.time_scale_width();
    if (on_time_axis) {
      wasm.reset_time_scale();
      chart.repaint();
    } else if (on_price_axis) {
      wasm.set_price_scale_auto_scale(pane_of(p.y), p.x < 0 ? 1 : 0, true);
      chart.repaint();
    }
  };

  const on_click = (e: MouseEvent) => {
    if (moved) return;
    const p = local_xy(e);
    chart.emit_click(p.x, p.y);
  };

  const on_leave = (e: PointerEvent) => {
    if (e.pointerType === "mouse") end_pointer(e);
  };

  const on_keydown = (e: KeyboardEvent) => {
    const cfg = chart.gesture_config();
    const step = e.shiftKey ? 10 : 1;
    const center = wasm.time_scale_width() / 2;
    let handled = true;
    switch (e.key) {
      case "ArrowLeft":
        wasm.scroll_to_position(wasm.scroll_position() + step);
        break;
      case "ArrowRight":
        wasm.scroll_to_position(wasm.scroll_position() - step);
        break;
      case "+":
      case "=":
        if (cfg.wheel_zoom) wasm.zoom(center, 0.5);
        break;
      case "-":
      case "_":
        if (cfg.wheel_zoom) wasm.zoom(center, -0.5);
        break;
      case "Home":
        wasm.fit_content();
        break;
      case "Escape":
        wasm.clear_crosshair();
        chart.repaint();
        chart.emit_crosshair_left();
        return;
      default:
        handled = false;
    }
    if (handled) {
      e.preventDefault();
      stop_kinetic();
      chart.repaint();
      chart.announce_view();
    }
  };

  overlay.addEventListener("wheel", on_wheel, { passive: false });
  overlay.addEventListener("pointerdown", on_down);
  overlay.addEventListener("pointermove", on_move);
  overlay.addEventListener("pointerup", end_pointer);
  overlay.addEventListener("pointercancel", end_pointer);
  overlay.addEventListener("pointerleave", on_leave);
  overlay.addEventListener("dblclick", on_dblclick);
  overlay.addEventListener("click", on_click);
  overlay.addEventListener("keydown", on_keydown);

  return () => {
    stop_kinetic();
    clear_longpress();
    overlay.removeEventListener("wheel", on_wheel);
    overlay.removeEventListener("pointerdown", on_down);
    overlay.removeEventListener("pointermove", on_move);
    overlay.removeEventListener("pointerup", end_pointer);
    overlay.removeEventListener("pointercancel", end_pointer);
    overlay.removeEventListener("pointerleave", on_leave);
    overlay.removeEventListener("dblclick", on_dblclick);
    overlay.removeEventListener("click", on_click);
    overlay.removeEventListener("keydown", on_keydown);
  };
}

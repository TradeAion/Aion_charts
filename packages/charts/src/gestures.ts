/**
 * Pointer/wheel gesture recognizer wired onto the axis/input overlay canvas.
 * Extracted from `index.ts`.
 */

import type { chart_impl } from "./impl.js";

export function install_gestures(chart: chart_impl): () => void {
  const overlay = chart.overlay_el();
  const wasm = chart.wasm;
  const pointers = new Map<number, { x: number; y: number }>();
  let dragging = false;
  let pinch_dist = 0;
  // separator drag state (roadmap Phase B1): index of the separator being dragged + last Y
  let sep_drag: { index: number; last_y: number } | null = null;
  const SEP_HIT = 4; // css px hit tolerance around a pane boundary

  const local_xy = (e: { clientX: number; clientY: number }) => {
    const r = overlay.getBoundingClientRect();
    return { x: e.clientX - r.left - wasm.pane_left(), y: e.clientY - r.top };
  };

  /** Index of the separator within SEP_HIT px of css-y `y`, or -1. */
  const separator_at = (y: number): number => {
    const ys = wasm.pane_separator_ys();
    for (let i = 0; i < ys.length; i++) {
      if (Math.abs(y - ys[i]!) <= SEP_HIT) return i;
    }
    return -1;
  };

  const on_wheel = (e: WheelEvent) => {
    e.preventDefault();
    const delta_y = -(e.deltaY / 100);
    if (delta_y !== 0) {
      const zoom_scale = Math.sign(delta_y) * Math.min(1, Math.abs(delta_y));
      wasm.zoom(e.offsetX - wasm.pane_left(), zoom_scale);
      chart.repaint();
    }
  };
  const on_down = (e: PointerEvent) => {
    try {
      overlay.setPointerCapture(e.pointerId);
    } catch {
      // ignore (e.g. synthetic events with no active pointer)
    }
    const p = local_xy(e);
    pointers.set(e.pointerId, p);
    // start a separator drag instead of a pan when pressing on a pane boundary
    if (pointers.size === 1) {
      const si = separator_at(p.y);
      if (si >= 0) {
        sep_drag = { index: si, last_y: p.y };
        return;
      }
    }
    if (pointers.size === 1) {
      dragging = true;
      wasm.scroll_start(p.x);
    } else if (pointers.size === 2) {
      dragging = false;
      wasm.scroll_end();
      const [a, b] = [...pointers.values()];
      pinch_dist = Math.hypot(a!.x - b!.x, a!.y - b!.y);
    }
  };
  const on_move = (e: PointerEvent) => {
    const p = local_xy(e);
    // active separator drag: resize the two adjacent panes
    if (sep_drag !== null) {
      const dy = p.y - sep_drag.last_y;
      sep_drag.last_y = p.y;
      wasm.drag_pane_separator(sep_drag.index, dy);
      chart.repaint();
      return;
    }
    // hover cursor feedback over a separator (no button pressed)
    if (pointers.size === 0) {
      overlay.style.cursor = separator_at(p.y) >= 0 ? "row-resize" : "crosshair";
    }
    if (pointers.has(e.pointerId)) pointers.set(e.pointerId, p);
    if (pointers.size >= 2) {
      const [a, b] = [...pointers.values()];
      const dist = Math.hypot(a!.x - b!.x, a!.y - b!.y);
      const mid = (a!.x + b!.x) / 2;
      if (pinch_dist > 0 && dist > 0) {
        const zoom_scale = Math.max(-1, Math.min(1, (dist - pinch_dist) / 40));
        if (zoom_scale !== 0) wasm.zoom(mid, zoom_scale);
      }
      pinch_dist = dist;
      chart.repaint();
      return;
    }
    if (dragging) wasm.scroll_move(p.x);
    wasm.set_crosshair(p.x, p.y);
    chart.repaint();
    chart.emit_crosshair(p.x, p.y);
  };
  const end_pointer = (e: PointerEvent) => {
    pointers.delete(e.pointerId);
    if (sep_drag !== null && pointers.size === 0) {
      sep_drag = null;
      return;
    }
    if (pointers.size < 2) pinch_dist = 0;
    if (pointers.size === 0) {
      if (dragging) {
        dragging = false;
        wasm.scroll_end();
      }
      wasm.clear_crosshair();
      chart.repaint();
      chart.emit_crosshair_left();
    }
  };
  const on_dblclick = (e: MouseEvent) => {
    const p = local_xy(e);
    chart.emit_dbl_click(p.x, p.y);
    wasm.fit_content();
    chart.repaint();
  };
  const on_click = (e: MouseEvent) => {
    const p = local_xy(e);
    chart.emit_click(p.x, p.y);
  };
  const on_leave = (e: PointerEvent) => {
    if (e.pointerType === "mouse") end_pointer(e);
  };

  overlay.addEventListener("wheel", on_wheel, { passive: false });
  overlay.addEventListener("pointerdown", on_down);
  overlay.addEventListener("pointermove", on_move);
  overlay.addEventListener("pointerup", end_pointer);
  overlay.addEventListener("pointercancel", end_pointer);
  overlay.addEventListener("pointerleave", on_leave);
  overlay.addEventListener("dblclick", on_dblclick);
  overlay.addEventListener("click", on_click);

  return () => {
    overlay.removeEventListener("wheel", on_wheel);
    overlay.removeEventListener("pointerdown", on_down);
    overlay.removeEventListener("pointermove", on_move);
    overlay.removeEventListener("pointerup", end_pointer);
    overlay.removeEventListener("pointercancel", end_pointer);
    overlay.removeEventListener("pointerleave", on_leave);
    overlay.removeEventListener("dblclick", on_dblclick);
    overlay.removeEventListener("click", on_click);
  };
}

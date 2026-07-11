/**
 * @aion/charts — public API (snake_case; semantics mirror lightweight-charts v5).
 *
 * This package is a thin shell over the aion_wasm module: it packs data into typed arrays,
 * forwards gestures, and materializes event params lazily. Wired up in Phase 0/1.
 */

export interface candlestick_data {
  /** UTC timestamp in seconds (or business-day string; converted at the boundary). */
  time: number | string;
  open: number;
  high: number;
  low: number;
  close: number;
}

export interface chart_handle {
  remove(): void;
}

/** Placeholder until the wasm module lands (Phase 0 surface bring-up). */
export function create_chart(_container: HTMLElement, _options?: unknown): chart_handle {
  throw new Error("aion_wasm module not built yet — see docs/ARCHITECTURE.md phase 0");
}

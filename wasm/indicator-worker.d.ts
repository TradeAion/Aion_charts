export type IndicatorWorkerMethod =
  | 'init'
  | 'setContext'
  | 'compile'
  | 'attach'
  | 'setData'
  | 'upsertBar'
  | 'drawInstructions'
  | 'drainEvents'
  | 'reset';

export interface IndicatorWorkerRequest {
  id?: number | string | null;
  method?: IndicatorWorkerMethod;
  type?: IndicatorWorkerMethod;
  moduleOrPath?: string | URL | Request | WebAssembly.Module;
  symbol?: string;
  interval?: string;
  source?: string;
  metaJson?: string;
  indicatorId?: number;
  optsJson?: string;
  open?: Float64Array;
  high?: Float64Array;
  low?: Float64Array;
  close?: Float64Array;
  volume?: Float64Array;
  timestamps?: BigUint64Array;
  timestamp?: bigint | number | string;
}

export type IndicatorWorkerResponse =
  | { id: number | string | null; ok: true; result: unknown }
  | { id: number | string | null; ok: false; error: string };

import init, { IndicatorWorkerRuntime } from './pkg/aion_charts_wasm.js';

let runtime = null;
let initPromise = null;

function runtimeInstance() {
  if (!runtime) {
    runtime = new IndicatorWorkerRuntime();
  }
  return runtime;
}

async function ensureInit(moduleOrPath) {
  if (!initPromise) {
    initPromise = init({
      module_or_path: moduleOrPath || new URL('./pkg/aion_charts_wasm_bg.wasm', import.meta.url),
    });
  }
  await initPromise;
  return runtimeInstance();
}

function parseMethod(message) {
  return message?.method || message?.type;
}

async function handleMessage(message) {
  const method = parseMethod(message);
  const rt = await ensureInit(message?.moduleOrPath);

  switch (method) {
    case 'init':
      return true;
    case 'setContext':
      rt.set_context(message.symbol || 'AION', message.interval || '1m');
      return true;
    case 'compile':
      return rt.compile(message.source || '', message.metaJson || '{}');
    case 'attach':
      return rt.attach(message.indicatorId, message.optsJson || '{}');
    case 'setData':
      rt.set_data_arrays(
        message.open,
        message.high,
        message.low,
        message.close,
        message.volume,
        message.timestamps,
      );
      return true;
    case 'upsertBar':
      rt.upsert_bar(
        BigInt(message.timestamp),
        message.open,
        message.high,
        message.low,
        message.close,
        message.volume,
      );
      return true;
    case 'drawInstructions':
      return JSON.parse(rt.draw_instructions_json());
    case 'drainEvents':
      return JSON.parse(rt.drain_events_json());
    case 'reset':
      runtime?.free?.();
      runtime = new IndicatorWorkerRuntime();
      return true;
    default:
      throw new Error(`Unknown indicator worker method: ${String(method)}`);
  }
}

self.addEventListener('message', event => {
  const message = event.data || {};
  const id = message.id ?? null;

  handleMessage(message)
    .then(result => {
      self.postMessage({ id, ok: true, result });
    })
    .catch(error => {
      self.postMessage({
        id,
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      });
    });
});

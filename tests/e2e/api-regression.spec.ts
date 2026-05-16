import { expect, test } from '@playwright/test';
import { installHarness } from './aion-harness';

async function paneChecksum(page: any) {
  return page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>('#aion_charts-pane-chart');
    if (!canvas) throw new Error('pane canvas missing');
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('pane canvas context missing');
    const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let sum = 0;
    for (let i = 0; i < pixels.length; i += 97) {
      sum = (sum + pixels[i] * (i + 1)) % 1_000_000_007;
    }
    return sum;
  });
}

test('raw WASM streaming mutators schedule auto-render without manual render', async ({ page }) => {
  await installHarness(page);

  const before = await paneChecksum(page);
  await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    chart.start_auto_render();
    const last = bars.timestamps[bars.timestamps.length - 1];
    chart.upsert_bar(last, 60, 240, 55, 230, 999);
  });

  await expect.poll(async () => paneChecksum(page), { timeout: 2_000 }).not.toBe(before);
});

test('batch streaming APIs append and exported snapshots produce PNG data URLs', async ({ page }) => {
  await installHarness(page);

  const result = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    const start = bars.timestamps[bars.timestamps.length - 1] + 60_000n;
    const open = new Float64Array([118, 121, 123]);
    const high = new Float64Array([122, 124, 129]);
    const low = new Float64Array([116, 120, 121]);
    const close = new Float64Array([121, 123, 128]);
    const volume = new Float64Array([550, 560, 570]);
    const timestamps = new BigUint64Array([start, start + 60_000n, start + 120_000n]);

    chart.append_bars(open, high, low, close, volume, timestamps);
    chart.render();

    return {
      dataRange: chart.data_range(),
      paneUrl: chart.export_pane_image_data_url(),
      fullUrl: chart.export_image_data_url(),
    };
  });

  expect(result.dataRange[1]).toBe(Number(1_700_000_000_000n + BigInt((64 + 2) * 60_000)));
  expect(result.paneUrl.startsWith('data:image/png;base64,')).toBe(true);
  expect(result.fullUrl.startsWith('data:image/png;base64,')).toBe(true);
});

test('indicator worker offloads compile, data load, and incremental updates', async ({ page }) => {
  await installHarness(page);

  const result = await page.evaluate(async () => {
    const { bars } = (window as any).__aionHarness;
    const worker = new Worker('/wasm/indicator-worker.js', { type: 'module' });
    let id = 0;
    const call = (message: Record<string, any>) => new Promise<any>((resolve, reject) => {
      const requestId = ++id;
      const listener = (event: MessageEvent) => {
        if (event.data?.id !== requestId) return;
        worker.removeEventListener('message', listener);
        if (event.data.ok) {
          resolve(event.data.result);
        } else {
          reject(new Error(event.data.error));
        }
      };
      worker.addEventListener('message', listener);
      worker.postMessage({ id: requestId, ...message });
    });

    try {
      await call({ method: 'init' });
      await call({ method: 'setContext', symbol: 'BTCUSD', interval: '1m' });
      const compiled = await call({
        method: 'compile',
        source: 'indicator("worker-close")\nplot(close)',
        metaJson: '{}',
      });
      if (!compiled.indicatorId) return { compiled, instructions: [], events: [] };
      const instanceId = await call({
        method: 'attach',
        indicatorId: compiled.indicatorId,
        optsJson: '{}',
      });
      await call({
        method: 'setData',
        open: bars.open,
        high: bars.high,
        low: bars.low,
        close: bars.close,
        volume: bars.volume,
        timestamps: bars.timestamps,
      });
      await call({
        method: 'upsertBar',
        timestamp: bars.timestamps[bars.timestamps.length - 1] + 60_000n,
        open: 130,
        high: 135,
        low: 128,
        close: 134,
        volume: 900,
      });
      const instructions = await call({ method: 'drawInstructions' });
      const events = await call({ method: 'drainEvents' });
      return { compiled, instanceId, instructions, events };
    } finally {
      worker.terminate();
    }
  });

  expect(result.compiled.indicatorId).toBeGreaterThan(0);
  expect(result.instanceId).toBeGreaterThan(0);
  expect(result.instructions.length).toBeGreaterThan(0);
});

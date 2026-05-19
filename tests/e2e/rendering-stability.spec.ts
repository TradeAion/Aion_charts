import { expect, test } from '@playwright/test';

async function installResponsiveHarness(page: import('@playwright/test').Page) {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  await page.evaluate(async () => {
    if ((window as any).__responsiveHarness) return;

    const module = await import('/wasm/pkg/aion_charts_wasm.js');
    await module.default({ module_or_path: '/wasm/pkg/aion_charts_wasm_bg.wasm' });

    const host = document.createElement('div');
    host.id = 'responsive-render-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:100vw',
      'height:100vh',
      'background:#131315',
      'overflow:hidden',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);
    document.body.style.margin = '0';
    document.body.style.background = '#131315';

    const chart = await module.Aion_charts.create_chart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });

    const count = 72;
    const open = new Float64Array(count);
    const high = new Float64Array(count);
    const low = new Float64Array(count);
    const close = new Float64Array(count);
    const volume = new Float64Array(count);
    const timestamps = new BigUint64Array(count);
    const start = 1_700_000_000_000n;
    let price = 100;
    for (let i = 0; i < count; i += 1) {
      const body = (i % 2 === 0 ? 1 : -1) * (0.6 + (i % 4) * 0.08);
      open[i] = price;
      close[i] = price + body;
      high[i] = Math.max(open[i], close[i]) + 3.2 + (i % 5) * 0.18;
      low[i] = Math.min(open[i], close[i]) - 3.0 - (i % 3) * 0.2;
      volume[i] = 100 + i * 5;
      timestamps[i] = start + BigInt(i * 60_000);
      price = close[i] + Math.sin(i / 5) * 0.25;
    }

    chart.set_data_arrays(open, high, low, close, volume, timestamps);
    chart.set_volume_visible(false);
    chart.reset_viewport('fit_all');
    chart.render();
    (window as any).__responsiveHarness = { chart, host };
  });
}

test('canvas rendering stays stable across responsive viewport sizes', async ({ page }) => {
  await installResponsiveHarness(page);

  const sizes = [
    { width: 375, height: 667 },
    { width: 768, height: 1024 },
    { width: 960, height: 540 },
    { width: 1366, height: 768 },
  ];

  for (const size of sizes) {
    await page.setViewportSize(size);
    await page.evaluate(() => {
      const { chart } = (window as any).__responsiveHarness;
      chart.render();
    });
    await page.waitForTimeout(80);

    const metrics = await page.evaluate(() => {
      const dpr = window.devicePixelRatio || 1;
      const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>('#responsive-render-harness canvas'));
      const canvasMetrics = canvases.map((canvas) => {
        const rect = canvas.getBoundingClientRect();
        const style = getComputedStyle(canvas);
        return {
          id: canvas.id,
          imageRendering: style.imageRendering,
          widthError: Math.abs(canvas.width - Math.round(rect.width * dpr)),
          heightError: Math.abs(canvas.height - Math.round(rect.height * dpr)),
          bitmapWidth: canvas.width,
          bitmapHeight: canvas.height,
          cssWidth: rect.width,
          cssHeight: rect.height,
        };
      });

      const pane = document.querySelector<HTMLCanvasElement>('#aion_charts-pane-chart');
      if (!pane) throw new Error('pane chart canvas missing');
      const ctx = pane.getContext('2d', { willReadFrequently: true });
      if (!ctx) throw new Error('2d context missing');
      const { width, height } = pane;
      const data = ctx.getImageData(0, 0, width, height).data;
      const isCandlePixel = (idx: number) => {
        const r = data[idx];
        const g = data[idx + 1];
        const b = data[idx + 2];
        const a = data[idx + 3];
        const bullish = Math.abs(r - 9) <= 4 && Math.abs(g - 117) <= 6 && Math.abs(b - 100) <= 6;
        const bearish = Math.abs(r - 167) <= 6 && Math.abs(g - 11) <= 4 && Math.abs(b - 60) <= 6;
        return a > 180 && (bullish || bearish);
      };

      let coloredPixels = 0;
      let tallColoredColumns = 0;
      for (let x = 0; x < width; x += 1) {
        let run = 0;
        let maxRun = 0;
        for (let y = 0; y < height; y += 1) {
          if (isCandlePixel((y * width + x) * 4)) {
            coloredPixels += 1;
            run += 1;
            maxRun = Math.max(maxRun, run);
          } else {
            run = 0;
          }
        }
        if (maxRun >= Math.max(8, Math.round(7 * dpr))) {
          tallColoredColumns += 1;
        }
      }

      return { dpr, canvasMetrics, coloredPixels, tallColoredColumns };
    });

    expect(metrics.canvasMetrics, `canvases at ${size.width}x${size.height}`).not.toHaveLength(0);
    for (const canvas of metrics.canvasMetrics) {
      expect(canvas.imageRendering, `${canvas.id} must not force nearest-neighbor canvas scaling`).not.toMatch(/pixelated|crisp-edges/);
      expect(canvas.widthError, `${canvas.id} bitmap width should track CSS size at DPR`).toBeLessThanOrEqual(1);
      expect(canvas.heightError, `${canvas.id} bitmap height should track CSS size at DPR`).toBeLessThanOrEqual(1);
    }
    expect(metrics.coloredPixels, `candles should render at ${size.width}x${size.height}`).toBeGreaterThan(1000);
    expect(metrics.tallColoredColumns, `long candle wicks/bodies should survive at ${size.width}x${size.height}`).toBeGreaterThan(20);
  }
});

test.describe('mobile DPR rendering', () => {
  test.use({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 3,
    isMobile: true,
    hasTouch: true,
  });

  test('candlestick wicks keep TradingView-style physical width on high-DPR mobile', async ({ page }) => {
    await installResponsiveHarness(page);
    await page.evaluate(() => {
      const { chart } = (window as any).__responsiveHarness;
      chart.render();
    });
    await page.waitForTimeout(80);

    const metrics = await page.evaluate(() => {
      const dpr = window.devicePixelRatio || 1;
      const pane = document.querySelector<HTMLCanvasElement>('#aion_charts-pane-chart');
      if (!pane) throw new Error('pane chart canvas missing');
      const ctx = pane.getContext('2d', { willReadFrequently: true });
      if (!ctx) throw new Error('2d context missing');
      const image = ctx.getImageData(0, 0, pane.width, pane.height);
      const { data, width, height } = image;

      const isCandlePixel = (idx: number) => {
        const r = data[idx];
        const g = data[idx + 1];
        const b = data[idx + 2];
        const a = data[idx + 3];
        const bullish = Math.abs(r - 9) <= 4 && Math.abs(g - 117) <= 6 && Math.abs(b - 100) <= 6;
        const bearish = Math.abs(r - 167) <= 6 && Math.abs(g - 11) <= 4 && Math.abs(b - 60) <= 6;
        return a > 180 && (bullish || bearish);
      };

      const narrowRuns: number[] = [];
      const allRuns: number[] = [];
      for (let y = 0; y < height; y += 1) {
        let run = 0;
        for (let x = 0; x < width; x += 1) {
          if (isCandlePixel((y * width + x) * 4)) {
            run += 1;
          } else if (run > 0) {
            allRuns.push(run);
            if (run <= 8) narrowRuns.push(run);
            run = 0;
          }
        }
        if (run > 0) {
          allRuns.push(run);
          if (run <= 8) narrowRuns.push(run);
        }
      }

      const expectedWickWidth = Math.floor(dpr);
      const matchingWickRuns = narrowRuns.filter(run => Math.abs(run - expectedWickWidth) <= 1).length;
      const tooThickNarrowRuns = narrowRuns.filter(run => run > expectedWickWidth + 1).length;
      return {
        dpr,
        bitmapWidth: pane.width,
        bitmapHeight: pane.height,
        cssWidth: pane.getBoundingClientRect().width,
        cssHeight: pane.getBoundingClientRect().height,
        expectedWickWidth,
        allRunCount: allRuns.length,
        narrowRunCount: narrowRuns.length,
        matchingWickRuns,
        tooThickNarrowRuns,
      };
    });

    expect(metrics.dpr).toBe(3);
    expect(metrics.matchingWickRuns).toBeGreaterThan(20);
    expect(metrics.tooThickNarrowRuns / Math.max(1, metrics.narrowRunCount)).toBeLessThan(0.1);
  });
});

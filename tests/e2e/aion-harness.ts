import { expect, type Page } from '@playwright/test';

export type HarnessState = {
  chart: any;
  host: HTMLElement;
  bars: {
    open: Float64Array;
    high: Float64Array;
    low: Float64Array;
    close: Float64Array;
    volume: Float64Array;
    timestamps: BigUint64Array;
  };
};

export async function installHarness(page: Page) {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  await page.evaluate(async () => {
    if ((window as any).__aionHarness) return;

    const module = await import('/wasm/pkg/aion_charts_wasm.js');
    await module.default({ module_or_path: '/wasm/pkg/aion_charts_wasm_bg.wasm' });

    const host = document.createElement('div');
    host.id = 'aion-e2e-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:720px',
      'height:420px',
      'background:#131315',
      'z-index:2147483647',
      'overflow:hidden',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.Aion_charts.create_chart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });

    const count = 64;
    const open = new Float64Array(count);
    const high = new Float64Array(count);
    const low = new Float64Array(count);
    const close = new Float64Array(count);
    const volume = new Float64Array(count);
    const timestamps = new BigUint64Array(count);
    let price = 100;
    const start = 1_700_000_000_000n;
    for (let i = 0; i < count; i += 1) {
      const delta = Math.sin(i / 4) * 1.5 + (i % 5 === 0 ? 1.25 : -0.35);
      open[i] = price;
      close[i] = price + delta;
      high[i] = Math.max(open[i], close[i]) + 1.5;
      low[i] = Math.min(open[i], close[i]) - 1.5;
      volume[i] = 100 + i * 3;
      timestamps[i] = start + BigInt(i * 60_000);
      price = close[i];
    }

    chart.set_data_arrays(open, high, low, close, volume, timestamps);
    chart.reset_viewport('fit_all');
    chart.render();

    document.addEventListener('keydown', event => {
      const selectedRaw = chart.get_selected_drawing_info_json();
      const selected = selectedRaw && selectedRaw !== 'null' ? JSON.parse(selectedRaw) : null;
      let handled = false;

      if (selected?.text_editing) {
        handled = chart.on_key_down(
          event.key,
          event.ctrlKey || event.metaKey,
          event.shiftKey,
          event.altKey,
        );
      } else if (event.key === 'Enter') {
        handled = chart.complete_drawing();
      } else if (event.key === 'Delete' || event.key === 'Backspace') {
        chart.remove_selected_drawing();
        handled = true;
      } else if (event.key === 'Escape') {
        chart.cancel_drawing();
        chart.deselect_drawings();
        handled = true;
      }

      if (handled) {
        chart.render();
        event.preventDefault();
      }
    });

    (window as any).__aionHarness = { chart, host, bars: { open, high, low, close, volume, timestamps } };
  });
}

export async function getHarness(page: Page): Promise<HarnessState> {
  return page.evaluateHandle(() => (window as any).__aionHarness) as Promise<any>;
}

export async function canvasLocator(page: Page) {
  return page.locator('#aion-e2e-harness canvas').first();
}

export async function commitCanvasFrame(page: Page) {
  await page.evaluate(() => {
    const { chart } = (window as any).__aionHarness;
    chart.render();
  });
  await page.waitForTimeout(50);
}

export async function importTextDrawing(page: Page, text = '') {
  await page.evaluate((value) => {
    const { chart } = (window as any).__aionHarness;
    chart.import_drawings(JSON.stringify({
      version: 1,
      main: {
        version: 7,
        drawings: [{
          id: 1001,
          tool: 'text',
          locked: false,
          style: {
            color: [0.88, 0.47, 0.04, 1],
            line_width: 1,
            fill_color: [0.88, 0.47, 0.04, 0.15],
            dash: null,
            font_size: 14,
          },
          anchors: [{
            point: { bar_index: 32, price: 103, timestamp: null },
            hit_radius: 5,
          }],
          points: [],
          text: value,
          horizontal_align: 'left',
          vertical_align: 'top',
          text_font_size: 20,
          border_enabled: false,
          fill_enabled: false,
        }],
      },
      subpanes: [],
    }));
    chart.render();
  }, text);
}

export async function readSelectedDrawingInfo(page: Page) {
  return page.evaluate(() => {
    const { chart } = (window as any).__aionHarness;
    const raw = chart.get_selected_drawing_info_json();
    return raw && raw !== 'null' ? JSON.parse(raw) : null;
  });
}

export async function clickTextDrawing(page: Page) {
  const point = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    return chart.project_point(bars.timestamps[32], 103);
  });
  const canvas = await (await canvasLocator(page)).boundingBox();
  if (!canvas || !Number.isFinite(point.x) || !Number.isFinite(point.y)) {
    throw new Error('Text drawing test point is not visible');
  }
  await page.mouse.click(canvas.x + point.x + 8, canvas.y + point.y + 8);
}

export async function startTextEdit(page: Page) {
  await clickTextDrawing(page);
  await page.evaluate(() => {
    const { chart } = (window as any).__aionHarness;
    chart.begin_selected_drawing_text_edit();
    chart.render();
  });
}

export async function textPixelBounds(page: Page) {
  return page.evaluate(() => {
    const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>('#aion-e2e-harness canvas'));
    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    let maxY = Number.NEGATIVE_INFINITY;
    let count = 0;

    for (const canvas of canvases) {
      const ctx = canvas.getContext('2d');
      if (!ctx || canvas.width === 0 || canvas.height === 0) continue;
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      const xScale = canvas.clientWidth > 0 ? canvas.width / canvas.clientWidth : 1;
      const yScale = canvas.clientHeight > 0 ? canvas.height / canvas.clientHeight : 1;
      for (let y = 0; y < canvas.height; y += 1) {
        for (let x = 0; x < canvas.width; x += 1) {
          const idx = (y * canvas.width + x) * 4;
          const r = data[idx];
          const g = data[idx + 1];
          const b = data[idx + 2];
          const a = data[idx + 3];
          const isTextPixel = a > 30 && r > 140 && g > 55 && g < 180 && b < 100;
          if (!isTextPixel) continue;
          const cssX = x / xScale;
          const cssY = y / yScale;
          minX = Math.min(minX, cssX);
          minY = Math.min(minY, cssY);
          maxX = Math.max(maxX, cssX);
          maxY = Math.max(maxY, cssY);
          count += 1;
        }
      }
    }

    if (count === 0) return null;
    return {
      x: minX,
      y: minY,
      width: maxX - minX + 1,
      height: maxY - minY + 1,
      count,
    };
  });
}

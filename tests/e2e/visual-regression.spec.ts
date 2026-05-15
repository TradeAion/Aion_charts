import { expect, test, type Page } from '@playwright/test';

async function installVisualHarness(page: Page, name: string) {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  await page.evaluate(async ({ harnessName }) => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = `visual-${harnessName}`;
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#101216',
      'z-index:2147483647',
      'overflow:hidden',
    ].join(';');
    document.body.replaceChildren(host);
    document.body.style.margin = '0';
    document.body.style.background = '#101216';

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      crosshair: { mode: 'magnet_ohlc' },
      priceScale: { margins: { top: 0.18, bottom: 0.16 } },
      handleScroll: false,
      handleScale: false,
      kineticScroll: false,
    });

    const start = 1_700_000_000_000;
    const data = Array.from({ length: 48 }, (_, index) => {
      const base = 102 + Math.sin(index / 4) * 4 + index * 0.08;
      return {
        time: start + index * 60_000,
        open: base - Math.sin(index / 3) * 1.2,
        high: base + 2.4 + (index % 6) * 0.18,
        low: base - 2.1 - (index % 5) * 0.16,
        close: base + Math.cos(index / 5) * 1.7,
        volume: 200 + index * 3,
      };
    });

    const candle = chart.addSeries(module.CandlestickSeries);
    candle.setData(data);
    chart.timeScale().fitContent();
    chart.raw().set_last_price_line_visible(false);
    chart.raw().set_last_price_label_visible(false);
    chart.raw().render();

    (window as any).__visualHarness = { chart, host, data, module };
  }, { harnessName: name });
}

test('visual baseline: initial candlestick facade options', async ({ page }) => {
  await installVisualHarness(page, 'initial-options');
  await expect(page.locator('#visual-initial-options')).toHaveScreenshot('initial-candlestick-options.png', {
    animations: 'disabled',
  });
});

test('visual baseline: markers and price lines', async ({ page }) => {
  await installVisualHarness(page, 'markers-price-lines');

  await page.evaluate(() => {
    const { chart, data } = (window as any).__visualHarness;
    const priceLineId = chart.raw().create_price_line(106.6, 0.149, 0.545, 0.824, 1, 2, 'dashed', false);
    chart.raw().set_price_line_label(priceLineId, 'TP 106.60');
    chart.raw().add_marker_at_time(0, BigInt(data[12].time), 'arrow_up', 'below_bar', 0, 0.18, 0.72, 0.48, 1, 1.15, 'Buy');
    chart.raw().add_marker_at_time(0, BigInt(data[28].time), 'arrow_down', 'above_bar', 0, 0.94, 0.32, 0.32, 1, 1.15, 'Sell');
    chart.raw().set_marker_z_order('top');
    chart.raw().render();
  });

  await expect(page.locator('#visual-markers-price-lines')).toHaveScreenshot('markers-price-lines.png', {
    animations: 'disabled',
  });
});

test('visual baseline: applying visible range and percentage price scale options', async ({ page }) => {
  await installVisualHarness(page, 'applied-options');

  await page.evaluate(() => {
    const { chart, data } = (window as any).__visualHarness;
    chart.timeScale().setVisibleRange({ from: data[8].time, to: data[34].time });
    chart.priceScale().applyOptions({
      mode: 2,
      scaleMargins: { top: 0.12, bottom: 0.22 },
    });
    chart.raw().render();
  });

  await expect(page.locator('#visual-applied-options')).toHaveScreenshot('applied-options-percentage-scale.png', {
    animations: 'disabled',
  });
});

test('visual baseline: visible and hidden overlay series', async ({ page }) => {
  await installVisualHarness(page, 'series-visibility');

  await page.evaluate(() => {
    const { chart, data } = (window as any).__visualHarness;
    const area = chart.addSeries((window as any).AreaSeries || 'Area', {
      lineColor: '#4ea1ff',
      topColor: '#4ea1ff55',
      bottomColor: '#4ea1ff08',
      lineWidth: 2,
    });
    area.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 98 + Math.sin(index / 5) * 2.4 + index * 0.05,
    })));

    const hiddenLine = chart.addSeries((window as any).LineSeries || 'Line', {
      color: '#ffcc33',
      lineWidth: 3,
    });
    hiddenLine.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 110 - Math.cos(index / 4) * 3,
    })));
    hiddenLine.applyOptions({ visible: false });

    chart.raw().render();
  });

  await expect(page.locator('#visual-series-visibility')).toHaveScreenshot('series-visibility.png', {
    animations: 'disabled',
  });
});

test('visual baseline: crosshair state', async ({ page }) => {
  await installVisualHarness(page, 'crosshair-state');

  await page.evaluate(() => {
    const { chart } = (window as any).__visualHarness;
    chart.raw().set_crosshair_state(true, 318, 154, 24, 105.4, 'normal');
    chart.raw().render();
  });

  await expect(page.locator('#visual-crosshair-state')).toHaveScreenshot('crosshair-state.png', {
    animations: 'disabled',
  });
});

test('visual baseline: resized chart layout', async ({ page }) => {
  await installVisualHarness(page, 'resized-layout');

  await page.evaluate(() => {
    const { chart, host, data } = (window as any).__visualHarness;
    host.style.width = '760px';
    host.style.height = '300px';
    chart.resize(760, 300);
    chart.timeScale().setVisibleRange({ from: data[4].time, to: data[44].time });
    chart.raw().render();
  });

  await expect(page.locator('#visual-resized-layout')).toHaveScreenshot('resized-layout.png', {
    animations: 'disabled',
  });
});

test('visual baseline: baseline and histogram overlay options', async ({ page }) => {
  await installVisualHarness(page, 'baseline-histogram-options');

  await page.evaluate(() => {
    const { chart, data, module } = (window as any).__visualHarness;
    const baseline = chart.addSeries(module.BaselineSeries, {
      baseValue: { price: 101.8 },
      topLineColor: '#2fbf71',
      bottomLineColor: '#e05252',
      topFillColor1: '#2fbf7150',
      bottomFillColor1: '#e0525250',
      lineWidth: 2,
    });
    baseline.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 101.8 + Math.sin(index / 4) * 4.6,
    })));

    const histogram = chart.addSeries(module.HistogramSeries, {
      color: '#8ab4ff',
      base: 0,
    });
    histogram.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 1.2 + Math.abs(Math.sin(index / 3)) * 5,
      color: index % 2 === 0 ? '#8ab4ff99' : '#f6c85f99',
    })));

    chart.raw().render();
  });

  await expect(page.locator('#visual-baseline-histogram-options')).toHaveScreenshot('baseline-histogram-options.png', {
    animations: 'disabled',
  });
});

test('visual baseline: logarithmic price scale with tighter margins', async ({ page }) => {
  await installVisualHarness(page, 'log-price-scale-options');

  await page.evaluate(() => {
    const { chart, data } = (window as any).__visualHarness;
    chart.timeScale().setVisibleRange({ from: data[6].time, to: data[42].time });
    chart.priceScale().applyOptions({
      mode: 'log',
      scaleMargins: { top: 0.08, bottom: 0.08 },
    });
    chart.raw().render();
  });

  await expect(page.locator('#visual-log-price-scale-options')).toHaveScreenshot('log-price-scale-options.png', {
    animations: 'disabled',
  });
});

test('visual baseline: indexed price scale with overlay reference line', async ({ page }) => {
  await installVisualHarness(page, 'indexed-price-scale-options');

  await page.evaluate(() => {
    const { chart, data, module } = (window as any).__visualHarness;
    chart.timeScale().setVisibleRange({ from: data[3].time, to: data[38].time });
    chart.priceScale().applyOptions({
      mode: 'indexedTo100',
      scaleMargins: { top: 0.14, bottom: 0.18 },
    });

    const line = chart.addSeries(module.LineSeries, {
      color: '#ffcc33',
      lineWidth: 2,
    });
    line.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 102 + Math.cos(index / 6) * 2.2 + index * 0.06,
    })));
    chart.raw().render();
  });

  await expect(page.locator('#visual-indexed-price-scale-options')).toHaveScreenshot('indexed-price-scale-options.png', {
    animations: 'disabled',
  });
});

test('visual baseline: magnet crosshair over mixed overlays', async ({ page }) => {
  await installVisualHarness(page, 'magnet-crosshair-overlays');

  await page.evaluate(() => {
    const { chart, data, module } = (window as any).__visualHarness;
    const area = chart.addSeries(module.AreaSeries, {
      lineColor: '#55d6be',
      topColor: '#55d6be44',
      bottomColor: '#55d6be05',
      lineWidth: 2,
    });
    area.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 99.5 + Math.sin(index / 4.5) * 2.8 + index * 0.04,
    })));

    chart.timeScale().setVisibleLogicalRange({ from: 6, to: 40 });
    chart.raw().set_crosshair_state(true, 348, 132, 25, data[25].close, 'magnet_ohlc');
    chart.raw().render();
  });

  await expect(page.locator('#visual-magnet-crosshair-overlays')).toHaveScreenshot('magnet-crosshair-overlays.png', {
    animations: 'disabled',
  });
});

test('visual baseline: chart applyOptions theme switch', async ({ page }) => {
  await installVisualHarness(page, 'apply-options-theme-switch');

  await page.evaluate(() => {
    const { chart, data, host } = (window as any).__visualHarness;
    host.style.background = '#f8f9fd';
    document.body.style.background = '#f8f9fd';
    chart.applyOptions({ theme: 'light' });
    chart.timeScale().setVisibleRange({ from: data[10].time, to: data[45].time });
    chart.priceScale().applyOptions({
      mode: 'normal',
      scaleMargins: { top: 0.16, bottom: 0.2 },
    });
    chart.raw().render();
  });

  await expect(page.locator('#visual-apply-options-theme-switch')).toHaveScreenshot('apply-options-theme-switch.png', {
    animations: 'disabled',
  });
});

test('visual baseline: series visibility toggles and removed overlays', async ({ page }) => {
  await installVisualHarness(page, 'series-toggle-remove');

  await page.evaluate(() => {
    const { chart, data, module } = (window as any).__visualHarness;
    const visibleLine = chart.addSeries(module.LineSeries, {
      color: '#f6c85f',
      lineWidth: 3,
    });
    visibleLine.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 101 + Math.sin(index / 3.8) * 3.2,
    })));
    visibleLine.applyOptions({ visible: false });
    visibleLine.applyOptions({ visible: true });

    const removedArea = chart.addSeries(module.AreaSeries, {
      lineColor: '#c17cff',
      topColor: '#c17cff44',
      bottomColor: '#c17cff05',
      lineWidth: 2,
    });
    removedArea.setData(data.map((bar: any, index: number) => ({
      time: bar.time,
      value: 106 - Math.cos(index / 4.2) * 2.4,
    })));
    chart.removeSeries(removedArea);

    chart.timeScale().setVisibleLogicalRange({ from: 4, to: 42 });
    chart.raw().render();
  });

  await expect(page.locator('#visual-series-toggle-remove')).toHaveScreenshot('series-toggle-remove.png', {
    animations: 'disabled',
  });
});

test('visual baseline: multiple price-line styles and marker layers', async ({ page }) => {
  await installVisualHarness(page, 'price-lines-marker-layers');

  await page.evaluate(() => {
    const { chart, data } = (window as any).__visualHarness;
    const stopLineId = chart.raw().create_price_line(99.4, 0.94, 0.32, 0.32, 1, 1, 'dotted', false);
    chart.raw().set_price_line_label(stopLineId, 'SL 99.40');
    const targetLineId = chart.raw().create_price_line(108.2, 0.18, 0.75, 0.45, 1, 2, 'large_dashed', false);
    chart.raw().set_price_line_label(targetLineId, 'Target 108.20');
    chart.raw().add_marker_at_time(0, BigInt(data[9].time), 'circle', 'in_bar', 0, 0.95, 0.78, 0.24, 1, 1.25, 'Note');
    chart.raw().add_marker_at_time(0, BigInt(data[19].time), 'arrow_up', 'below_bar', 0, 0.18, 0.75, 0.45, 1, 1.2, 'Long');
    chart.raw().add_marker_at_time(0, BigInt(data[31].time), 'arrow_down', 'above_bar', 0, 0.94, 0.32, 0.32, 1, 1.2, 'Exit');
    chart.raw().set_marker_z_order('aboveSeries');
    chart.timeScale().setVisibleRange({ from: data[4].time, to: data[38].time });
    chart.raw().render();
  });

  await expect(page.locator('#visual-price-lines-marker-layers')).toHaveScreenshot('price-lines-marker-layers.png', {
    animations: 'disabled',
  });
});

test('visual baseline: indicator pane layout and separators', async ({ page }) => {
  await installVisualHarness(page, 'indicator-pane-layout');

  await page.evaluate(() => {
    const { chart, host, data } = (window as any).__visualHarness;
    host.style.width = '720px';
    host.style.height = '420px';
    chart.resize(720, 420);
    const studyId = chart.raw().create_study('rsi');
    chart.raw().add_indicator_pane(studyId, 'rsi', 108);
    chart.raw().set_subpane_separator_color(0.29, 0.34, 0.42, 1);
    chart.raw().set_subpane_separator_hover_color(0.18, 0.55, 0.82, 1);
    chart.raw().set_subpane_separator_thickness(2);
    chart.timeScale().setVisibleRange({ from: data[7].time, to: data[45].time });
    chart.raw().render();
  });

  await expect(page.locator('#visual-indicator-pane-layout')).toHaveScreenshot('indicator-pane-layout.png', {
    animations: 'disabled',
  });
});

test('visual baseline: host autosize observer resize', async ({ page }) => {
  await installVisualHarness(page, 'host-autosize-resize');

  await page.evaluate(async () => {
    const { chart, host, data } = (window as any).__visualHarness;
    host.style.width = '540px';
    host.style.height = '410px';
    await new Promise(requestAnimationFrame);
    await new Promise(requestAnimationFrame);
    chart.timeScale().setVisibleRange({ from: data[2].time, to: data[30].time });
    chart.raw().render();
  });

  await expect(page.locator('#visual-host-autosize-resize')).toHaveScreenshot('host-autosize-resize.png', {
    animations: 'disabled',
  });
});

test('visual baseline: initial light area facade options', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'visual-initial-light-area-options';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:620px',
      'height:340px',
      'background:#f8f9fd',
      'z-index:2147483647',
      'overflow:hidden',
    ].join(';');
    document.body.replaceChildren(host);
    document.body.style.margin = '0';
    document.body.style.background = '#f8f9fd';

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'light',
      priceScale: {
        mode: 'percentage',
        margins: { top: 0.12, bottom: 0.18 },
      },
      crosshair: { mode: 'normal' },
      handleScroll: { mouseWheel: false, pressedMouseMove: false },
      handleScale: { mouseWheel: false, axisDoubleClickReset: false },
      kineticScroll: false,
    });

    const start = 1_700_000_000_000;
    const area = chart.addSeries(module.AreaSeries, {
      lineColor: '#2563eb',
      topColor: '#2563eb55',
      bottomColor: '#2563eb08',
      lineWidth: 2,
    });
    area.setData(Array.from({ length: 42 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + Math.sin(index / 5) * 3.4 + index * 0.12,
    })));
    chart.timeScale().fitContent();
    chart.raw().set_last_price_line_visible(false);
    chart.raw().set_last_price_label_visible(false);
    chart.raw().render();
  });

  await expect(page.locator('#visual-initial-light-area-options')).toHaveScreenshot('initial-light-area-options.png', {
    animations: 'disabled',
  });
});

import { expect, test } from '@playwright/test';
import { installHarness } from './aion-harness';

test.beforeEach(async ({ page }) => {
  await installHarness(page);
});

test('marker APIs reject malformed input and stale series IDs', async ({ page }) => {
  const result = await page.evaluate(() => {
    const { chart } = (window as any).__aionHarness;
    const errors: string[] = [];

    const capture = (fn: () => unknown) => {
      try {
        fn();
      } catch (error) {
        errors.push(error instanceof Error ? error.message : String(error));
      }
    };

    capture(() => chart.add_marker(999, 1, 'circle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'missing'));
    capture(() => chart.add_marker(0, 999, 'circle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'oob'));
    capture(() => chart.add_marker(0, 1, 'triangle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'shape'));
    capture(() => chart.add_marker(0, 1, 'circle', 'at_price', Number.NaN, 0.2, 0.4, 1, 1, 6, 'nan'));
    capture(() => chart.set_markers(0, new Float64Array([1, 2, 0])));
    capture(() => chart.set_markers(0, new Float64Array([1, 2, 0, 0, 0.2, 0.4, 1, 2, 6])));
    capture(() => chart.set_marker_z_order('front'));
    capture(() => chart.add_marker_at_time(0, 1n, 'circle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'before'));
    capture(() => chart.set_time_markers(0, new BigUint64Array([1n, 2n]), new Float64Array([2, 0, 0, 1, 0, 1, 1, 6])));

    const defaultZOrder = chart.marker_z_order();
    chart.set_marker_z_order('aboveSeries');
    const aboveSeriesZOrder = chart.marker_z_order();
    chart.set_marker_z_order('top');
    const topZOrder = chart.marker_z_order();
    chart.set_marker_z_order('normal');
    const normalZOrder = chart.marker_z_order();

    const series = chart.add_line_series(0.2, 0.4, 1, 1, 2, 'solid');
    const markerId = chart.add_marker(series, 1, 'circle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'ok');
    const timeMarkerId = chart.add_marker_at_time(series, (window as any).__aionHarness.bars.timestamps[2], 'circle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'time');
    const removed = chart.remove_series(series);
    capture(() => chart.add_marker(series, 1, 'circle', 'above_bar', 0, 0.2, 0.4, 1, 1, 6, 'stale'));

    return {
      errors,
      markerId,
      timeMarkerId,
      removed,
      zOrders: [defaultZOrder, aboveSeriesZOrder, topZOrder, normalZOrder],
    };
  });

  expect(result.markerId).toBe(1);
  expect(result.timeMarkerId).toBe(2);
  expect(result.removed).toBe(true);
  expect(result.errors).toEqual(expect.arrayContaining([
    expect.stringContaining('marker series id 999 not found'),
    expect.stringContaining('marker bar_index 999 is outside loaded bars length'),
    expect.stringContaining('invalid marker shape'),
    expect.stringContaining('marker price must be finite'),
    expect.stringContaining('marker_data length must be a multiple of 9'),
    expect.stringContaining('color_a must be finite and between 0 and 1'),
    expect.stringContaining('invalid marker z_order'),
    expect.stringContaining('marker timestamp 1 is before loaded data'),
    expect.stringContaining('timestamps length 2 must match marker count 1'),
    expect.stringContaining('marker series id'),
  ]));
  expect(result.errors).toHaveLength(10);
  expect(result.zOrders).toEqual(['normal', 'aboveSeries', 'top', 'normal']);
});

test('timestamp-anchored markers stay attached after data prepend', async ({ page }) => {
  const result = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    const markerTime = bars.timestamps[24];
    const markerPrice = bars.close[24] + 8;

    chart.add_marker_at_time(0, markerTime, 'circle', 'at_price', markerPrice, 1, 0, 1, 1, 9, 'anchor');
    chart.render();

    const before = chart.project_point(markerTime, markerPrice);

    const prepend = 5;
    const count = bars.open.length + prepend;
    const open = new Float64Array(count);
    const high = new Float64Array(count);
    const low = new Float64Array(count);
    const close = new Float64Array(count);
    const volume = new Float64Array(count);
    const timestamps = new BigUint64Array(count);

    for (let i = 0; i < prepend; i += 1) {
      const src = 0;
      const offset = prepend - i;
      open[i] = bars.open[src] - offset;
      close[i] = bars.close[src] - offset;
      high[i] = Math.max(open[i], close[i]) + 1.5;
      low[i] = Math.min(open[i], close[i]) - 1.5;
      volume[i] = 50 + i;
      timestamps[i] = bars.timestamps[0] - BigInt((prepend - i) * 60_000);
    }
    for (let i = 0; i < bars.open.length; i += 1) {
      const dst = i + prepend;
      open[dst] = bars.open[i];
      high[dst] = bars.high[i];
      low[dst] = bars.low[i];
      close[dst] = bars.close[i];
      volume[dst] = bars.volume[i];
      timestamps[dst] = bars.timestamps[i];
    }

    chart.set_data_arrays(open, high, low, close, volume, timestamps);
    chart.reset_viewport('fit_all');
    chart.render();

    const after = chart.project_point(markerTime, markerPrice);

    const countMarkerPixelsNear = (point: { x: number; y: number }) => {
      const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>('#aion-e2e-harness canvas'));
      let count = 0;
      for (const canvas of canvases) {
        const ctx = canvas.getContext('2d');
        if (!ctx || canvas.clientWidth <= 0 || canvas.clientHeight <= 0) continue;
        const xScale = canvas.width / canvas.clientWidth;
        const yScale = canvas.height / canvas.clientHeight;
        const cx = Math.round(point.x * xScale);
        const cy = Math.round(point.y * yScale);
        const radius = Math.round(14 * Math.max(xScale, yScale));
        const x0 = Math.max(0, cx - radius);
        const y0 = Math.max(0, cy - radius);
        const x1 = Math.min(canvas.width - 1, cx + radius);
        const y1 = Math.min(canvas.height - 1, cy + radius);
        const data = ctx.getImageData(x0, y0, x1 - x0 + 1, y1 - y0 + 1).data;
        for (let i = 0; i < data.length; i += 4) {
          const r = data[i];
          const g = data[i + 1];
          const b = data[i + 2];
          const a = data[i + 3];
          if (a > 120 && r > 180 && g < 80 && b > 180) count += 1;
        }
      }
      return count;
    };

    return {
      before,
      after,
      pixelsAfter: countMarkerPixelsNear(after),
      shifted: Math.abs(after.x - before.x),
    };
  });

  expect(result.after.visible).toBe(true);
  expect(result.shifted).toBeGreaterThan(10);
  expect(result.pixelsAfter).toBeGreaterThan(20);
});

test('timestamp-anchored markers survive trims and full reloads while their timestamp remains', async ({ page }) => {
  const result = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    const markerTime = bars.timestamps[24];
    const markerPrice = bars.close[24] + 8;

    chart.clear_all_markers();
    chart.add_marker_at_time(0, markerTime, 'circle', 'at_price', markerPrice, 1, 0, 1, 1, 9, 'trim');

    const countMarkerPixelsNear = (point: { x: number; y: number }) => {
      const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>('#aion-e2e-harness canvas'));
      let count = 0;
      for (const canvas of canvases) {
        const ctx = canvas.getContext('2d');
        if (!ctx || canvas.clientWidth <= 0 || canvas.clientHeight <= 0) continue;
        const xScale = canvas.width / canvas.clientWidth;
        const yScale = canvas.height / canvas.clientHeight;
        const cx = Math.round(point.x * xScale);
        const cy = Math.round(point.y * yScale);
        const radius = Math.round(14 * Math.max(xScale, yScale));
        const x0 = Math.max(0, cx - radius);
        const y0 = Math.max(0, cy - radius);
        const x1 = Math.min(canvas.width - 1, cx + radius);
        const y1 = Math.min(canvas.height - 1, cy + radius);
        const data = ctx.getImageData(x0, y0, x1 - x0 + 1, y1 - y0 + 1).data;
        for (let i = 0; i < data.length; i += 4) {
          const r = data[i];
          const g = data[i + 1];
          const b = data[i + 2];
          const a = data[i + 3];
          if (a > 120 && r > 180 && g < 80 && b > 180) count += 1;
        }
      }
      return count;
    };

    const loadSlice = (start: number, priceDelta = 0) => {
      const count = bars.open.length - start;
      const open = new Float64Array(count);
      const high = new Float64Array(count);
      const low = new Float64Array(count);
      const close = new Float64Array(count);
      const volume = new Float64Array(count);
      const timestamps = new BigUint64Array(count);

      for (let i = 0; i < count; i += 1) {
        const src = i + start;
        open[i] = bars.open[src] + priceDelta;
        high[i] = bars.high[src] + priceDelta;
        low[i] = bars.low[src] + priceDelta;
        close[i] = bars.close[src] + priceDelta;
        volume[i] = bars.volume[src];
        timestamps[i] = bars.timestamps[src];
      }

      chart.set_data_arrays(open, high, low, close, volume, timestamps);
      chart.reset_viewport('fit_all');
      chart.render();
    };

    loadSlice(10);
    const afterTrim = chart.project_point(markerTime, markerPrice);
    const pixelsAfterTrim = countMarkerPixelsNear(afterTrim);

    loadSlice(10, 1.25);
    const afterReload = chart.project_point(markerTime, markerPrice);
    const pixelsAfterReload = countMarkerPixelsNear(afterReload);

    loadSlice(30);
    const afterTrimmedOut = chart.project_point(markerTime, markerPrice);

    return {
      afterTrim,
      afterReload,
      afterTrimmedOut,
      pixelsAfterTrim,
      pixelsAfterReload,
    };
  });

  expect(result.afterTrim.visible).toBe(true);
  expect(result.afterReload.visible).toBe(true);
  expect(result.afterTrimmedOut.visible).toBe(false);
  expect(result.pixelsAfterTrim).toBeGreaterThan(20);
  expect(result.pixelsAfterReload).toBeGreaterThan(20);
});

test('marker auto scale keeps above-bar markers inside the pane', async ({ page }) => {
  const result = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    let markerIndex = 0;
    for (let i = 1; i < bars.high.length; i += 1) {
      if (bars.high[i] > bars.high[markerIndex]) markerIndex = i;
    }

    const markerSize = 28;
    const markerTime = bars.timestamps[markerIndex];
    chart.clear_all_markers();
    chart.set_price_scale_margins(0, 0);
    chart.set_marker_auto_scale(false);
    chart.reset_viewport('fit_all');
    chart.add_marker_at_time(0, markerTime, 'circle', 'above_bar', 0, 1, 0, 1, 1, markerSize, 'scale');
    chart.render();
    const withoutAutoScale = chart.project_point(markerTime, bars.high[markerIndex]);

    chart.set_marker_auto_scale(true);
    chart.render();
    const withAutoScale = chart.project_point(markerTime, bars.high[markerIndex]);

    const markerCenter = {
      x: withAutoScale.x,
      y: withAutoScale.y - markerSize - 4,
    };

    const countMarkerPixelsNear = (point: { x: number; y: number }) => {
      const canvases = Array.from(document.querySelectorAll<HTMLCanvasElement>('#aion-e2e-harness canvas'));
      let count = 0;
      for (const canvas of canvases) {
        const ctx = canvas.getContext('2d');
        if (!ctx || canvas.clientWidth <= 0 || canvas.clientHeight <= 0) continue;
        const xScale = canvas.width / canvas.clientWidth;
        const yScale = canvas.height / canvas.clientHeight;
        const cx = Math.round(point.x * xScale);
        const cy = Math.round(point.y * yScale);
        const radius = Math.round(16 * Math.max(xScale, yScale));
        const x0 = Math.max(0, cx - radius);
        const y0 = Math.max(0, cy - radius);
        const x1 = Math.min(canvas.width - 1, cx + radius);
        const y1 = Math.min(canvas.height - 1, cy + radius);
        if (x1 < x0 || y1 < y0) continue;
        const data = ctx.getImageData(x0, y0, x1 - x0 + 1, y1 - y0 + 1).data;
        for (let i = 0; i < data.length; i += 4) {
          const r = data[i];
          const g = data[i + 1];
          const b = data[i + 2];
          const a = data[i + 3];
          if (a > 120 && r > 180 && g < 80 && b > 180) count += 1;
        }
      }
      return count;
    };

    return {
      defaultAutoScale: chart.marker_auto_scale(),
      withoutAutoScale,
      withAutoScale,
      markerCenter,
      pixels: countMarkerPixelsNear(markerCenter),
    };
  });

  expect(result.defaultAutoScale).toBe(true);
  expect(result.withoutAutoScale.y).toBeLessThan(6);
  expect(result.markerCenter.y).toBeGreaterThan(0);
  expect(result.withAutoScale.y).toBeGreaterThan(result.withoutAutoScale.y + 20);
  expect(result.pixels).toBeGreaterThan(20);
});

test('marker hit testing returns rendered marker metadata', async ({ page }) => {
  const result = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    const markerTime = bars.timestamps[18];
    const markerPrice = bars.close[18] + 4;

    chart.clear_all_markers();
    chart.set_marker_z_order('top');
    const markerId = chart.add_marker_at_time(0, markerTime, 'square', 'at_price', markerPrice, 1, 0, 1, 1, 10, 'hit');
    chart.reset_viewport('fit_all');
    chart.render();

    const point = chart.project_point(markerTime, markerPrice);
    const hit = chart.hit_test_marker(point.x, point.y);
    const nearMiss = chart.hit_test_marker(point.x + 80, point.y + 80);

    chart.clear_all_markers();
    chart.render();
    const afterClear = chart.hit_test_marker(point.x, point.y);

    return { markerId, markerTime: Number(markerTime), point, hit, nearMiss, afterClear };
  });

  expect(result.point.visible).toBe(true);
  expect(result.markerId).toBe(1);
  expect(result.hit).toMatchObject({
    seriesId: 0,
    markerId: 1,
    shape: 'square',
    position: 'atPrice',
    zOrder: 'top',
    text: 'hit',
  });
  expect(result.hit.timestamp).toBe(result.markerTime);
  expect(result.nearMiss).toBeNull();
  expect(result.afterClear).toBeNull();
});

test('marker hover events fire on enter and leave without duplicates', async ({ page }) => {
  const setup = await page.evaluate(() => {
    const { chart, bars } = (window as any).__aionHarness;
    const markerTime = bars.timestamps[20];
    const markerPrice = bars.close[20] + 3;

    chart.clear_all_markers();
    chart.set_marker_z_order('top');
    chart.add_marker_at_time(0, markerTime, 'circle', 'at_price', markerPrice, 1, 0, 1, 1, 10, 'hover');
    chart.reset_viewport('fit_all');
    chart.render();

    const events: any[] = [];
    chart.on('markerHover', (event: any) => {
      events.push(event);
    });
    (window as any).__markerHoverEvents = events;

    return chart.project_point(markerTime, markerPrice);
  });

  expect(setup.visible).toBe(true);
  const canvasBox = await page.locator('#aion-e2e-harness canvas').first().boundingBox();
  expect(canvasBox).not.toBeNull();

  await page.mouse.move(canvasBox!.x + setup.x, canvasBox!.y + setup.y);
  await page.evaluate(() => (window as any).__aionHarness.chart.render());
  await page.waitForTimeout(30);
  await page.mouse.move(canvasBox!.x + setup.x + 2, canvasBox!.y + setup.y + 1);
  await page.evaluate(() => (window as any).__aionHarness.chart.render());
  await page.waitForTimeout(30);
  await page.mouse.move(canvasBox!.x + setup.x + 120, canvasBox!.y + setup.y + 120);
  await page.evaluate(() => (window as any).__aionHarness.chart.render());
  await page.waitForTimeout(30);

  const events = await page.evaluate(() => (window as any).__markerHoverEvents);

  expect(events).toHaveLength(2);
  expect(events[0]).toMatchObject({
    type: 'markerHover',
    seriesId: 0,
    markerId: 1,
    shape: 'circle',
    position: 'atPrice',
    zOrder: 'top',
    text: 'hover',
  });
  expect(events[1]).toMatchObject({
    type: 'markerHover',
    seriesId: null,
    markerId: null,
    barIndex: null,
    timestamp: null,
    shape: null,
    position: null,
    zOrder: null,
    text: null,
  });
});

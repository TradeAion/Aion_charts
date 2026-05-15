import { expect, test } from '@playwright/test';

test('compat facade creates a chart, series, time scale, and subscriptions', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-facade-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: false,
      handleScale: false,
      kineticScroll: { touch: false, mouse: true },
    });
    const series = chart.addSeries(module.LineSeries, { color: '#ff00ff', lineWidth: 2 });
    const dataChangedEvents: any[] = [];
    series.subscribeDataChanged(event => dataChangedEvents.push(event));
    const start = 1_700_000_000_000;
    const data = Array.from({ length: 32 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + Math.sin(index / 4) * 4 + index * 0.25,
    }));
    series.setData(data);
    series.update({ time: start + 32 * 60_000, value: 112 });
    chart.timeScale().fitContent();
    chart.raw().render();

    const targetTime = data[10].time;
    const x = chart.timeScale().timeToCoordinate(targetTime);
    const logical = x == null ? null : chart.timeScale().coordinateToLogical(x);
    const roundTripTime = x == null ? null : chart.timeScale().coordinateToTime(x);
    const visibleRange = chart.timeScale().getVisibleRange();
    chart.priceScale().applyOptions({ mode: 2, scaleMargins: { top: 0.18, bottom: 0.12 } });
    const priceScaleOptions = chart.priceScale().options();
    const priceScaleWidth = chart.priceScale().width();
    const exportedOptions = JSON.parse(chart.raw().export_persistence_state(null)).options;
    const paneTouchAction = host.querySelector('#aion_charts-pane')?.style.touchAction;

    const crosshairEvents: any[] = [];
    chart.subscribeCrosshairMove(event => {
      crosshairEvents.push(event);
    });

    const dblClickEvents: any[] = [];
    const onDblClick = event => dblClickEvents.push(event);
    chart.subscribeDblClick(onDblClick);
    host.dispatchEvent(new MouseEvent('dblclick', {
      bubbles: true,
      clientX: host.getBoundingClientRect().left + 120,
      clientY: host.getBoundingClientRect().top + 80,
    }));
    chart.unsubscribeDblClick(onDblClick);
    host.dispatchEvent(new MouseEvent('dblclick', {
      bubbles: true,
      clientX: host.getBoundingClientRect().left + 140,
      clientY: host.getBoundingClientRect().top + 90,
    }));

    return {
      x,
      logical,
      roundTripTime,
      visibleRange,
      crosshairEventsLength: crosshairEvents.length,
      dataChangedEvents,
      priceScaleOptions,
      priceScaleWidth,
      exportedOptions,
      paneTouchAction,
      dblClickEventsLength: dblClickEvents.length,
      dblClickType: dblClickEvents[0]?.type,
    };
  });

  expect(result.x).toEqual(expect.any(Number));
  expect(result.logical).toEqual(expect.any(Number));
  expect(result.roundTripTime).toEqual(expect.any(Number));
  expect(result.visibleRange?.from).toEqual(expect.any(Number));
  expect(result.visibleRange?.to).toEqual(expect.any(Number));
  expect(result.crosshairEventsLength).toBe(0);
  expect(result.dataChangedEvents.map((event: any) => event.scope)).toEqual(['full', 'update']);
  expect(result.priceScaleOptions.mode).toBe('percentage');
  expect(result.priceScaleOptions.scaleMargins.top).toBeCloseTo(0.18);
  expect(result.priceScaleOptions.scaleMargins.bottom).toBeCloseTo(0.12);
  expect(result.priceScaleWidth).toBeGreaterThan(0);
  expect(result.exportedOptions.handleScroll.mouseWheel).toBe(false);
  expect(result.exportedOptions.handleScale.mouseWheel).toBe(false);
  expect(result.exportedOptions.kineticScroll.touch).toBe(false);
  expect(result.exportedOptions.kineticScroll.mouse).toBe(true);
  expect(result.paneTouchAction).toBe('auto');
  expect(result.dblClickEventsLength).toBe(1);
  expect(result.dblClickType).toBe('dblclick');
});

test('compat facade manages series markers and chart marker options', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-marker-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: false,
      handleScale: false,
    });
    const series = chart.addSeries(module.CandlestickSeries, {
      upColor: '#22c55e',
      downColor: '#ef4444',
    });
    const start = 1_700_000_000_000;
    const data = Array.from({ length: 48 }, (_, index) => ({
      time: start + index * 60_000,
      open: 100 + Math.sin(index / 3) * 4 + index * 0.2,
      high: 103 + Math.sin(index / 3) * 4 + index * 0.2,
      low: 97 + Math.sin(index / 3) * 4 + index * 0.2,
      close: 101 + Math.sin(index / 3) * 4 + index * 0.2,
    }));
    series.setData(data);
    chart.applyMarkerOptions({ zOrder: 'top', autoScale: true });

    const target = data[20];
    const firstId = series.createMarker({
      time: target.time,
      shape: 'square',
      position: 'atPrice',
      price: target.close,
      color: '#ff00ff',
      size: 12,
      text: 'facade',
    });
    chart.timeScale().fitContent();
    chart.raw().render();
    const point = chart.raw().project_point(BigInt(target.time), target.close);
    const hit = chart.raw().hit_test_marker(point.x, point.y);

    const removed = series.removeMarker(firstId);
    series.setMarkers([
      {
        time: data[22].time,
        shape: 'arrowUp',
        position: 'belowBar',
        color: '#22c55e',
        size: 10,
        text: 'entry',
      },
      {
        time: data[24].time,
        shape: 'arrowDown',
        position: 'aboveBar',
        color: '#ef4444',
        size: 10,
        text: 'exit',
      },
    ]);
    const secondPoint = chart.raw().project_point(BigInt(data[22].time), data[22].low);
    chart.raw().render();
    const missRemoved = chart.raw().hit_test_marker(point.x, point.y);
    const afterSetMarkers = chart.raw().hit_test_marker(secondPoint.x, secondPoint.y + 16);
    series.clearMarkers();
    chart.raw().render();
    const afterClear = chart.raw().hit_test_marker(secondPoint.x, secondPoint.y + 16);

    return {
      firstId,
      markerOptions: chart.markerOptions(),
      point,
      hit,
      removed,
      missRemoved,
      afterSetMarkers,
      afterClear,
    };
  });

  expect(result.firstId).toBe(1);
  expect(result.markerOptions).toEqual({ zOrder: 'top', autoScale: true });
  expect(result.point.visible).toBe(true);
  expect(result.hit).toMatchObject({
    seriesId: 0,
    markerId: 1,
    shape: 'square',
    position: 'atPrice',
    zOrder: 'top',
    text: 'facade',
  });
  expect(result.removed).toBe(true);
  expect(result.missRemoved).toBeNull();
  expect(result.afterSetMarkers).toMatchObject({
    seriesId: 0,
    markerId: 2,
    shape: 'arrowUp',
    position: 'belowBar',
    zOrder: 'top',
    text: 'entry',
  });
  expect(result.afterClear).toBeNull();
});

test('compat facade manages text and image watermarks', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-watermark-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    chart.applyWatermarkOptions({
      visible: true,
      text: 'AION_CHARTS',
      color: 'rgba(148, 163, 184, 0.32)',
      fontSize: 54,
      fontWeight: 700,
      horzAlign: 'center',
      vertAlign: 'center',
      zOrder: 'top',
    });
    const textNode = host.querySelector('.aion_charts-watermark');
    const textStyle = textNode ? window.getComputedStyle(textNode) : null;
    const textSnapshot = {
      text: textNode?.textContent,
      color: textStyle?.color,
      fontSize: textStyle?.fontSize,
      pointerEvents: textStyle?.pointerEvents,
      zIndex: textStyle?.zIndex,
      options: chart.watermarkOptions(),
    };

    const imageUrl = 'data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%22120%22 height=%2248%22%3E%3Crect width=%22120%22 height=%2248%22 rx=%228%22 fill=%22%2322c55e%22/%3E%3C/svg%3E';
    chart.applyWatermarkOptions({
      visible: true,
      imageUrl,
      alt: 'brand watermark',
      width: 120,
      height: 48,
      horzAlign: 'right',
      vertAlign: 'bottom',
      paddingX: 20,
      paddingY: 16,
      layer: 'background',
    });
    const imageNode = host.querySelector('.aion_charts-watermark img') as HTMLImageElement | null;
    const imageWrapper = host.querySelector('.aion_charts-watermark') as HTMLElement | null;
    const imageStyle = imageWrapper ? window.getComputedStyle(imageWrapper) : null;
    const box = imageWrapper?.getBoundingClientRect();
    const imageSnapshot = {
      src: imageNode?.src,
      alt: imageNode?.alt,
      maxWidth: imageNode?.style.maxWidth,
      maxHeight: imageNode?.style.maxHeight,
      zIndex: imageStyle?.zIndex,
      box: box ? { left: box.left, top: box.top, right: box.right, bottom: box.bottom } : null,
      options: chart.watermarkOptions(),
    };

    chart.clearWatermark();
    const afterClear = host.querySelector('.aion_charts-watermark');
    chart.remove();

    return {
      textSnapshot,
      imageSnapshot,
      afterClearExists: !!afterClear,
      afterRemoveExists: !!host.querySelector('.aion_charts-watermark'),
    };
  });

  expect(result.textSnapshot.text).toBe('AION_CHARTS');
  expect(result.textSnapshot.fontSize).toBe('54px');
  expect(result.textSnapshot.pointerEvents).toBe('none');
  expect(result.textSnapshot.zIndex).toBe('20');
  expect(result.textSnapshot.options.visible).toBe(true);
  expect(result.textSnapshot.options.zOrder).toBe('top');
  expect(result.imageSnapshot.alt).toBe('brand watermark');
  expect(result.imageSnapshot.maxWidth).toBe('120px');
  expect(result.imageSnapshot.maxHeight).toBe('48px');
  expect(result.imageSnapshot.zIndex).toBe('1');
  expect(result.imageSnapshot.options.horzAlign).toBe('right');
  expect(result.imageSnapshot.options.vertAlign).toBe('bottom');
  expect(result.imageSnapshot.options.layer).toBe('background');
  expect(result.afterClearExists).toBe(false);
  expect(result.afterRemoveExists).toBe(false);
});

test('compat facade autoscale contributions expand and release price fit', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-autoscale-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: false,
      handleScale: false,
    });
    const series = chart.addSeries(module.CandlestickSeries);
    const start = 1_700_000_000_000;
    const data = Array.from({ length: 48 }, (_, index) => ({
      time: start + index * 60_000,
      open: 100 + index * 0.1,
      high: 104 + index * 0.1,
      low: 96 + index * 0.1,
      close: 101 + index * 0.1,
    }));
    series.setData(data);
    chart.timeScale().fitContent();
    chart.raw().render();

    const before = chart.raw().project_point(BigInt(data[12].time), 500);
    const id = chart.addAutoscaleContribution({ minPrice: -200, maxPrice: 500 });
    chart.raw().render();
    const withContributionHigh = chart.raw().project_point(BigInt(data[12].time), 500);
    const withContributionLow = chart.raw().project_point(BigInt(data[12].time), -200);
    const removed = chart.removeAutoscaleContribution(id);
    chart.raw().render();
    const afterRemove = chart.raw().project_point(BigInt(data[12].time), 500);

    chart.addAutoscaleContribution({ minPrice: -300, maxPrice: 700 });
    chart.clearAutoscaleContributions();
    chart.raw().render();
    const afterClear = chart.raw().project_point(BigInt(data[12].time), 700);

    return {
      id,
      before,
      withContributionHigh,
      withContributionLow,
      removed,
      afterRemove,
      afterClear,
    };
  });

  expect(result.id).toBe(1);
  expect(result.before.visible).toBe(false);
  expect(result.withContributionHigh.visible).toBe(true);
  expect(result.withContributionLow.visible).toBe(true);
  expect(result.removed).toBe(true);
  expect(result.afterRemove.visible).toBe(false);
  expect(result.afterClear.visible).toBe(false);
});

test('compat facade manages custom canvas series lifecycle', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-custom-series-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#0f172a',
      'z-index:1000',
    ].join(';');
    document.body.appendChild(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      handleScroll: false,
      handleScale: false,
    });
    const start = 1_700_000_000_000;
    const data = Array.from({ length: 48 }, (_, index) => ({
      time: start + index * 60_000,
      open: 100 + index * 0.15,
      high: 102 + index * 0.15,
      low: 98 + index * 0.15,
      close: 101 + index * 0.15,
    }));
    chart.addSeries(module.CandlestickSeries).setData(data);
    chart.timeScale().fitContent();
    chart.raw().render();

    const events: any[] = [];
    const custom = chart.addCustomSeries({
      draw(context, params) {
        context.fillStyle = params.options.color || '#f59e0b';
        for (const point of params.data) {
          const projected = params.pointToCoordinate(point);
          if (!projected.visible) continue;
          context.fillRect(projected.x - 3, projected.y - 3, 6, 6);
        }
      },
    }, { color: '#f59e0b', layer: 'top' });
    custom.subscribeDataChanged(event => events.push(event));
    custom.setData([{ time: data[20].time, value: data[20].close + 3 }]);
    custom.update({ time: data[21].time, value: data[21].close + 4 });
    custom.applyOptions({ color: '#22c55e', zOrder: 12 });

    const canvas = host.querySelector('.aion_charts-custom-series') as HTMLCanvasElement | null;
    const projected = chart.raw().project_point(BigInt(data[21].time), data[21].close + 4);
    const context = canvas?.getContext('2d');
    const sample = context?.getImageData(Math.round(projected.x), Math.round(projected.y), 1, 1).data;
    const zIndex = canvas ? window.getComputedStyle(canvas).zIndex : null;
    const storedDataLength = custom.data().length;
    const options = custom.options();
    chart.removeSeries(custom);
    const removed = !host.querySelector('.aion_charts-custom-series');
    chart.remove();

    return {
      exportedConst: module.CustomSeries,
      events,
      alpha: sample?.[3] ?? 0,
      zIndex,
      storedDataLength,
      options,
      removed,
    };
  });

  expect(result.exportedConst).toBe('Custom');
  expect(result.events.map((event: any) => event.scope)).toEqual(['full', 'update']);
  expect(result.events.every((event: any) => event.seriesType === 'Custom')).toBe(true);
  expect(result.alpha).toBeGreaterThan(0);
  expect(result.zIndex).toBe('12');
  expect(result.storedDataLength).toBe(2);
  expect(result.options.color).toBe('#22c55e');
  expect(result.removed).toBe(true);
});

test('handleScroll.pressedMouseMove disables chart-pane drag panning', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-scroll-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: { pressedMouseMove: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.timeScale().fitContent();
    chart.raw().render();
    (window as any).__compatChart = chart;
    const rect = host.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top },
      before: chart.raw().visible_range(),
    };
  });

  await page.mouse.move(setup.rect.left + 300, setup.rect.top + 180);
  await page.mouse.down();
  await page.mouse.move(setup.rect.left + 80, setup.rect.top + 180, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(after[0]).toBeCloseTo(setup.before[0]);
  expect(after[1]).toBeCloseTo(setup.before[1]);
});

test('time scale logical ranges and overlay times stay consistent', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-logical-range-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: false,
      handleScale: false,
    });
    const start = 1_700_000_000_000;
    const main = chart.addSeries(module.LineSeries);
    const mainData = Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    }));
    main.setData(mainData);

    const overlay = chart.addSeries(module.HistogramSeries);
    overlay.setData(Array.from({ length: 20 }, (_, index) => ({
      time: start + (index + 80) * 60_000,
      value: index + 1,
      color: '#8ab4ff99',
    })));

    const scale = chart.timeScale();
    scale.setVisibleLogicalRange({ from: 12.5, to: 36.5 });
    chart.raw().render();
    const logicalRange = scale.getVisibleLogicalRange();
    const spacing = scale.barSpacing();
    const coordinate = scale.logicalToCoordinate(20);
    const roundTripLogical = coordinate == null ? null : scale.coordinateToLogical(coordinate);
    const time = scale.coordinateToTime(scale.logicalToCoordinate(85));
    const initialScrollPosition = scale.scrollPosition();
    scale.scrollToPosition(10);
    const scrolledRange = scale.getVisibleLogicalRange();
    const scrolledPosition = scale.scrollPosition();
    scale.scrollToRealTime();
    const realTimeRange = scale.getVisibleLogicalRange();
    const realTimeScrollPosition = scale.scrollPosition();

    return {
      logicalRange,
      spacing,
      roundTripLogical,
      time,
      expectedOverlayTime: start + 85 * 60_000,
      initialScrollPosition,
      scrolledRange,
      scrolledPosition,
      realTimeRange,
      realTimeScrollPosition,
    };
  });

  expect(result.logicalRange.from).toBeCloseTo(12.5);
  expect(result.logicalRange.to).toBeCloseTo(36.5);
  expect(result.spacing).toBeCloseTo(640 / 24);
  expect(result.roundTripLogical).toBeCloseTo(20);
  expect(result.time).toBe(result.expectedOverlayTime);
  expect(result.initialScrollPosition).toBeCloseTo(99 - 36.5);
  expect(result.scrolledRange.from).toBeCloseTo(65);
  expect(result.scrolledRange.to).toBeCloseTo(89);
  expect(result.scrolledPosition).toBeCloseTo(10);
  expect(result.realTimeRange.from).toBeCloseTo(75);
  expect(result.realTimeRange.to).toBeCloseTo(99);
  expect(result.realTimeScrollPosition).toBeCloseTo(0);
});

test('handleScale.axisDoubleClickReset disables double-click viewport reset', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-double-click-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScale: { axisDoubleClickReset: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();
    (window as any).__compatChart = chart;
    const rect = host.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  await page.mouse.dblclick(setup.rect.left + 300, setup.rect.top + 180);

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScale.axisDoubleClickReset).toBe(false);
  expect(after[0]).toBeCloseTo(setup.before[0]);
  expect(after[1]).toBeCloseTo(setup.before[1]);
});

test('handleScroll.horzTouchDrag disables horizontal touch panning', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-touch-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: { horzTouchDrag: false, vertTouchDrag: true },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    const pane = host.querySelector('#aion_charts-pane')!;
    const before = chart.raw().visible_range();
    const dispatch = (type: string, x: number, y: number) => pane.dispatchEvent(new PointerEvent(type, {
      bubbles: true,
      cancelable: true,
      clientX: x,
      clientY: y,
      pointerId: 7,
      pointerType: 'touch',
      isPrimary: true,
      button: 0,
      buttons: type === 'pointerup' ? 0 : 1,
    }));

    dispatch('pointerdown', 300, 180);
    dispatch('pointermove', 80, 180);
    dispatch('pointermove', 60, 180);
    dispatch('pointerup', 60, 180);

    return {
      before,
      after: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  expect(result.options.handleScroll.horzTouchDrag).toBe(false);
  expect(result.after[0]).toBeCloseTo(result.before[0]);
  expect(result.after[1]).toBeCloseTo(result.before[1]);
});

test('handleScroll.vertTouchDrag disables vertical touch price panning', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-vertical-touch-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: { horzTouchDrag: true, vertTouchDrag: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    const pane = host.querySelector('#aion_charts-pane')!;
    const priceAxis = host.querySelector('#aion_charts-price-axis')!;
    const priceRect = priceAxis.getBoundingClientRect();
    priceAxis.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: priceRect.left + Math.max(4, priceRect.width * 0.5),
      clientY: priceRect.top + priceRect.height * 0.5,
      deltaY: -360,
      deltaMode: 0,
    }));
    const before = JSON.parse(chart.raw().export_persistence_state(null)).viewport;
    const dispatch = (type: string, x: number, y: number) => pane.dispatchEvent(new PointerEvent(type, {
      bubbles: true,
      cancelable: true,
      clientX: x,
      clientY: y,
      pointerId: 8,
      pointerType: 'touch',
      isPrimary: true,
      button: 0,
      buttons: type === 'pointerup' ? 0 : 1,
    }));

    dispatch('pointerdown', 300, 220);
    dispatch('pointermove', 300, 80);
    dispatch('pointermove', 300, 60);
    dispatch('pointerup', 300, 60);

    return {
      before,
      after: JSON.parse(chart.raw().export_persistence_state(null)).viewport,
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  expect(result.options.handleScroll.vertTouchDrag).toBe(false);
  expect(result.after.priceMin).toBeCloseTo(result.before.priceMin);
  expect(result.after.priceMax).toBeCloseTo(result.before.priceMax);
});

test('vertical touch price panning remains enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-vertical-touch-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    const pane = host.querySelector('#aion_charts-pane')!;
    const priceAxis = host.querySelector('#aion_charts-price-axis')!;
    const priceRect = priceAxis.getBoundingClientRect();
    priceAxis.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: priceRect.left + Math.max(4, priceRect.width * 0.5),
      clientY: priceRect.top + priceRect.height * 0.5,
      deltaY: -360,
      deltaMode: 0,
    }));
    const before = JSON.parse(chart.raw().export_persistence_state(null)).viewport;
    const dispatch = (type: string, x: number, y: number) => pane.dispatchEvent(new PointerEvent(type, {
      bubbles: true,
      cancelable: true,
      clientX: x,
      clientY: y,
      pointerId: 8,
      pointerType: 'touch',
      isPrimary: true,
      button: 0,
      buttons: type === 'pointerup' ? 0 : 1,
    }));

    dispatch('pointerdown', 300, 220);
    dispatch('pointermove', 300, 80);
    dispatch('pointermove', 300, 60);
    dispatch('pointerup', 300, 60);

    return {
      before,
      after: JSON.parse(chart.raw().export_persistence_state(null)).viewport,
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  expect(result.options.handleScroll.vertTouchDrag).toBe(true);
  expect(result.after.priceMin).not.toBeCloseTo(result.before.priceMin);
  expect(result.after.priceMax).not.toBeCloseTo(result.before.priceMax);
});

test('mouse wheel scroll and scale options disable pane wheel changes', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-wheel-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: { mouseWheel: false },
      handleScale: { mouseWheel: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    const pane = host.querySelector('#aion_charts-pane')!;
    const rect = pane.getBoundingClientRect();
    const before = chart.raw().visible_range();
    pane.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: rect.left + 320,
      clientY: rect.top + 160,
      deltaX: 260,
      deltaY: -320,
      deltaMode: 0,
    }));

    return {
      before,
      after: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  expect(result.options.handleScroll.mouseWheel).toBe(false);
  expect(result.options.handleScale.mouseWheel).toBe(false);
  expect(result.after[0]).toBeCloseTo(result.before[0]);
  expect(result.after[1]).toBeCloseTo(result.before[1]);
});

test('handleScale.axisPressedMouseMove disables time-axis drag scaling', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-axis-drag-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScale: { axisPressedMouseMove: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-time-axis')!.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const y = setup.rect.top + Math.max(4, setup.rect.height / 2);
  await page.mouse.move(setup.rect.left + setup.rect.width * 0.65, y);
  await page.mouse.down();
  await page.mouse.move(setup.rect.left + setup.rect.width * 0.25, y, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScale.axisPressedMouseMove).toBe(false);
  expect(after[0]).toBeCloseTo(setup.before[0]);
  expect(after[1]).toBeCloseTo(setup.before[1]);
});

test('handleScale.axisPressedMouseMove disables price-axis drag scaling', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-price-axis-drag-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScale: { axisPressedMouseMove: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-price-axis')!.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      before: JSON.parse(chart.raw().export_persistence_state(null)).viewport,
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const x = setup.rect.left + Math.max(4, setup.rect.width / 2);
  await page.mouse.move(x, setup.rect.top + setup.rect.height * 0.35);
  await page.mouse.down();
  await page.mouse.move(x, setup.rect.top + setup.rect.height * 0.75, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => JSON.parse((window as any).__compatChart.raw().export_persistence_state(null)).viewport);
  expect(setup.options.handleScale.axisPressedMouseMove).toBe(false);
  expect(after.priceMin).toBeCloseTo(setup.before.priceMin);
  expect(after.priceMax).toBeCloseTo(setup.before.priceMax);
});

test('handleScale.mouseWheel disables time-axis and price-axis wheel scaling', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-axis-wheel-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScale: { mouseWheel: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    const beforeVisibleRange = chart.raw().visible_range();
    const beforeViewport = JSON.parse(chart.raw().export_persistence_state(null)).viewport;
    const timeAxis = host.querySelector('#aion_charts-time-axis')!;
    const priceAxis = host.querySelector('#aion_charts-price-axis')!;
    const timeRect = timeAxis.getBoundingClientRect();
    const priceRect = priceAxis.getBoundingClientRect();

    timeAxis.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: timeRect.left + timeRect.width * 0.5,
      clientY: timeRect.top + Math.max(4, timeRect.height * 0.5),
      deltaY: -360,
      deltaMode: 0,
    }));
    priceAxis.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: priceRect.left + Math.max(4, priceRect.width * 0.5),
      clientY: priceRect.top + priceRect.height * 0.5,
      deltaY: -360,
      deltaMode: 0,
    }));

    const afterViewport = JSON.parse(chart.raw().export_persistence_state(null)).viewport;
    return {
      beforeVisibleRange,
      afterVisibleRange: chart.raw().visible_range(),
      beforeViewport,
      afterViewport,
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  expect(result.options.handleScale.mouseWheel).toBe(false);
  expect(result.afterVisibleRange[0]).toBeCloseTo(result.beforeVisibleRange[0]);
  expect(result.afterVisibleRange[1]).toBeCloseTo(result.beforeVisibleRange[1]);
  expect(result.afterViewport.priceMin).toBeCloseTo(result.beforeViewport.priceMin);
  expect(result.afterViewport.priceMax).toBeCloseTo(result.beforeViewport.priceMax);
});

test('time-axis and price-axis wheel scaling remain enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const result = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-axis-wheel-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    const timeAxis = host.querySelector('#aion_charts-time-axis')!;
    const priceAxis = host.querySelector('#aion_charts-price-axis')!;
    const timeRect = timeAxis.getBoundingClientRect();
    const priceRect = priceAxis.getBoundingClientRect();
    const beforeTimeRange = chart.raw().visible_range();

    timeAxis.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: timeRect.left + timeRect.width * 0.5,
      clientY: timeRect.top + Math.max(4, timeRect.height * 0.5),
      deltaY: -360,
      deltaMode: 0,
    }));
    const afterTimeRange = chart.raw().visible_range();
    const beforePriceViewport = JSON.parse(chart.raw().export_persistence_state(null)).viewport;

    priceAxis.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: priceRect.left + Math.max(4, priceRect.width * 0.5),
      clientY: priceRect.top + priceRect.height * 0.5,
      deltaY: -360,
      deltaMode: 0,
    }));
    const afterPriceViewport = JSON.parse(chart.raw().export_persistence_state(null)).viewport;

    return {
      beforeTimeRange,
      afterTimeRange,
      beforePriceViewport,
      afterPriceViewport,
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  expect(result.options.handleScale.mouseWheel).toBe(true);
  expect(result.afterTimeRange[1] - result.afterTimeRange[0]).not.toBeCloseTo(
    result.beforeTimeRange[1] - result.beforeTimeRange[0],
  );
  expect(result.afterPriceViewport.priceMax - result.afterPriceViewport.priceMin).not.toBeCloseTo(
    result.beforePriceViewport.priceMax - result.beforePriceViewport.priceMin,
  );
});

test('price-axis drag scaling remains enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-price-axis-drag-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-price-axis')!.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      before: JSON.parse(chart.raw().export_persistence_state(null)).viewport,
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const x = setup.rect.left + Math.max(4, setup.rect.width / 2);
  await page.mouse.move(x, setup.rect.top + setup.rect.height * 0.35);
  await page.mouse.down();
  await page.mouse.move(x, setup.rect.top + setup.rect.height * 0.75, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => JSON.parse((window as any).__compatChart.raw().export_persistence_state(null)).viewport);
  expect(setup.options.handleScale.axisPressedMouseMove).toBe(true);
  expect(after.priceMax - after.priceMin).not.toBeCloseTo(setup.before.priceMax - setup.before.priceMin);
});

test('handleScale.pinch disables touch pinch zoom', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-pinch-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      handleScroll: false,
      handleScale: { pinch: false },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();
    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-pane')!.getBoundingClientRect();
    return {
      points: {
        y: rect.top + rect.height * 0.5,
        startLeft: rect.left + rect.width * 0.42,
        startRight: rect.left + rect.width * 0.58,
        endLeft: rect.left + rect.width * 0.25,
        endRight: rect.left + rect.width * 0.75,
      },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const client = await page.context().newCDPSession(page);
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchStart',
    touchPoints: [
      { x: setup.points.startLeft, y: setup.points.y, id: 1 },
      { x: setup.points.startRight, y: setup.points.y, id: 2 },
    ],
  });
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchMove',
    touchPoints: [
      { x: setup.points.endLeft, y: setup.points.y, id: 1 },
      { x: setup.points.endRight, y: setup.points.y, id: 2 },
    ],
  });
  await client.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScale.pinch).toBe(false);
  expect(after[0]).toBeCloseTo(setup.before[0]);
  expect(after[1]).toBeCloseTo(setup.before[1]);
});

test('touch pinch zoom remains enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-pinch-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();
    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-pane')!.getBoundingClientRect();
    return {
      points: {
        y: rect.top + rect.height * 0.5,
        startLeft: rect.left + rect.width * 0.42,
        startRight: rect.left + rect.width * 0.58,
        endLeft: rect.left + rect.width * 0.25,
        endRight: rect.left + rect.width * 0.75,
      },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const client = await page.context().newCDPSession(page);
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchStart',
    touchPoints: [
      { x: setup.points.startLeft, y: setup.points.y, id: 1 },
      { x: setup.points.startRight, y: setup.points.y, id: 2 },
    ],
  });
  await client.send('Input.dispatchTouchEvent', {
    type: 'touchMove',
    touchPoints: [
      { x: setup.points.endLeft, y: setup.points.y, id: 1 },
      { x: setup.points.endRight, y: setup.points.y, id: 2 },
    ],
  });
  await client.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScale.pinch).toBe(true);
  expect(after[1] - after[0]).not.toBeCloseTo(setup.before[1] - setup.before[0]);
});

test('trackingMode.exitMode onTouchEnd hides touch tracking crosshair on release', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-tracking-options-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
      trackingMode: { exitMode: 'onTouchEnd' },
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().render();
    (window as any).__compatChart = chart;
    (window as any).__compatPane = host.querySelector('#aion_charts-pane');
    const rect = (window as any).__compatPane.getBoundingClientRect();
    return {
      point: { x: rect.left + 300, y: rect.top + 180 },
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  await page.evaluate(point => {
    (window as any).__compatPane.dispatchEvent(new PointerEvent('pointerdown', {
      bubbles: true,
      cancelable: true,
      clientX: point.x,
      clientY: point.y,
      pointerId: 9,
      pointerType: 'touch',
      isPrimary: true,
      button: 0,
      buttons: 1,
    }));
  }, setup.point);
  await page.waitForTimeout(320);
  const during = await page.evaluate(() => (window as any).__compatChart.raw().crosshair_state()[0]);
  await page.evaluate(point => {
    (window as any).__compatPane.dispatchEvent(new PointerEvent('pointerup', {
      bubbles: true,
      cancelable: true,
      clientX: point.x,
      clientY: point.y,
      pointerId: 9,
      pointerType: 'touch',
      isPrimary: true,
      button: 0,
      buttons: 0,
    }));
  }, setup.point);
  const after = await page.evaluate(() => (window as any).__compatChart.raw().crosshair_state()[0]);

  expect(setup.options.trackingMode.exitMode).toBe('onTouchEnd');
  expect(during).toBe(1);
  expect(after).toBe(0);
});

test('chart-pane drag panning remains enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-scroll-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-pane')!.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  await page.mouse.move(setup.rect.left + 300, setup.rect.top + 180);
  await page.mouse.down();
  await page.mouse.move(setup.rect.left + 80, setup.rect.top + 180, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScroll.pressedMouseMove).toBe(true);
  expect(after[0]).not.toBeCloseTo(setup.before[0]);
  expect(after[1]).not.toBeCloseTo(setup.before[1]);
});

test('time-axis drag scaling remains enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-time-axis-drag-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-time-axis')!.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const y = setup.rect.top + Math.max(4, setup.rect.height / 2);
  await page.mouse.move(setup.rect.left + setup.rect.width * 0.65, y);
  await page.mouse.down();
  await page.mouse.move(setup.rect.left + setup.rect.width * 0.25, y, { steps: 8 });
  await page.mouse.up();

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScale.axisPressedMouseMove).toBe(true);
  expect(after[1] - after[0]).not.toBeCloseTo(setup.before[1] - setup.before[0]);
});

test('double-click viewport reset remains enabled by default', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-double-click-enabled-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().set_visible_range(20, 40);
    chart.raw().render();

    (window as any).__compatChart = chart;
    const rect = host.querySelector('#aion_charts-pane')!.getBoundingClientRect();
    return {
      rect: { left: rect.left, top: rect.top },
      before: chart.raw().visible_range(),
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  await page.mouse.dblclick(setup.rect.left + 300, setup.rect.top + 180);

  const after = await page.evaluate(() => (window as any).__compatChart.raw().visible_range());
  expect(setup.options.handleScale.axisDoubleClickReset).toBe(true);
  expect(after[0]).not.toBeCloseTo(setup.before[0]);
  expect(after[1]).not.toBeCloseTo(setup.before[1]);
});

test('trackingMode.exitMode onNextTap keeps touch tracking crosshair until the next tap', async ({ page }) => {
  await page.goto('/demo/index.html?renderer=canvas2d');
  await page.waitForLoadState('domcontentloaded');
  await expect(page.locator('#error-banner')).toBeHidden();

  const setup = await page.evaluate(async () => {
    const module = await import('/wasm/aion_charts-compat.js');
    const host = document.createElement('div');
    host.id = 'compat-tracking-default-harness';
    host.style.cssText = [
      'position:fixed',
      'left:0',
      'top:0',
      'width:640px',
      'height:360px',
      'background:#131315',
      'z-index:2147483647',
    ].join(';');
    document.body.replaceChildren(host);

    const chart = await module.createChart(host, {
      renderer: 'canvas2d',
      autoRender: false,
      theme: 'dark',
    });
    const series = chart.addSeries(module.LineSeries);
    const start = 1_700_000_000_000;
    series.setData(Array.from({ length: 80 }, (_, index) => ({
      time: start + index * 60_000,
      value: 100 + index,
    })));
    chart.raw().render();
    (window as any).__compatChart = chart;
    (window as any).__compatPane = host.querySelector('#aion_charts-pane');
    const rect = (window as any).__compatPane.getBoundingClientRect();
    return {
      point: { x: rect.left + 300, y: rect.top + 180 },
      options: JSON.parse(chart.raw().export_persistence_state(null)).options,
    };
  });

  const dispatchTouch = async (type: string, pointerId: number) => {
    await page.evaluate(({ point, type, pointerId }) => {
      (window as any).__compatPane.dispatchEvent(new PointerEvent(type, {
        bubbles: true,
        cancelable: true,
        clientX: point.x,
        clientY: point.y,
        pointerId,
        pointerType: 'touch',
        isPrimary: true,
        button: 0,
        buttons: type === 'pointerup' ? 0 : 1,
      }));
    }, { point: setup.point, type, pointerId });
  };

  await dispatchTouch('pointerdown', 10);
  await page.waitForTimeout(320);
  await dispatchTouch('pointerup', 10);
  const afterRelease = await page.evaluate(() => (window as any).__compatChart.raw().crosshair_state()[0]);
  await dispatchTouch('pointerdown', 11);
  await dispatchTouch('pointerup', 11);
  const afterNextTap = await page.evaluate(() => (window as any).__compatChart.raw().crosshair_state()[0]);

  expect(setup.options.trackingMode.exitMode).toBe('onNextTap');
  expect(afterRelease).toBe(1);
  expect(afterNextTap).toBe(0);
});

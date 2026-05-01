import init, { AxiusCharts } from './pkg/axiuscharts_wasm.js';

let initPromise = null;

export const LineSeries = 'Line';
export const AreaSeries = 'Area';
export const BarSeries = 'Bar';
export const CandlestickSeries = 'Candlestick';
export const HistogramSeries = 'Histogram';
export const BaselineSeries = 'Baseline';
export const CustomSeries = 'Custom';

export function initAxiusCharts(initOptions) {
  if (!initPromise) {
    initPromise = init(initOptions);
  }
  return initPromise;
}

export async function createChart(container, options = {}) {
  await initAxiusCharts(options.wasm);
  const chart = await AxiusCharts.create_chart(container, options);
  return new ChartApi(chart, container);
}

function rgba(input, fallback = [0.161, 0.384, 1, 1]) {
  if (Array.isArray(input) && input.length >= 3) {
    return [
      Number(input[0]) || 0,
      Number(input[1]) || 0,
      Number(input[2]) || 0,
      input.length >= 4 ? Number(input[3]) || 0 : 1,
    ];
  }
  if (typeof input === 'string') {
    const hex = input.trim();
    const match = /^#?([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex);
    if (match) {
      const raw = match[1];
      const alpha = match[2];
      return [
        parseInt(raw.slice(0, 2), 16) / 255,
        parseInt(raw.slice(2, 4), 16) / 255,
        parseInt(raw.slice(4, 6), 16) / 255,
        alpha ? parseInt(alpha, 16) / 255 : 1,
      ];
    }
  }
  return fallback;
}

function lineStyle(style) {
  if (typeof style !== 'number') return style || 'solid';
  return ['solid', 'dotted', 'dashed', 'large_dashed', 'sparse_dotted'][style] || 'solid';
}

function markerShape(shape) {
  if (shape === 'arrowUp') return 'arrow_up';
  if (shape === 'arrowDown') return 'arrow_down';
  return shape || 'circle';
}

function markerPosition(position) {
  if (position === 'aboveBar') return 'above_bar';
  if (position === 'belowBar') return 'below_bar';
  if (position === 'inBar') return 'in_bar';
  if (position === 'atPrice') return 'at_price';
  return position || 'above_bar';
}

function markerZOrder(zOrder) {
  if (zOrder === 'above_series') return 'aboveSeries';
  return zOrder || 'normal';
}

function overlayZIndex(options = {}) {
  const value = options.zOrder ?? options.layer ?? 'normal';
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  switch (String(value)) {
    case 'background':
    case 'belowSeries':
    case 'below-series':
      return '1';
    case 'top':
    case 'topOverlay':
    case 'top-overlay':
      return '20';
    case 'normal':
    case 'seriesOverlay':
    case 'series-overlay':
    default:
      return '5';
  }
}

function toTimestamp(value) {
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number') {
    return BigInt(Math.trunc(value));
  }
  if (typeof value === 'string') {
    return BigInt(Date.parse(value));
  }
  if (value && typeof value === 'object') {
    if ('timestamp' in value) return toTimestamp(value.timestamp);
    const date = Date.UTC(value.year, (value.month || 1) - 1, value.day || 1);
    return BigInt(date);
  }
  return 0n;
}

function dataArrays(data, valueKey = 'value') {
  const values = new Float64Array(data.length);
  const timestamps = new BigUint64Array(data.length);
  data.forEach((point, index) => {
    timestamps[index] = toTimestamp(point.time);
    values[index] = Number(point[valueKey]);
  });
  return { values, timestamps };
}

function histogramArrays(data) {
  const values = new Float64Array(data.length);
  const timestamps = new BigUint64Array(data.length);
  const colorsR = new Float32Array(data.length);
  const colorsG = new Float32Array(data.length);
  const colorsB = new Float32Array(data.length);
  const colorsA = new Float32Array(data.length);
  data.forEach((point, index) => {
    const color = rgba(point.color);
    timestamps[index] = toTimestamp(point.time);
    values[index] = Number(point.value);
    colorsR[index] = color[0];
    colorsG[index] = color[1];
    colorsB[index] = color[2];
    colorsA[index] = color[3];
  });
  return { values, timestamps, colorsR, colorsG, colorsB, colorsA };
}

function ohlcArrays(data) {
  const open = new Float64Array(data.length);
  const high = new Float64Array(data.length);
  const low = new Float64Array(data.length);
  const close = new Float64Array(data.length);
  const volume = new Float64Array(data.length);
  const timestamps = new BigUint64Array(data.length);
  data.forEach((point, index) => {
    timestamps[index] = toTimestamp(point.time);
    open[index] = Number(point.open);
    high[index] = Number(point.high);
    low[index] = Number(point.low);
    close[index] = Number(point.close);
    volume[index] = Number(point.volume || 0);
  });
  return { open, high, low, close, volume, timestamps };
}

class ChartApi {
  constructor(raw, container) {
    this._raw = raw;
    this._container = typeof container === 'string' ? document.getElementById(container) : container;
    this._times = [];
    this._dblClickHandlers = new Map();
    this._watermark = null;
    this._watermarkOptions = { visible: false };
    this._customSeries = new Set();
  }

  raw() {
    return this._raw;
  }

  remove() {
    this.clearWatermark();
    for (const series of Array.from(this._customSeries)) {
      series.detach();
    }
    this._raw.dispose();
  }

  resize(width, height) {
    if (this._container) {
      this._container.style.width = `${width}px`;
      this._container.style.height = `${height}px`;
    }
    this._raw.render();
    this._renderCustomSeries();
  }

  applyOptions(options) {
    this._raw.apply_options(options || {});
    this._renderCustomSeries();
  }

  addSeries(seriesType, options = {}) {
    return new SeriesApi(this, addSeries(this._raw, seriesType, options), seriesType);
  }

  addCustomSeries(renderer, options = {}) {
    const series = new CustomSeriesApi(this, renderer, options);
    this._customSeries.add(series);
    series.render();
    return series;
  }

  removeSeries(series) {
    if (series instanceof CustomSeriesApi) {
      series.detach();
      this._customSeries.delete(series);
      return;
    }
    if (series._id === 0) return;
    this._raw.remove_series(series._id);
  }

  _renderCustomSeries() {
    for (const series of this._customSeries) {
      series.render();
    }
  }

  timeScale() {
    return new TimeScaleApi(this);
  }

  priceScale(id = 'right') {
    return new PriceScaleApi(this, id);
  }

  applyMarkerOptions(options = {}) {
    if ('zOrder' in options) {
      this._raw.set_marker_z_order(markerZOrder(options.zOrder));
    }
    if ('autoScale' in options) {
      this._raw.set_marker_auto_scale(!!options.autoScale);
    }
  }

  markerOptions() {
    return {
      zOrder: this._raw.marker_z_order(),
      autoScale: this._raw.marker_auto_scale(),
    };
  }

  applyWatermarkOptions(options = {}) {
    const nextOptions = { ...this._watermarkOptions, ...options };
    if ('layer' in options && !('zOrder' in options)) {
      delete nextOptions.zOrder;
    }
    if ('zOrder' in options && !('layer' in options)) {
      delete nextOptions.layer;
    }
    this._watermarkOptions = nextOptions;
    const visible = this._watermarkOptions.visible !== false
      && (this._watermarkOptions.text || this._watermarkOptions.imageUrl);
    if (!visible) {
      this.clearWatermark();
      return;
    }
    const host = this._container;
    if (!host) return;
    const computedPosition = window.getComputedStyle(host).position;
    if (!computedPosition || computedPosition === 'static') {
      host.style.position = 'relative';
    }

    let watermark = this._watermark;
    if (!watermark) {
      watermark = document.createElement('div');
      watermark.className = 'axiuscharts-watermark';
      watermark.style.cssText = [
        'position:absolute',
        'pointer-events:none',
        'user-select:none',
        'z-index:5',
        'display:flex',
        'align-items:center',
        'justify-content:center',
        'text-align:center',
        'box-sizing:border-box',
        'white-space:pre-line',
      ].join(';');
      host.appendChild(watermark);
      this._watermark = watermark;
    }

    const horzAlign = this._watermarkOptions.horzAlign || 'center';
    const vertAlign = this._watermarkOptions.vertAlign || 'center';
    const paddingX = Number(this._watermarkOptions.paddingX ?? 0);
    const paddingY = Number(this._watermarkOptions.paddingY ?? 0);
    watermark.style.left = `${paddingX}px`;
    watermark.style.right = `${paddingX}px`;
    watermark.style.top = `${paddingY}px`;
    watermark.style.bottom = `${paddingY}px`;
    watermark.style.justifyContent = horzAlign === 'left' ? 'flex-start' : horzAlign === 'right' ? 'flex-end' : 'center';
    watermark.style.alignItems = vertAlign === 'top' ? 'flex-start' : vertAlign === 'bottom' ? 'flex-end' : 'center';
    watermark.style.opacity = String(Number(this._watermarkOptions.opacity ?? 1));
    watermark.style.zIndex = overlayZIndex(this._watermarkOptions);
    watermark.style.color = this._watermarkOptions.color || 'rgba(120, 130, 150, 0.24)';
    watermark.style.font = `${Number(this._watermarkOptions.fontSize || 48)}px ${this._watermarkOptions.fontFamily || 'sans-serif'}`;
    watermark.style.fontWeight = String(this._watermarkOptions.fontWeight || 600);

    watermark.replaceChildren();
    if (this._watermarkOptions.imageUrl) {
      const image = document.createElement('img');
      image.alt = this._watermarkOptions.alt || '';
      image.src = this._watermarkOptions.imageUrl;
      image.style.maxWidth = this._watermarkOptions.width ? `${Number(this._watermarkOptions.width)}px` : '70%';
      image.style.maxHeight = this._watermarkOptions.height ? `${Number(this._watermarkOptions.height)}px` : '70%';
      image.style.objectFit = this._watermarkOptions.objectFit || 'contain';
      image.style.opacity = String(Number(this._watermarkOptions.imageOpacity ?? 1));
      watermark.appendChild(image);
    } else {
      watermark.textContent = String(this._watermarkOptions.text || '');
    }
  }

  watermarkOptions() {
    return { ...this._watermarkOptions };
  }

  clearWatermark() {
    if (this._watermark) {
      this._watermark.remove();
      this._watermark = null;
    }
    this._watermarkOptions = { ...this._watermarkOptions, visible: false };
  }

  addAutoscaleContribution(options = {}) {
    return this._raw.add_autoscale_contribution(
      Number(options.minPrice ?? options.priceMin),
      Number(options.maxPrice ?? options.priceMax),
    );
  }

  removeAutoscaleContribution(id) {
    return this._raw.remove_autoscale_contribution(Number(id));
  }

  clearAutoscaleContributions() {
    this._raw.clear_autoscale_contributions();
  }

  subscribeClick(handler) {
    this._raw.on('click', handler);
  }

  unsubscribeClick(handler) {
    this._raw.off('click', handler);
  }

  subscribeCrosshairMove(handler) {
    this._raw.on('crosshairMove', handler);
  }

  unsubscribeCrosshairMove(handler) {
    this._raw.off('crosshairMove', handler);
  }

  subscribeDblClick(handler) {
    if (!this._container || this._dblClickHandlers.has(handler)) return;
    const wrapped = event => {
      const rect = this._container.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;
      const logical = this.timeScale().coordinateToLogical(x);
      handler({
        type: 'dblclick',
        x,
        y,
        point: { x, y },
        logical,
        time: logical == null ? null : this.timeScale().coordinateToTime(x),
        sourceEvent: event,
      });
    };
    this._dblClickHandlers.set(handler, wrapped);
    this._container.addEventListener('dblclick', wrapped);
  }

  unsubscribeDblClick(handler) {
    const wrapped = this._dblClickHandlers.get(handler);
    if (!this._container || !wrapped) return;
    this._container.removeEventListener('dblclick', wrapped);
    this._dblClickHandlers.delete(handler);
  }

  _replaceTimes(data) {
    this._times = normalizedTimes(data);
  }

  _mergeTimes(data) {
    const merged = new Set(this._times);
    for (const time of normalizedTimes(data)) {
      merged.add(time);
    }
    this._times = Array.from(merged).sort((a, b) => a - b);
  }

  _upsertTime(time) {
    const numericTimestamp = Number(time);
    if (!Number.isFinite(numericTimestamp)) return;
    const index = this._times.findIndex(existing => existing >= numericTimestamp);
    if (index === -1) {
      this._times.push(numericTimestamp);
    } else if (this._times[index] !== numericTimestamp) {
      this._times.splice(index, 0, numericTimestamp);
    }
  }
}

function normalizedTimes(data) {
  return Array.from(new Set(data.map(point => Number(toTimestamp(point.time)))))
    .filter(Number.isFinite)
    .sort((a, b) => a - b);
}

function addSeries(raw, seriesType, options) {
  const type = seriesType?.type || seriesType;
  if (type === LineSeries || type === 'line') {
    const color = rgba(options.color);
    return raw.add_line_series(color[0], color[1], color[2], color[3], options.lineWidth || 2, lineStyle(options.lineStyle));
  }
  if (type === AreaSeries || type === 'area') {
    const line = rgba(options.lineColor || options.color);
    const top = rgba(options.topColor, [line[0], line[1], line[2], 0.35]);
    const bottom = rgba(options.bottomColor, [line[0], line[1], line[2], 0.05]);
    return raw.add_area_series(
      line[0], line[1], line[2], line[3],
      top[0], top[1], top[2], top[3],
      bottom[0], bottom[1], bottom[2], bottom[3],
      options.lineWidth || 2,
    );
  }
  if (type === HistogramSeries || type === 'histogram') {
    const color = rgba(options.color);
    return raw.add_histogram_series(color[0], color[1], color[2], color[3], Number(options.base || 0));
  }
  if (type === BarSeries || type === 'bar') {
    const up = rgba(options.upColor || '#26a69a');
    const down = rgba(options.downColor || '#ef5350');
    return raw.add_bar_series(up[0], up[1], up[2], up[3], down[0], down[1], down[2], down[3], options.openVisible !== false, !!options.thinBars);
  }
  if (type === BaselineSeries || type === 'baseline') {
    const top = rgba(options.topLineColor || options.color || '#26a69a');
    const bottom = rgba(options.bottomLineColor || '#ef5350');
    const topFill = rgba(options.topFillColor1 || top, [top[0], top[1], top[2], 0.35]);
    const bottomFill = rgba(options.bottomFillColor1 || bottom, [bottom[0], bottom[1], bottom[2], 0.35]);
    return raw.add_baseline_series(
      Number(options.baseValue?.price ?? options.baseValue ?? 0),
      top[0], top[1], top[2], top[3],
      bottom[0], bottom[1], bottom[2], bottom[3],
      topFill[0], topFill[1], topFill[2], topFill[3],
      0, 0, 0, 0,
      bottomFill[0], bottomFill[1], bottomFill[2], bottomFill[3],
      0, 0, 0, 0,
      options.lineWidth || 2,
    );
  }
  if (type === CandlestickSeries || type === 'candlestick') {
    raw.set_chart_type('candlestick');
    return 0;
  }
  throw new Error(`Unsupported series type: ${String(type)}`);
}

class SeriesApi {
  constructor(chart, id, seriesType) {
    this._chart = chart;
    this._raw = chart._raw;
    this._id = id;
    this._seriesType = seriesType?.type || seriesType;
    this._dataChangedHandlers = new Set();
  }

  setData(data) {
    if (this._id === 0 || this._seriesType === CandlestickSeries || this._seriesType === 'candlestick') {
      this._chart._replaceTimes(data);
      const arrays = ohlcArrays(data);
      this._raw.set_data_arrays(arrays.open, arrays.high, arrays.low, arrays.close, arrays.volume, arrays.timestamps);
      this._emitDataChanged('full');
      return;
    }
    this._chart._mergeTimes(data);
    if (this._seriesType === BarSeries || this._seriesType === 'bar') {
      const arrays = ohlcArrays(data);
      this._raw.set_bar_series_data(this._id, arrays.timestamps, arrays.open, arrays.high, arrays.low, arrays.close);
      this._emitDataChanged('full');
      return;
    }
    if (this._seriesType === HistogramSeries || this._seriesType === 'histogram') {
      const arrays = histogramArrays(data);
      this._raw.set_histogram_data(
        this._id,
        arrays.values,
        arrays.timestamps,
        arrays.colorsR,
        arrays.colorsG,
        arrays.colorsB,
        arrays.colorsA,
      );
      this._emitDataChanged('full');
      return;
    }
    const arrays = dataArrays(data);
    this._raw.set_series_data(this._id, arrays.values, arrays.timestamps);
    this._emitDataChanged('full');
  }

  update(point) {
    const timestamp = toTimestamp(point.time);
    this._chart._upsertTime(timestamp);
    if (this._id === 0 || this._seriesType === CandlestickSeries || this._seriesType === 'candlestick') {
      this._raw.upsert_bar(timestamp, Number(point.open), Number(point.high), Number(point.low), Number(point.close), Number(point.volume || 0));
      this._emitDataChanged('update');
      return;
    }
    if (this._seriesType === BarSeries || this._seriesType === 'bar') {
      this._raw.upsert_bar_series_point(this._id, timestamp, Number(point.open), Number(point.high), Number(point.low), Number(point.close));
      this._emitDataChanged('update');
      return;
    }
    if (this._seriesType === HistogramSeries || this._seriesType === 'histogram') {
      const color = rgba(point.color);
      this._raw.upsert_histogram_point(this._id, timestamp, Number(point.value), color[0], color[1], color[2], color[3]);
      this._emitDataChanged('update');
      return;
    }
    this._raw.upsert_series_point(this._id, timestamp, Number(point.value));
    this._emitDataChanged('update');
  }

  applyOptions(options = {}) {
    if ('visible' in options && this._id !== 0) {
      this._raw.set_series_visible(this._id, !!options.visible);
    }
  }

  createPriceLine(options = {}) {
    const color = rgba(options.color, [0.161, 0.384, 1, 1]);
    return this._raw.create_price_line(Number(options.price), color[0], color[1], color[2], color[3], options.lineWidth || 1, lineStyle(options.lineStyle), !!options.draggable);
  }

  createMarker(options = {}) {
    const color = rgba(options.color, [0.161, 0.384, 1, 1]);
    return this._raw.add_marker_at_time(
      this._id,
      toTimestamp(options.time),
      markerShape(options.shape),
      markerPosition(options.position),
      Number(options.price || 0),
      color[0],
      color[1],
      color[2],
      color[3],
      Number(options.size || 8),
      options.text == null ? '' : String(options.text),
    );
  }

  setMarkers(markers = []) {
    this._raw.clear_markers(this._id);
    for (const marker of markers) {
      this.createMarker(marker);
    }
  }

  removeMarker(markerId) {
    return this._raw.remove_marker(this._id, Number(markerId));
  }

  clearMarkers() {
    this._raw.clear_markers(this._id);
  }

  priceScale() {
    return this._chart.priceScale();
  }

  subscribeDataChanged(handler) {
    this._dataChangedHandlers.add(handler);
  }

  unsubscribeDataChanged(handler) {
    this._dataChangedHandlers.delete(handler);
  }

  _emitDataChanged(scope) {
    const event = {
      type: 'dataChanged',
      scope,
      seriesType: this._seriesType,
      seriesId: this._id,
    };
    for (const handler of this._dataChangedHandlers) {
      handler(event);
    }
  }
}

class CustomSeriesApi {
  constructor(chart, renderer, options = {}) {
    if (!renderer || (typeof renderer !== 'function' && typeof renderer.draw !== 'function')) {
      throw new Error('custom series renderer must be a function or expose draw(context, params)');
    }
    this._chart = chart;
    this._renderer = renderer;
    this._options = { ...options };
    this._data = [];
    this._dataChangedHandlers = new Set();
    this._canvas = document.createElement('canvas');
    this._canvas.className = 'axiuscharts-custom-series';
    this._canvas.style.cssText = [
      'position:absolute',
      'left:0',
      'top:0',
      'width:100%',
      'height:100%',
      'pointer-events:none',
      `z-index:${overlayZIndex(options)}`,
    ].join(';');
    const host = chart._container;
    if (host) {
      const computedPosition = window.getComputedStyle(host).position;
      if (!computedPosition || computedPosition === 'static') {
        host.style.position = 'relative';
      }
      host.appendChild(this._canvas);
    }
  }

  setData(data) {
    this._data = Array.isArray(data) ? data.slice() : [];
    this._chart._mergeTimes(this._data);
    this.render();
    this._emitDataChanged('full');
  }

  update(point) {
    if (!point) return;
    const timestamp = Number(toTimestamp(point.time));
    const last = this._data[this._data.length - 1];
    if (last && Number(toTimestamp(last.time)) === timestamp) {
      this._data[this._data.length - 1] = point;
    } else {
      this._data.push(point);
    }
    this._chart._mergeTimes([point]);
    this.render();
    this._emitDataChanged('update');
  }

  applyOptions(options = {}) {
    this._options = { ...this._options, ...options };
    if ('layer' in options && !('zOrder' in options)) {
      delete this._options.zOrder;
    }
    if ('zOrder' in options && !('layer' in options)) {
      delete this._options.layer;
    }
    this._canvas.style.zIndex = overlayZIndex(this._options);
    this.render();
  }

  options() {
    return { ...this._options };
  }

  data() {
    return this._data.slice();
  }

  priceScale() {
    return this._chart.priceScale();
  }

  createPriceLine() {
    return 0;
  }

  createMarker() {
    return 0;
  }

  setMarkers() {}

  removeMarker() {
    return false;
  }

  clearMarkers() {}

  subscribeDataChanged(handler) {
    this._dataChangedHandlers.add(handler);
  }

  unsubscribeDataChanged(handler) {
    this._dataChangedHandlers.delete(handler);
  }

  render() {
    if (!this._canvas.isConnected) return;
    const dpr = window.devicePixelRatio || 1;
    const width = this._canvas.clientWidth || this._chart._container?.clientWidth || 0;
    const height = this._canvas.clientHeight || this._chart._container?.clientHeight || 0;
    this._canvas.width = Math.max(1, Math.round(width * dpr));
    this._canvas.height = Math.max(1, Math.round(height * dpr));
    const context = this._canvas.getContext('2d');
    if (!context) return;
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);
    const params = {
      data: this._data.slice(),
      options: { ...this._options },
      width,
      height,
      devicePixelRatio: dpr,
      timeToCoordinate: time => this._chart.timeScale().timeToCoordinate(time),
      priceToCoordinate: (time, price) => this._chart.raw().project_point(toTimestamp(time), Number(price)).y,
      pointToCoordinate: point => {
        const price = Number(point.value ?? point.close ?? point.price ?? 0);
        return this._chart.raw().project_point(toTimestamp(point.time), price);
      },
    };
    if (typeof this._renderer === 'function') {
      this._renderer(context, params);
    } else {
      this._renderer.draw(context, params);
    }
  }

  detach() {
    this._canvas.remove();
    this._dataChangedHandlers.clear();
  }

  _emitDataChanged(scope) {
    const event = {
      type: 'dataChanged',
      scope,
      seriesType: CustomSeries,
      seriesId: null,
    };
    for (const handler of this._dataChangedHandlers) {
      handler(event);
    }
  }
}

class TimeScaleApi {
  constructor(chart) {
    this._chart = chart;
    this._raw = chart._raw;
  }

  fitContent() {
    this._raw.reset_viewport('fit_all');
  }

  setVisibleRange(range) {
    this._raw.zoom_to_range(toTimestamp(range.from), toTimestamp(range.to));
  }

  setVisibleLogicalRange(range) {
    this._raw.set_visible_range(Number(range.from), Number(range.to));
  }

  getVisibleRange() {
    const range = this._raw.visible_range();
    if (!range || range.length < 2) return null;
    if (this._chart._times.length > 0) {
      return {
        from: this._chart._times[Math.max(0, Math.min(this._chart._times.length - 1, Math.floor(range[0])))],
        to: this._chart._times[Math.max(0, Math.min(this._chart._times.length - 1, Math.floor(range[1])))],
      };
    }
    return {
      from: Number(this._raw.bar_index_to_timestamp(Math.max(0, Math.floor(range[0])))),
      to: Number(this._raw.bar_index_to_timestamp(Math.max(0, Math.floor(range[1])))),
    };
  }

  getVisibleLogicalRange() {
    const range = this._raw.visible_range();
    if (!range || range.length < 2) return null;
    return {
      from: Number(range[0]),
      to: Number(range[1]),
    };
  }

  timeToCoordinate(time) {
    const point = this._raw.project_point(toTimestamp(time), 0);
    return point.visible || Number.isFinite(point.x) ? point.x : null;
  }

  coordinateToTime(x) {
    const logical = this.coordinateToLogical(x);
    if (logical == null) return null;
    if (this._chart._times.length > 0) {
      return this._chart._times[Math.max(0, Math.min(this._chart._times.length - 1, Math.floor(logical)))];
    }
    const ts = this._raw.bar_index_to_timestamp(Math.max(0, Math.floor(logical)));
    return ts ? Number(ts) : null;
  }

  logicalToCoordinate(logical) {
    const range = this._raw.visible_range();
    const width = this._chart._container?.clientWidth || 0;
    if (!range || range.length < 2 || width <= 0) return null;
    return ((logical - range[0]) / (range[1] - range[0])) * width - 1;
  }

  coordinateToLogical(x) {
    const range = this._raw.visible_range();
    const width = this._chart._container?.clientWidth || 0;
    if (!range || range.length < 2 || width <= 0) return null;
    return range[0] + ((x + 1) / width) * (range[1] - range[0]);
  }

  barSpacing() {
    const range = this._raw.visible_range();
    const width = this._chart._container?.clientWidth || 0;
    if (!range || range.length < 2 || width <= 0 || range[1] <= range[0]) return 0;
    return width / (range[1] - range[0]);
  }

  scrollPosition() {
    const range = this._raw.visible_range();
    const lastLogical = this._chart._times.length - 1;
    if (!range || range.length < 2 || lastLogical < 0) return 0;
    return Math.max(0, lastLogical - Number(range[1]));
  }

  scrollToPosition(position, _animated = false) {
    const range = this._raw.visible_range();
    const lastLogical = this._chart._times.length - 1;
    if (!range || range.length < 2 || lastLogical < 0) return;
    const span = Number(range[1]) - Number(range[0]);
    const end = lastLogical - Number(position || 0);
    this._raw.set_visible_range(end - span, end);
  }

  scrollToRealTime() {
    this.scrollToPosition(0, false);
  }
}

class PriceScaleApi {
  constructor(chart, id) {
    this._chart = chart;
    this._id = id;
  }

  applyOptions(options = {}) {
    if (options.mode != null) {
      this._chart._raw.set_price_scale_mode(priceScaleMode(options.mode));
    }
    const margins = options.scaleMargins || options.margins;
    if (margins) {
      this._chart._raw.set_price_scale_margins(
        Number(margins.top ?? 0.2),
        Number(margins.bottom ?? 0.1),
      );
    }
  }

  options() {
    const snapshot = readPersistence(this._chart._raw);
    const priceScale = snapshot?.options?.priceScale || {};
    return {
      mode: priceScaleMode(priceScale.mode || 'normal'),
      scaleMargins: {
        top: Number(priceScale.margins?.top ?? 0.2),
        bottom: Number(priceScale.margins?.bottom ?? 0.1),
      },
      ticksVisible: priceScale.ticksVisible !== false,
      tickDensity: Number(priceScale.tickDensity ?? 1),
    };
  }

  width() {
    return 34;
  }
}

function priceScaleMode(mode) {
  if (typeof mode === 'number') {
    return ['normal', 'logarithmic', 'percentage', 'indexed_to_100'][mode] || 'normal';
  }
  const key = String(mode || 'normal');
  if (key === 'indexedTo100' || key === 'indexed') return 'indexed_to_100';
  if (key === 'percent') return 'percentage';
  if (key === 'log') return 'logarithmic';
  return key;
}

function readPersistence(raw) {
  try {
    if (typeof raw.export_persistence_state !== 'function') return null;
    return JSON.parse(raw.export_persistence_state(null));
  } catch (_err) {
    return null;
  }
}

export default {
  createChart,
  initAxiusCharts,
  LineSeries,
  AreaSeries,
  BarSeries,
  CandlestickSeries,
  HistogramSeries,
  BaselineSeries,
  CustomSeries,
};

export type Time = number | bigint | string | { year: number; month: number; day: number };

export type SeriesType =
  | typeof LineSeries
  | typeof AreaSeries
  | typeof BarSeries
  | typeof CandlestickSeries
  | typeof HistogramSeries
  | typeof BaselineSeries
  | typeof CustomSeries
  | string;

export interface ChartOptions {
  wasm?: unknown;
  [key: string]: unknown;
}

export interface LineData {
  time: Time;
  value: number;
}

export interface OhlcData {
  time: Time;
  open: number;
  high: number;
  low: number;
  close: number;
  volume?: number;
}

export interface HistogramData extends LineData {
  color?: string | [number, number, number, number?];
}

export interface LogicalRange {
  from: number;
  to: number;
}

export interface TimeRange {
  from: Time;
  to: Time;
}

export type MarkerShape = 'arrowUp' | 'arrowDown' | 'circle' | 'square' | 'arrow_up' | 'arrow_down';
export type MarkerPosition = 'aboveBar' | 'belowBar' | 'inBar' | 'atPrice' | 'above_bar' | 'below_bar' | 'in_bar' | 'at_price';
export type MarkerZOrder = 'normal' | 'aboveSeries' | 'top' | 'above_series';
export type OverlayLayer = 'background' | 'belowSeries' | 'below-series' | 'normal' | 'seriesOverlay' | 'series-overlay' | 'top' | 'topOverlay' | 'top-overlay';

export interface SeriesMarkerOptions {
  time: Time;
  shape?: MarkerShape;
  position?: MarkerPosition;
  price?: number;
  color?: string | [number, number, number, number?];
  size?: number;
  text?: string;
}

export interface MarkerOptions {
  zOrder?: MarkerZOrder;
  autoScale?: boolean;
}

export interface WatermarkOptions {
  visible?: boolean;
  text?: string;
  imageUrl?: string;
  alt?: string;
  color?: string;
  opacity?: number;
  imageOpacity?: number;
  fontSize?: number;
  fontFamily?: string;
  fontWeight?: string | number;
  horzAlign?: 'left' | 'center' | 'right';
  vertAlign?: 'top' | 'center' | 'bottom';
  paddingX?: number;
  paddingY?: number;
  width?: number;
  height?: number;
  objectFit?: 'contain' | 'cover' | 'fill' | 'none' | 'scale-down';
  zOrder?: OverlayLayer | number;
  layer?: OverlayLayer | number;
}

export interface AutoscaleContributionOptions {
  minPrice?: number;
  maxPrice?: number;
  priceMin?: number;
  priceMax?: number;
}

export interface CustomSeriesRenderParams<TData = unknown> {
  data: TData[];
  options: Record<string, unknown>;
  width: number;
  height: number;
  devicePixelRatio: number;
  timeToCoordinate(time: Time): number | null;
  priceToCoordinate(time: Time, price: number): number;
  pointToCoordinate(point: TData): { x: number; y: number; visible: boolean };
}

export type CustomSeriesRenderer<TData = unknown> =
  | ((context: CanvasRenderingContext2D, params: CustomSeriesRenderParams<TData>) => void)
  | { draw(context: CanvasRenderingContext2D, params: CustomSeriesRenderParams<TData>): void };

export interface ISeriesApi {
  setData(data: Array<LineData | OhlcData | HistogramData>): void;
  update(point: LineData | OhlcData | HistogramData): void;
  applyOptions(options: Record<string, unknown>): void;
  createPriceLine(options: Record<string, unknown>): number;
  createMarker(options: SeriesMarkerOptions): number;
  setMarkers(markers: SeriesMarkerOptions[]): void;
  removeMarker(markerId: number): boolean;
  clearMarkers(): void;
  priceScale(): IPriceScaleApi;
  subscribeDataChanged(handler: (param: unknown) => void): void;
  unsubscribeDataChanged(handler: (param: unknown) => void): void;
}

export interface ICustomSeriesApi<TData = unknown> {
  setData(data: TData[]): void;
  update(point: TData): void;
  applyOptions(options: Record<string, unknown>): void;
  options(): Record<string, unknown>;
  data(): TData[];
  priceScale(): IPriceScaleApi;
  subscribeDataChanged(handler: (param: unknown) => void): void;
  unsubscribeDataChanged(handler: (param: unknown) => void): void;
  detach(): void;
}

export interface ITimeScaleApi {
  fitContent(): void;
  setVisibleRange(range: TimeRange): void;
  setVisibleLogicalRange(range: LogicalRange): void;
  getVisibleRange(): { from: number; to: number } | null;
  getVisibleLogicalRange(): LogicalRange | null;
  timeToCoordinate(time: Time): number | null;
  coordinateToTime(x: number): number | null;
  logicalToCoordinate(logical: number): number | null;
  coordinateToLogical(x: number): number | null;
  barSpacing(): number;
  scrollPosition(): number;
  scrollToPosition(position: number, animated?: boolean): void;
  scrollToRealTime(): void;
}

export interface IPriceScaleApi {
  applyOptions(options: Record<string, unknown>): void;
  options(): Record<string, unknown>;
  width(): number;
}

export interface IChartApi {
  raw(): unknown;
  remove(): void;
  resize(width: number, height: number): void;
  applyOptions(options: Record<string, unknown>): void;
  addSeries(seriesType: SeriesType, options?: Record<string, unknown>, paneIndex?: number): ISeriesApi;
  addCustomSeries<TData = unknown>(renderer: CustomSeriesRenderer<TData>, options?: Record<string, unknown>): ICustomSeriesApi<TData>;
  removeSeries(series: ISeriesApi | ICustomSeriesApi): void;
  timeScale(): ITimeScaleApi;
  priceScale(id?: string): IPriceScaleApi;
  applyMarkerOptions(options: MarkerOptions): void;
  markerOptions(): Required<MarkerOptions>;
  applyWatermarkOptions(options: WatermarkOptions): void;
  watermarkOptions(): WatermarkOptions;
  clearWatermark(): void;
  addAutoscaleContribution(options: AutoscaleContributionOptions): number;
  removeAutoscaleContribution(id: number): boolean;
  clearAutoscaleContributions(): void;
  subscribeClick(handler: (param: unknown) => void): void;
  unsubscribeClick(handler: (param: unknown) => void): void;
  subscribeDblClick(handler: (param: unknown) => void): void;
  unsubscribeDblClick(handler: (param: unknown) => void): void;
  subscribeCrosshairMove(handler: (param: unknown) => void): void;
  unsubscribeCrosshairMove(handler: (param: unknown) => void): void;
}

export const LineSeries: 'Line';
export const AreaSeries: 'Area';
export const BarSeries: 'Bar';
export const CandlestickSeries: 'Candlestick';
export const HistogramSeries: 'Histogram';
export const BaselineSeries: 'Baseline';
export const CustomSeries: 'Custom';

export function initAion_charts(initOptions?: unknown): Promise<unknown>;
export function createChart(container: HTMLElement | string, options?: ChartOptions): Promise<IChartApi>;

declare const _default: {
  createChart: typeof createChart;
  initAion_charts: typeof initAion_charts;
  LineSeries: typeof LineSeries;
  AreaSeries: typeof AreaSeries;
  BarSeries: typeof BarSeries;
  CandlestickSeries: typeof CandlestickSeries;
  HistogramSeries: typeof HistogramSeries;
  BaselineSeries: typeof BaselineSeries;
  CustomSeries: typeof CustomSeries;
};

export default _default;

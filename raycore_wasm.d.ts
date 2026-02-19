/* tslint:disable */
/* eslint-disable */

export class RayCore {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a new RayCore instance inside a container div.
     */
    static create(container_id: string): Promise<RayCore>;
    /**
     * Create with a specific renderer backend ("webgpu" or "canvas2d").
     */
    static create_with(container_id: string, renderer: string): Promise<RayCore>;
    demo_mode(): void;
    /**
     * Dispose: disconnect resize observer.
     */
    dispose(): void;
    static get_supported_renderers(): Array<any>;
    /**
     * Render one frame. Call from requestAnimationFrame.
     */
    render(): void;
    renderer_name(): string;
    set_data(data: Float32Array): void;
    set_data_arrays(open: Float32Array, high: Float32Array, low: Float32Array, close: Float32Array, volume: Float32Array, timestamps: BigUint64Array): void;
    visible_range(): Float64Array;
    zoom_to_range(start: bigint, end: bigint): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_raycore_free: (a: number, b: number) => void;
    readonly raycore_create: (a: number, b: number) => number;
    readonly raycore_create_with: (a: number, b: number, c: number, d: number) => number;
    readonly raycore_demo_mode: (a: number) => void;
    readonly raycore_dispose: (a: number) => void;
    readonly raycore_get_supported_renderers: () => number;
    readonly raycore_render: (a: number) => void;
    readonly raycore_renderer_name: (a: number, b: number) => void;
    readonly raycore_set_data: (a: number, b: number, c: number) => void;
    readonly raycore_set_data_arrays: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => void;
    readonly raycore_visible_range: (a: number, b: number) => void;
    readonly raycore_zoom_to_range: (a: number, b: bigint, c: bigint) => void;
    readonly __wasm_bindgen_func_elem_352: (a: number, b: number) => void;
    readonly __wasm_bindgen_func_elem_737: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_353: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

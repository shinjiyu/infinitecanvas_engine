/* tslint:disable */
/* eslint-disable */
/**
 * WASM 主接口，提供无限画布的所有功能
 *
 * # 职责
 * - 初始化渲染引擎
 * - 管理 Canvas 上下文
 * - 管理场景和视口
 * - 接收并处理 JavaScript 事件
 * - 管理渲染循环
 *
 * # 示例（JavaScript）
 * ```js
 * const canvas = new InfiniteCanvas();
 * await canvas.init(document.getElementById('canvas'));
 * canvas.render();
 * ```
 */
export class InfiniteCanvas {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * 创建新的画布实例
   *
   * # 返回
   * - `Ok(InfiniteCanvas)`: 成功创建
   * - `Err(JsValue)`: 初始化失败
   */
  constructor();
  /**
   * 初始化画布
   *
   * # 参数
   * - `canvas`: HTMLCanvasElement
   *
   * # 返回
   * - `Ok(())`: 初始化成功
   * - `Err(JsValue)`: Canvas 无效或初始化失败
   */
  init(canvas: HTMLCanvasElement): void;
  /**
   * 获取 Canvas 宽度
   */
  width(): number;
  /**
   * 获取 Canvas 高度
   */
  height(): number;
  /**
   * 渲染一帧
   *
   * # 返回
   * - `Ok(())`: 渲染成功
   * - `Err(JsValue)`: 渲染失败
   */
  render(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_infinitecanvas_free: (a: number, b: number) => void;
  readonly infinitecanvas_height: (a: number) => number;
  readonly infinitecanvas_init: (a: number, b: any) => [number, number];
  readonly infinitecanvas_new: () => [number, number, number];
  readonly infinitecanvas_render: (a: number) => [number, number];
  readonly infinitecanvas_width: (a: number) => number;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
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

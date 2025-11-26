/* tslint:disable */
/* eslint-disable */
/**
 * WASM 主接口，提供无限画布的所有功能
 *
 * # 职责
 * - 初始化 WebGPU 渲染引擎
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
   * 添加圆形
   *
   * # 参数
   * - `x, y`: 中心点坐标
   * - `radius`: 半径
   * - `fill_color`: 填充颜色（可选）
   * - `stroke_color`: 描边颜色（可选）
   * - `stroke_width`: 描边宽度
   *
   * # 返回
   * 元素 ID
   */
  add_circle(x: number, y: number, radius: number, fill_color: string | null | undefined, stroke_color: string | null | undefined, stroke_width: number): bigint;
  /**
   * 清空场景
   */
  clear_scene(): void;
  /**
   * 添加矩形
   *
   * # 参数
   * - `x, y`: 中心点坐标
   * - `width, height`: 尺寸
   * - `fill_color`: 填充颜色（可选，如 "#ff0000"）
   * - `stroke_color`: 描边颜色（可选）
   * - `stroke_width`: 描边宽度
   *
   * # 返回
   * 元素 ID
   */
  add_rectangle(x: number, y: number, width: number, height: number, fill_color: string | null | undefined, stroke_color: string | null | undefined, stroke_width: number): bigint;
  /**
   * 删除元素
   *
   * # 参数
   * - `id`: 元素 ID
   *
   * # 返回
   * 是否成功删除
   */
  remove_element(id: bigint): boolean;
  /**
   * 上传纹理到 GPU
   *
   * # 参数
   * - `texture_id`: 纹理的唯一标识符
   * - `width`: 图片宽度（像素）
   * - `height`: 图片高度（像素）
   * - `data`: RGBA8 图像数据（Uint8Array，长度应为 width * height * 4）
   *
   * # 返回
   * - `Ok(())`: 上传成功
   * - `Err(JsValue)`: 上传失败（错误信息）
   *
   * # JavaScript 示例
   * ```js
   * // 从 ImageData 上传
   * const imageData = ctx.getImageData(0, 0, img.width, img.height);
   * canvas.upload_texture("logo", img.width, img.height, imageData.data);
   * ```
   */
  upload_texture(texture_id: string, width: number, height: number, data: Uint8Array): void;
  /**
   * 设置图片透明度
   *
   * # 参数
   * - `image_id`: 图片元素 ID
   * - `alpha`: 透明度 [0.0 - 1.0]
   *
   * # 返回
   * 是否成功设置
   */
  set_image_alpha(image_id: bigint, alpha: number): boolean;
  /**
   * 获取元素数量
   */
  get_element_count(): number;
  /**
   * 获取视口中心 X
   */
  get_viewport_center_x(): number;
  /**
   * 获取视口中心 Y
   */
  get_viewport_center_y(): number;
  /**
   * 创建新的画布实例（构造器，实际初始化在 init() 中完成）
   *
   * # 返回
   * - `Ok(InfiniteCanvas)`: 成功创建
   * - `Err(JsValue)`: 初始化失败
   */
  constructor();
  /**
   * 平移视口
   *
   * # 参数
   * - `dx, dy`: 移动量（世界坐标）
   */
  pan(dx: number, dy: number): void;
  /**
   * 初始化画布（异步，因为 WebGPU 初始化需要异步）
   *
   * # 参数
   * - `canvas`: HTMLCanvasElement
   *
   * # 返回
   * - `Ok(())`: WebGPU 初始化成功
   * - `Err(JsValue)`: WebGPU 不可用或初始化失败
   */
  init(canvas: HTMLCanvasElement): Promise<void>;
  /**
   * 清空场景中所有元素
   *
   * # JavaScript 示例
   * ```js
   * canvas.clear(); // 清空所有元素
   * ```
   */
  clear(): void;
  /**
   * 渲染一帧
   *
   * # 返回
   * - `Ok(())`: 渲染成功
   * - `Err(JsValue)`: 渲染失败（渲染器未初始化）
   */
  render(): void;
  /**
   * 缩放视口
   *
   * # 参数
   * - `anchor_x, anchor_y`: 缩放中心点（屏幕坐标）
   * - `delta_zoom`: 缩放倍数（例如 1.1 放大 10%）
   */
  zoom_at(anchor_x: number, anchor_y: number, delta_zoom: number): void;
  /**
   * 获取视口缩放
   */
  get_zoom(): number;
  /**
   * 添加图片元素
   *
   * # 参数
   * - `x, y`: 左上角位置
   * - `width, height`: 显示尺寸
   * - `texture_id`: 纹理 ID（必须先通过 upload_texture 上传）
   *
   * # 返回
   * 元素 ID
   *
   * # JavaScript 示例
   * ```js
   * // 上传纹理
   * await canvas.upload_texture("logo", 256, 256, imageData);
   * 
   * // 添加图片元素
   * const imageId = canvas.add_image(100, 100, 200, 150, "logo");
   * ```
   */
  add_image(x: number, y: number, width: number, height: number, texture_id: string): bigint;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_infinitecanvas_free: (a: number, b: number) => void;
  readonly infinitecanvas_add_circle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => bigint;
  readonly infinitecanvas_add_image: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => bigint;
  readonly infinitecanvas_add_rectangle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => bigint;
  readonly infinitecanvas_clear: (a: number) => void;
  readonly infinitecanvas_clear_scene: (a: number) => void;
  readonly infinitecanvas_get_element_count: (a: number) => number;
  readonly infinitecanvas_get_viewport_center_x: (a: number) => number;
  readonly infinitecanvas_get_viewport_center_y: (a: number) => number;
  readonly infinitecanvas_get_zoom: (a: number) => number;
  readonly infinitecanvas_init: (a: number, b: any) => any;
  readonly infinitecanvas_new: () => [number, number, number];
  readonly infinitecanvas_pan: (a: number, b: number, c: number) => void;
  readonly infinitecanvas_remove_element: (a: number, b: bigint) => number;
  readonly infinitecanvas_render: (a: number) => [number, number];
  readonly infinitecanvas_set_image_alpha: (a: number, b: bigint, c: number) => number;
  readonly infinitecanvas_upload_texture: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
  readonly infinitecanvas_zoom_at: (a: number, b: number, c: number, d: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hac03de89d7fe1ed3: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__heb574fdda24d5f04: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h71d6670d93209360: (a: number, b: number, c: any, d: any) => void;
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

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
    /**
     * 添加直线
     *
     * # 参数
     * - `x1, y1`: 起点（世界坐标）
     * - `x2, y2`: 终点（世界坐标）
     * - `color`: 描边颜色（可选，如 "#111827"）
     * - `stroke_width`: 线宽
     * - `arrow`: 终点是否绘制箭头
     *
     * # 返回
     * 元素 ID
     *
     * # 示例（JavaScript）
     * ```js
     * const id = canvas.add_line(10, 20, 180, 20, "#111827", 3, true);
     * ```
     */
    add_line(x1: number, y1: number, x2: number, y2: number, color: string | null | undefined, stroke_width: number, arrow: boolean): bigint;
    /**
     * 添加自由画笔路径
     *
     * # 参数
     * - `points`: 扁平坐标 `[x0, y0, x1, y1, ...]`（世界坐标）
     * - `color`: 描边颜色（可选）
     * - `stroke_width`: 线宽
     *
     * # 返回
     * 元素 ID
     *
     * # 示例（JavaScript）
     * ```js
     * const id = canvas.add_path(new Float32Array([0, 0, 40, 10, 40, 80]), "#111827", 2.5);
     * ```
     */
    add_path(points: Float32Array, color: string | null | undefined, stroke_width: number): bigint;
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
     * 是否可以重做
     */
    can_redo(): boolean;
    /**
     * 是否可以撤销
     */
    can_undo(): boolean;
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
     * 清空场景
     */
    clear_scene(): void;
    clear_selection(): void;
    /**
     * 指针下的光标。选择工具命中缩放/旋转手柄时返回对应光标。
     *
     * # 参数
     * - `x`, `y`: 屏幕坐标（与指针事件相同）
     */
    cursor_at(x: number, y: number): string;
    debug_clear_events(): void;
    debug_pending_event_count(): number;
    deselect_element(element_id: bigint): boolean;
    /**
     * 导出场景为版本化 JSON
     *
     * 文档含 `version`、`next_id` 和 `elements`。矩形、圆、图片、直线（含箭头）
     * 和自由路径都会写入；变换的 `rotation` 与 `scale` 在 `props.transform` 里。
     *
     * # 返回
     * 紧凑 JSON 字符串
     *
     * # 示例（JavaScript）
     * ```js
     * const json = canvas.export_json();
     * localStorage.setItem("infinitecanvas.scene.v1", json);
     * ```
     */
    export_json(): string;
    /**
     * 把当前视口或选中区域导出为 PNG
     *
     * # 参数
     * - `selection_only`: `true` 时导出选中元素的包围盒；`false` 时导出当前视口
     *
     * # 返回
     * RGBA PNG 字节。直线、箭头和自由路径按线宽栅格化。图片纹理不在场景文件里，
     * 导出为纯色占位。
     *
     * # 错误
     * 没有选中元素、区域为空或编码失败
     */
    export_png(selection_only: boolean): Uint8Array;
    /**
     * 获取当前工具名称
     */
    get_current_tool(): string;
    /**
     * 获取元素数量
     */
    get_element_count(): number;
    /**
     * 获取元素信息
     *
     * # 返回
     * 如果元素存在，返回元素信息对象；否则返回 null
     */
    get_element_info(id: bigint): any;
    /**
     * 获取帧计数
     */
    get_frame_count(): bigint;
    /**
     * 获取上一帧绘制调用数
     */
    get_last_draw_calls(): number;
    /**
     * 获取上一帧顶点数
     */
    get_last_vertex_count(): number;
    /**
     * 获取性能统计信息
     *
     * # 返回
     * 包含以下字段的对象：
     * - frameCount: 总帧数
     * - vertexCount: 上一帧顶点数
     * - drawCalls: 上一帧绘制调用数
     * - frameTimeMs: 上一帧渲染时间（毫秒）
     * - elementCount: 场景元素总数
     */
    get_performance_stats(): any;
    /**
     * 获取重做栈大小
     */
    get_redo_count(): number;
    get_selected_ids(): Array<any>;
    /**
     * 获取选中元素的边界框
     *
     * # 返回
     * 如果有选中元素，返回 { x, y, width, height }；否则返回 null
     */
    get_selection_bounds(): any;
    /**
     * 获取当前框选矩形（如果正在框选）
     *
     * # 返回
     * 如果正在框选，返回 { x, y, width, height }；否则返回 null
     */
    get_selection_box_bounds(): any;
    get_selection_count(): number;
    /**
     * 获取完整的画布状态（用于 UI 同步）
     *
     * # 返回
     * 包含以下字段的对象：
     * - tool: 当前工具名称
     * - cursor: 光标样式
     * - selectionCount: 选中元素数量
     * - elementCount: 总元素数量
     * - canUndo: 是否可撤销
     * - canRedo: 是否可重做
     * - isDragging: 是否正在拖动
     * - isSelectionBoxActive: 是否正在框选
     * - zoom: 当前缩放比例
     */
    get_state(): any;
    /**
     * 获取当前工具的光标样式
     */
    get_tool_cursor(): string;
    /**
     * 获取撤销栈大小
     */
    get_undo_count(): number;
    /**
     * 获取视口中心 X
     */
    get_viewport_center_x(): number;
    /**
     * 获取视口中心 Y
     */
    get_viewport_center_y(): number;
    /**
     * 获取视口缩放
     */
    get_zoom(): number;
    handle_keyboard_event(kind: string, key: string, code: string, repeat: boolean, shift_key: boolean, ctrl_key: boolean, alt_key: boolean, meta_key: boolean): void;
    handle_pointer_event(kind: string, pointer_id: number, x: number, y: number, movement_x: number, movement_y: number, button: number, buttons_mask: number, shift_key: boolean, ctrl_key: boolean, alt_key: boolean, meta_key: boolean): void;
    handle_wheel_event(delta_x: number, delta_y: number, delta_mode: number, shift_key: boolean, ctrl_key: boolean, alt_key: boolean, meta_key: boolean): void;
    hit_test_screen_point(x: number, y: number, max_results?: number | null): Array<any>;
    hit_test_world_point(x: number, y: number, max_results?: number | null): Array<any>;
    /**
     * 用版本化 JSON 替换当前场景
     *
     * 缺省字段使用默认值，未知字段会被忽略。导入会按 `selected` 标记重建选择，
     * 并清空撤销栈和未完成的拖动。解析失败时场景保持不变。
     *
     * # 参数
     * - `json`: [`export_json`](Self::export_json) 生成的文本
     *
     * # 错误
     * JSON 无法解析、版本不受支持、元素种类未知或 ID 重复
     */
    import_json(json: string): void;
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
     * 检查是否正在拖动
     */
    is_dragging(): boolean;
    /**
     * 检查是否正在框选
     */
    is_selection_box_active(): boolean;
    /**
     * 移动选中的元素
     *
     * # 参数
     * - `dx`: X 方向移动量（世界坐标）
     * - `dy`: Y 方向移动量（世界坐标）
     */
    move_selection(dx: number, dy: number): void;
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
     * 恢复到之前的工具
     */
    pop_temporary_tool(): void;
    /**
     * 临时切换到手型工具（按住空格）
     */
    push_hand_tool(): void;
    /**
     * 重做上一个撤销的操作
     *
     * # 返回
     * 是否成功重做
     */
    redo(): boolean;
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
     * 渲染一帧
     *
     * # 返回
     * - `Ok(())`: 渲染成功
     * - `Err(JsValue)`: 渲染失败（渲染器未初始化）
     */
    render(): void;
    select_element(element_id: bigint, additive: boolean): boolean;
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
     * 设置当前工具
     *
     * # 参数
     * - `tool_name`: 工具名称 ("select", "rectangle", "circle", "pan", "line", "pen")
     *
     * 直线快捷键 `l`，画笔快捷键 `p`。切换工具会丢掉尚未抬笔的预览。
     *
     * # 返回
     * 是否成功设置
     */
    set_tool(tool_name: string): boolean;
    /**
     * 撤销上一个操作
     *
     * # 返回
     * 是否成功撤销
     */
    undo(): boolean;
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
     * 缩放视口
     *
     * # 参数
     * - `anchor_x, anchor_y`: 缩放中心点（屏幕坐标）
     * - `delta_zoom`: 缩放倍数（例如 1.1 放大 10%）
     */
    zoom_at(anchor_x: number, anchor_y: number, delta_zoom: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_infinitecanvas_free: (a: number, b: number) => void;
    readonly infinitecanvas_add_circle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => bigint;
    readonly infinitecanvas_add_image: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => bigint;
    readonly infinitecanvas_add_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => bigint;
    readonly infinitecanvas_add_path: (a: number, b: number, c: number, d: number, e: number, f: number) => bigint;
    readonly infinitecanvas_add_rectangle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => bigint;
    readonly infinitecanvas_can_redo: (a: number) => number;
    readonly infinitecanvas_can_undo: (a: number) => number;
    readonly infinitecanvas_clear: (a: number) => void;
    readonly infinitecanvas_clear_scene: (a: number) => void;
    readonly infinitecanvas_clear_selection: (a: number) => void;
    readonly infinitecanvas_cursor_at: (a: number, b: number, c: number) => [number, number];
    readonly infinitecanvas_debug_clear_events: (a: number) => void;
    readonly infinitecanvas_debug_pending_event_count: (a: number) => number;
    readonly infinitecanvas_deselect_element: (a: number, b: bigint) => number;
    readonly infinitecanvas_export_json: (a: number) => [number, number, number, number];
    readonly infinitecanvas_export_png: (a: number, b: number) => [number, number, number, number];
    readonly infinitecanvas_get_current_tool: (a: number) => [number, number];
    readonly infinitecanvas_get_element_count: (a: number) => number;
    readonly infinitecanvas_get_element_info: (a: number, b: bigint) => any;
    readonly infinitecanvas_get_frame_count: (a: number) => bigint;
    readonly infinitecanvas_get_last_draw_calls: (a: number) => number;
    readonly infinitecanvas_get_last_vertex_count: (a: number) => number;
    readonly infinitecanvas_get_performance_stats: (a: number) => any;
    readonly infinitecanvas_get_redo_count: (a: number) => number;
    readonly infinitecanvas_get_selected_ids: (a: number) => any;
    readonly infinitecanvas_get_selection_bounds: (a: number) => any;
    readonly infinitecanvas_get_selection_box_bounds: (a: number) => any;
    readonly infinitecanvas_get_selection_count: (a: number) => number;
    readonly infinitecanvas_get_state: (a: number) => any;
    readonly infinitecanvas_get_tool_cursor: (a: number) => [number, number];
    readonly infinitecanvas_get_undo_count: (a: number) => number;
    readonly infinitecanvas_get_viewport_center_x: (a: number) => number;
    readonly infinitecanvas_get_viewport_center_y: (a: number) => number;
    readonly infinitecanvas_get_zoom: (a: number) => number;
    readonly infinitecanvas_handle_keyboard_event: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly infinitecanvas_handle_pointer_event: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => void;
    readonly infinitecanvas_handle_wheel_event: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly infinitecanvas_hit_test_screen_point: (a: number, b: number, c: number, d: number) => any;
    readonly infinitecanvas_hit_test_world_point: (a: number, b: number, c: number, d: number) => any;
    readonly infinitecanvas_import_json: (a: number, b: number, c: number) => [number, number];
    readonly infinitecanvas_init: (a: number, b: any) => any;
    readonly infinitecanvas_is_dragging: (a: number) => number;
    readonly infinitecanvas_is_selection_box_active: (a: number) => number;
    readonly infinitecanvas_move_selection: (a: number, b: number, c: number) => void;
    readonly infinitecanvas_new: () => [number, number, number];
    readonly infinitecanvas_pan: (a: number, b: number, c: number) => void;
    readonly infinitecanvas_pop_temporary_tool: (a: number) => void;
    readonly infinitecanvas_push_hand_tool: (a: number) => void;
    readonly infinitecanvas_redo: (a: number) => number;
    readonly infinitecanvas_remove_element: (a: number, b: bigint) => number;
    readonly infinitecanvas_render: (a: number) => [number, number];
    readonly infinitecanvas_select_element: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_image_alpha: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_tool: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_undo: (a: number) => number;
    readonly infinitecanvas_upload_texture: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly infinitecanvas_zoom_at: (a: number, b: number, c: number, d: number) => void;
    readonly wasm_bindgen_413c9591f4645d29___convert__closures_____invoke___js_sys_9cc5f2a5e2dd9295___Function_fn_wasm_bindgen_413c9591f4645d29___JsValue_____wasm_bindgen_413c9591f4645d29___sys__Undefined___js_sys_9cc5f2a5e2dd9295___Function_fn_wasm_bindgen_413c9591f4645d29___JsValue_____wasm_bindgen_413c9591f4645d29___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_413c9591f4645d29___convert__closures_____invoke___wasm_bindgen_413c9591f4645d29___JsValue__core_608f92abc48d28da___result__Result_____wasm_bindgen_413c9591f4645d29___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_413c9591f4645d29___convert__closures_____invoke___wasm_bindgen_413c9591f4645d29___JsValue______true_: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
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

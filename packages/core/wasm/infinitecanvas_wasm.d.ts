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
     * 光栅化失败后允许下一帧重新取任务。
     */
    abandon_text_raster(id: bigint): void;
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
     * 从 PNG 或 JPEG 字节创建图片元素。
     *
     * 像素进入场景资源表：同一份字节多次调用只存一份。边长超过 4096 时先降采样。
     * 显示尺寸默认等于像素尺寸；长边超过 512 世界单位时等比缩小，避免一张照片铺满视口。
     * 元素进入撤销栈，并成为当前选择。
     *
     * # 参数
     * - `bytes`: 文件字节
     * - `mime`: `image/png` 或 `image/jpeg`。空字符串时按文件头识别
     * - `x`, `y`: 左上角（世界坐标）
     *
     * # 返回
     * 元素 ID
     *
     * # 错误
     * 格式不支持或解码失败
     *
     * # 示例（JavaScript）
     * ```js
     * const bytes = new Uint8Array(await file.arrayBuffer());
     * const id = canvas.add_image_bytes(bytes, file.type, worldX, worldY);
     * ```
     */
    add_image_bytes(bytes: Uint8Array, mime: string, x: number, y: number): bigint;
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
     * 添加正多边形，并记入撤销栈。
     *
     * # 参数
     * - `x, y`: 中心点
     * - `radius`: 外接圆半径
     * - `sides`: 边数，钳到 3–64。缺省调用方应传 6
     * - `fill_color` / `stroke_color` / `stroke_width`: 与 [`Self::add_rectangle`] 相同
     *
     * # 返回
     * 元素 ID
     *
     * # 示例（JavaScript）
     * ```js
     * const id = canvas.add_polygon(0, 0, 40, 6, "#f59e0b", null, 1);
     * canvas.undo();
     * ```
     */
    add_polygon(x: number, y: number, radius: number, sides: number, fill_color: string | null | undefined, stroke_color: string | null | undefined, stroke_width: number): bigint;
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
     * 添加星形，并记入撤销栈。
     *
     * # 参数
     * - `points`: 角数，钳到 3–64。五角星传 5
     * - `inner_radius`: 内点半径相对外接圆的比例，钳到 0.05–0.95
     *
     * # 返回
     * 元素 ID
     *
     * # 示例（JavaScript）
     * ```js
     * const id = canvas.add_star(0, 0, 40, 5, 0.5, "#a855f7", null, 1);
     * ```
     */
    add_star(x: number, y: number, radius: number, points: number, inner_radius: number, fill_color: string | null | undefined, stroke_color: string | null | undefined, stroke_width: number): bigint;
    /**
     * 添加文本元素。
     *
     * 位置是锚点：左对齐为左上角，居中为上边中点，右对齐为右上角。
     * 这个接口和 `add_rectangle` 一样直接写入场景，不进入撤销栈。
     * T 工具创建的文本会进入撤销栈。
     *
     * # 参数
     * - `x`, `y`: 锚点（世界坐标）
     * - `content`: 文本，支持换行
     * - `font_size`: 世界单位字号，`<= 0` 时用 32
     * - `color`: `#RRGGBB` 或 `#RRGGBBAA`，空则用浅色
     * - `align`: `"left"` / `"center"` / `"right"`
     *
     * # 返回
     * 元素 ID
     */
    add_text(x: number, y: number, content: string, font_size: number, color?: string | null, align?: string | null): bigint;
    /**
     * 按包围盒对齐当前选择，并记入一条撤销记录。
     *
     * 至少两个顶层可见、未锁定的选中元素才生效。选中组时用组的世界包围盒，只平移组根。
     * 单选、空选或已经贴齐时不入栈。不经过拖拽吸附。
     *
     * # 参数
     * - `mode`: `left` / `center` / `right` / `top` / `middle` / `bottom`
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.align_selection("left");
     * ```
     */
    align_selection(mode: string): boolean;
    /**
     * 采样该世界点，并把颜色写到当前选择。
     *
     * 填充写到形状 `fill` 和文本颜色，描边写到 `stroke`。缺的通道不改，也不清掉。
     * 多选合成一条撤销记录。空选、全部锁定、全部隐藏、点在空白、采样没有颜色，
     * 或颜色与目标相同，都返回 false，不入栈。
     *
     * 不切换工具。页面上的吸管工具在松开指针后再回到之前的工具。
     *
     * # 示例（JavaScript）
     * ```js
     * const applied = canvas.apply_sampled_style_at(worldX, worldY);
     * ```
     */
    apply_sampled_style_at(x: number, y: number): boolean;
    /**
     * 标尺和手动参考线是否显示。
     */
    are_rulers_visible(): boolean;
    /**
     * 双击命中文本时返回编辑框几何。没命中文本返回 `null`。
     *
     * # 参数
     * - `x`, `y`: 屏幕像素坐标（和 `handle_pointer_event` 用的是同一套）
     */
    begin_text_edit_at(x: number, y: number): any;
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
     * 把进行中的样式预览收成一条撤销记录。
     *
     * 场景已经是预览后的值，所以只入栈、不再执行。没有变化时返回 false。
     */
    commit_style_preview(): boolean;
    /**
     * 提交编辑内容。
     *
     * 内容有变化时写入撤销栈。只剩空白时删除元素，撤销会把原文放回来。
     *
     * # 返回
     * 元素存在并且这次调用处理完了编辑状态。
     */
    commit_text_content(id: bigint, content: string): boolean;
    /**
     * 把已经上传的纹理绑定到文本元素。
     *
     * `revision` 必须等于取任务时的版本，否则结果被丢弃并删除这张纹理。
     * `world_width` / `world_height` 是纹理像素除以 `rasterScale`，用来固定世界尺寸。
     *
     * # 返回
     * 是否接受了这张纹理。
     */
    commit_text_raster(id: bigint, revision: number, lod: number, texture_id: string, world_width: number, world_height: number): boolean;
    /**
     * 把当前选中元素写成剪贴板 JSON。
     *
     * 格式与 [`export_json`](Self::export_json) 相同，只含选中根和它们的子孙。
     * 组会带上整棵子树，父子 ID 仍指向这份文档里的元素。图片会带上它们的 `assets[]`。
     * 没有选中元素时返回空字符串。
     *
     * # 示例（JavaScript）
     * ```js
     * const json = canvas.copy_selection_json();
     * await navigator.clipboard.writeText(json);
     * ```
     */
    copy_selection_json(): string;
    /**
     * 指针下的光标。选择工具命中缩放/旋转手柄时返回对应光标。
     *
     * # 参数
     * - `x`, `y`: 屏幕坐标（与指针事件相同）
     */
    cursor_at(x: number, y: number): string;
    debug_clear_events(): void;
    debug_pending_event_count(): number;
    /**
     * 双击命中参考线时删除它。
     *
     * # 参数
     * - `x`: 画布像素 x
     * - `y`: 画布像素 y
     *
     * # 返回
     * 命中并删除时为 `true`。标尺隐藏时为 `false`。
     */
    delete_guide_at(x: number, y: number): boolean;
    /**
     * 删除当前选中的参考线，并记入撤销栈。
     *
     * # 返回
     * 确实删掉了一条参考线时为 `true`。
     */
    delete_selected_guide(): boolean;
    /**
     * 删除当前选中的元素。
     *
     * 多个元素合成一条撤销记录。选中的是组时，整棵子树一起删除。
     * 没有选中时什么也不做。快照里的选中标记会随撤销恢复。
     * 删除后会像 [`Self::remove_element`] 一样，把这些 id（含子孙）从选择集里去掉，
     * 并释放不再被场景使用的文本纹理。
     *
     * # 返回
     * 从场景中删除的元素数量，含组的子孙
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.delete_selection();
     * ```
     */
    delete_selection(): number;
    deselect_element(element_id: bigint): boolean;
    /**
     * 按包围盒间隙均匀分布当前选择，并记入一条撤销记录。
     *
     * 至少三个顶层可见、未锁定的选中元素才生效。两端元素不动。
     * 选中组时移动组根，不拆开子树。单选、空选或只有两个目标时不入栈。
     * 不经过拖拽吸附。
     *
     * # 参数
     * - `axis`: `horizontal` / `vertical`（也接受 `x` / `y`）
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.distribute_selection("horizontal");
     * ```
     */
    distribute_selection(axis: string): boolean;
    /**
     * 原地克隆当前选中元素，并让新元素成为选中。
     *
     * 走 [`DuplicateElementsCommand`]，Ctrl+Z 会删掉副本，Ctrl+Y 按原 ID 恢复。
     * 新元素标脏，下一帧顶点缓存会写上它们。组会克隆整棵子树。
     *
     * # 参数
     * - `dx`, `dy`: 世界位移。Demo 的 Ctrl+D 使用 `(10, 10)`
     *
     * # 返回
     * 新根元素的 ID。没有选中元素时为空数组。
     */
    duplicate_selection(dx: number, dy: number): Array<any>;
    /**
     * 把元素放进组，世界坐标不变。
     */
    enter_group(element_id: bigint, group_id: bigint): boolean;
    /**
     * 导出场景为版本化 JSON
     *
     * 文档含 `version`、`next_id` 和 `elements`。矩形、圆、图片、直线（含箭头）、
     * 自由路径、文本和分组都会写入。组的 `children` 以及元素的 `parent` / `locked`
     * 在 `data` 里。变换的 `rotation` 与 `scale` 在 `props.transform` 里。
     * `dirty` 和文本光栅字段是运行时标记，不会出现在 JSON 里。
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
     * RGBA PNG 字节。直线、箭头和自由路径按线宽栅格化。文本用 font8x8 点阵，
     * 没有点阵的字符画方框。资源表里有像素的图片按显示矩形采样；只有 GPU 纹理
     * id 的图片仍是纯色占位。分组本身不绘制；隐藏的组会连同子孙一起跳过。
     *
     * # 错误
     * 没有选中元素、区域为空或编码失败
     */
    export_png(selection_only: boolean): Uint8Array;
    /**
     * 把场景导出为 SVG 文本。
     *
     * 矩形、圆、直线（含箭头）、自由路径、文本，以及带像素的图片按绘制顺序写入。
     * 变换使用与画布相同的 `matrix`（缩放、旋转、平移）。`viewBox` 默认是全部可见元素的包围盒。
     *
     * # 参数
     * - `selected_only`: `true` 时只导出当前选中元素，以及选中组下的可见子孙。包围盒也只包住这些元素
     *
     * # 返回
     * SVG 文档。有资源字节的图片写成 `<image>`，`href` 是 data URL。
     * 没有字节的纹理会被跳过并在文档中注明。文本写成 `<text>`，锚点与对齐方式一致。
     *
     * # 示例（JavaScript）
     * ```js
     * const svg = canvas.export_svg(false);
     * ```
     */
    export_svg(selected_only: boolean): string;
    /**
     * [`Self::zoom_to_fit`] 的别名。
     */
    fit_scene(): boolean;
    /**
     * 水平或垂直翻转当前选中元素，并记入一条撤销记录。
     *
     * 绕选区中心镜像。只有一个目标时，中心是该元素的世界包围盒中心。
     * 选中组时翻转组根，子孙跟着走，不拆组。只改平移、旋转和缩放符号。
     * 空选、锁定或无法识别的轴不入栈。
     *
     * 进行中的拖动、变换手柄和样式预览会先各自入栈，再做这次翻转。
     *
     * # 参数
     * - `axis`: `"horizontal"` / `"h"` / `"x"`，或 `"vertical"` / `"v"` / `"y"`
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.flip_selection("horizontal");
     * ```
     */
    flip_selection(axis: string): boolean;
    /**
     * 当前生效的对齐参考线。
     *
     * 只在吸附开启且正在拖拽、并且选区进入阈值时非空。
     * 每条记录包含 `axis`（`"horizontal"` 或 `"vertical"`）和 `position`（世界坐标）。
     *
     * # 返回
     * 参考线数组。没有参考线时为空数组。
     */
    get_alignment_guides(): Array<any>;
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
     * 可见性或锁定的版本。图层面板用它重读眼睛和锁，不代表场景结构变了。
     */
    get_flags_epoch(): bigint;
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
     * 场景里的手动参考线。
     *
     * 每条记录含 `id`、`axis`（`"horizontal"` / `"vertical"`）、`position`（世界坐标）
     * 和 `selected`。隐藏标尺时仍返回数据，绘制由调用方决定。
     */
    get_manual_guides(): Array<any>;
    /**
     * 获取性能统计信息
     *
     * # 返回
     * 包含以下字段的对象：
     * - frameCount: 总帧数
     * - vertexCount: 上一帧顶点数
     * - drawCalls: 上一帧绘制调用数
     * - frameTimeMs: 上一帧渲染时间（毫秒，CPU 提交，不含 GPU 完成）
     * - elementCount: 场景元素总数
     * - regeneratedElements: 本帧重生成顶点的元素数
     * - gpuBytesWritten: 本帧写入顶点缓冲的字节数
     * - geometryUnchanged: 为 true 时本帧只更新了视口 uniform
     */
    get_performance_stats(): any;
    /**
     * 获取重做栈大小
     */
    get_redo_count(): number;
    /**
     * 主刻度的世界步长。缩放越大，步长越小。
     */
    get_ruler_major_step(): number;
    /**
     * 标尺厚度（屏幕像素）。
     */
    get_ruler_thickness(): number;
    /**
     * 一条标尺上的刻度。
     *
     * `axis` 为 `"horizontal"`（顶边，世界 x）或 `"vertical"`（左边，世界 y）。
     * 每条记录含 `world`、`screen` 和 `major`。
     *
     * # 参数
     * - `axis`: `"horizontal"` 或 `"vertical"`
     */
    get_ruler_ticks(axis: string): Array<any>;
    /**
     * 场景树。`roots` 里每个节点含 `id`、`elementType`、`parentId`、`visible`、`locked`、`zIndex`、`children`。
     */
    get_scene_tree(): any;
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
     * 读取当前选择的公共样式。
     *
     * 没有选中元素时 `count` 为 0，`hasShape` / `hasText` / `hasImage` 为 false。
     * 组会展开到子孙。图片只出现在 `opacity` 里。字段含义：
     *
     * - `fill` / `stroke` / `textColor`：`#rrggbbaa`，没有颜色时为 null
     * - `fillMixed` 等：多选不一致
     * - `fillNone`：大家都没有填充或描边
     * - `fillSupported`：选择里有这个字段
     * - `strokeWidth` / `opacity` / `fontSize` / `cornerRadius`：统一数值，混合或不支持时为 null
     * - `strokeDash`：数字数组，空数组是实线。`strokeCap` 为 `butt` / `round` / `square`，
     *   `strokeJoin` 为 `miter` / `round` / `bevel`。这三项只统计形状，文本描边不参与
     * - `cornerRadius` 只在选择里有矩形时出现（`hasRect` / `cornerRadiusSupported`）。多选半径不一致时 `cornerRadiusMixed` 为 true
     * - `sides` 是多边形边数或星形角数（`hasPolygon` / `hasStar` / `sidesSupported`）。`innerRadius` 只统计星形
     * - `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowOpacity` 是数字。`shadowColor` 是 `#rrggbbaa`（面板只用 RGB，浓度在 `shadowOpacity`）。没有形状、文本或图片时 `shadowSupported` 为 false
     * - `fillKind`：`solid` / `linear` / `radial`。`fillStops` 是 `{offset, color}` 数组。
     *   `fillAngle` 为度，`fillCenter` 为 `{x, y}`，`fillRadius` 为局部半径（0 表示自动）。
     *   这几项只统计形状。多选不一致时对应的 `*Mixed` 为 true
     *
     * # 示例（JavaScript）
     * ```js
     * const style = canvas.get_selection_style();
     * if (style.hasShape && !style.fillMixed) colorInput.value = style.fill.slice(0, 7);
     * ```
     */
    get_selection_style(): any;
    /**
     * 当前吸附阈值（屏幕像素）。
     */
    get_snap_threshold(): number;
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
     * - snapEnabled: 拖拽吸附是否开启
     * - snapThreshold: 吸附阈值（屏幕像素）
     */
    get_state(): any;
    /**
     * 场景结构版本。增删元素或大纲重排后变化，图层面板用它决定是否重读场景树。
     */
    get_structure_epoch(): bigint;
    /**
     * 查询文本在屏幕上的位置，编辑框跟着平移和缩放移动。
     */
    get_text_editor_metrics(id: bigint): any;
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
    /**
     * 背景网格是否可见。
     *
     * # 返回
     * 可见时为 `true`
     */
    grid_visible(): boolean;
    /**
     * 把至少两个元素组成一个 Group。
     *
     * # 参数
     * - `ids`: 元素 ID。可以是 `BigInt`（`add_*` 的返回值）或数字。
     *
     * # 返回
     * 新组的 ID。失败时返回 0（ID 从 1 开始）。
     *
     * # 示例（JavaScript）
     * ```js
     * const groupId = canvas.group_elements([rectA, rectB]);
     * canvas.translate_element(groupId, 20, 0);
     * canvas.ungroup(groupId);
     * ```
     */
    group_elements(ids: Array<any>): bigint;
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
     * 成功后顶点缓存整表失效，下一帧全量上传。
     *
     * # 参数
     * - `json`: [`export_json`](Self::export_json) 生成的文本
     *
     * # 错误
     * JSON 无法解析、版本不受支持、元素种类未知或 ID 重复
     */
    import_json(json: string): void;
    /**
     * 把 SVG 文本解析成场景元素，并记一条撤销记录。
     *
     * 支持 `rect`、`circle`、`ellipse`、`line`、`polyline`、`polygon`、`path`、
     * `text`，以及 `translate` / `scale` / `rotate` / `matrix` 的嵌套 `g`。
     * `image` 只接受 PNG/JPEG 的 data URL。一次导入是一条历史，Ctrl+Z 删掉整批。
     *
     * # 参数
     * - `svg`: SVG 文档
     * - `x`, `y`: viewBox 原点（没有 viewBox 时是用户坐标原点）的世界坐标
     *
     * # 返回
     * 新元素 ID，文档顺序。这些元素成为当前选择。
     *
     * # 错误
     * 空文本、非法 XML、根节点不是 `svg`，或没有可识别图元。失败时场景不变。
     *
     * # 示例（JavaScript）
     * ```js
     * const ids = canvas.import_svg('<svg><rect width="10" height="10"/></svg>', worldX, worldY);
     * ```
     */
    import_svg(svg: string, x: number, y: number): Array<any>;
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
     * 拖拽吸附是否开启。
     */
    is_snap_enabled(): boolean;
    /**
     * 把元素移出当前组，挂回组的父节点或根。
     */
    leave_group(element_id: bigint): boolean;
    /**
     * 用 `count` 个矩形替换当前场景，供压测页切换 1k/10k/100k。
     *
     * 元素按网格铺开。第一次渲染会建立顶点缓存，之后平移/缩放只更新 uniform。
     */
    load_stress_scene(count: number): void;
    /**
     * 元素在小地图上的简化外框。
     *
     * 同一几何版本和尺寸会复用缓存，避免 10 万元素时每帧重扫。
     * 返回 `Float32Array`，每 4 个数是一个像素矩形 `x, y, width, height`。
     *
     * # 参数
     * - `width`, `height`: 小地图像素尺寸，例如 200×150
     * - `padding`: 四周留白（像素）
     */
    minimap_element_rects(width: number, height: number, padding: number): Float32Array;
    /**
     * 小地图几何版本。
     *
     * 与 [`infinitecanvas_engine::Scene::geometry_epoch`] 相同。
     * 元素增删或几何变化时递增，平移/缩放相机不变。
     * 页面应在版本变化时才调用 [`Self::minimap_element_rects`]，
     * 相机变化只调用 [`Self::minimap_viewport_rect`]。
     */
    minimap_epoch(): bigint;
    /**
     * 小地图像素坐标反算世界坐标。
     *
     * 点击处要成为相机中心时，把返回值交给 [`Self::set_camera_center`]。
     * 留白和内容区用的是同一个线性映射。
     *
     * # 返回
     * `Float32Array` `[x, y]`。
     */
    minimap_to_world(x: number, y: number, width: number, height: number, padding: number): Float32Array;
    /**
     * 当前视口在小地图上的像素矩形。
     *
     * 映射跟元素外框共用缓存，本身不扫描场景。
     *
     * # 返回
     * `Float32Array` `[x, y, width, height]`。
     */
    minimap_viewport_rect(width: number, height: number, padding: number): Float32Array;
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
     * 用方向键平移当前选中元素，并记入一条撤销记录。
     *
     * 只移动顶层、可见、未锁定的选中元素。选中组时移动组根，子孙跟着走，只改平移。
     * 步长是世界单位，与缩放无关：默认 1，`shift` 为 true 时为 10。
     * 多选合成一条历史。空选、锁定或无法识别的方向不入栈。不经过拖拽吸附。
     *
     * 进行中的拖动、变换手柄和样式预览会先各自入栈，再做这次平移。
     *
     * # 参数
     * - `direction`: `"left"`、`"right"`、`"up"`、`"down"`，或 `"ArrowLeft"` 等
     * - `shift`: 为 true 时步长 ×10
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.nudge_selection("right", event.shiftKey);
     * ```
     */
    nudge_selection(direction: string, shift: boolean): boolean;
    /**
     * 平移视口
     *
     * # 参数
     * - `dx, dy`: 移动量（世界坐标）
     */
    pan(dx: number, dy: number): void;
    /**
     * 把剪贴板 JSON 粘贴进场景，并选中新根。
     *
     * `focus_x` / `focus_y` 都给出时，叶子包围盒中心对准该世界坐标，再加 `(dx, dy)`。
     * 缺一个焦点时只应用位移（鼠标不在画布上时 Demo 传 `(10, 10)` 的倍数）。
     * 非法 JSON 不修改场景。
     *
     * # 参数
     * - `json`: [`copy_selection_json`](Self::copy_selection_json) 的文本
     * - `dx`, `dy`: 额外世界位移，连续粘贴用来错开
     * - `focus_x`, `focus_y`: 可选的粘贴中心
     *
     * # 返回
     * 新根元素 ID
     *
     * # 错误
     * JSON 无法解析、版本不受支持或元素种类未知
     */
    paste_elements(json: string, dx: number, dy: number, focus_x?: number | null, focus_y?: number | null): Array<any>;
    /**
     * 恢复到之前的工具
     */
    pop_temporary_tool(): void;
    /**
     * 预览样式。第一次调用记下基线，之后每次都从基线重放。
     *
     * 不进入撤销栈。松手后调用 [`Self::commit_style_preview`]。
     */
    preview_selection_style(patch: any): boolean;
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
    /**
     * 把元素移到大纲中的新位置，并记入撤销栈。
     *
     * 同一父节点下改变绘制顺序，或移入 / 移出组。世界坐标不变。
     * `parent_id` 为 `0` 时挂到根上（元素 ID 从 1 开始）。`index` 是摘下该元素之后
     * 在目标兄弟列表中的下标，`0` 为最底层。
     *
     * 锁定、成环或目标不是组时返回 `false`，不入栈。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.reorder_element(id, groupId, 0);
     * canvas.undo();
     * ```
     */
    reorder_element(id: bigint, parent_id: bigint, index: number): boolean;
    /**
     * 按当前选中重排同一层的兄弟，并记入一条撤销记录。
     *
     * `action` 为 `front`（置顶）、`back`（置底）、`forward`（前移一层）或 `backward`（后移一层）。
     * 只在同一个父节点下移动。选中组时移动组根，不拆开子树。
     * 空选、跨父节点、全部锁定（含锁定的祖先）或顺序不变时返回 `false`，不入栈。
     * 同一层里锁定的选中项会跳过。隐藏不阻止重排。
     *
     * 进行中的拖动、变换手柄和样式预览会先各自入栈，再做这次重排。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.reorder_selection("front");
     * canvas.undo();
     * ```
     */
    reorder_selection(action: string): boolean;
    /**
     * 把当前选中元素顺时针或逆时针转 90°，并记入一条撤销记录。
     *
     * 绕选区中心旋转。屏幕 Y 向下，顺时针是正角度。缩放（含符号）保持不变。
     * 选中组时旋转组根。空选、锁定或无法识别的方向不入栈。
     *
     * # 参数
     * - `dir`: `"cw"` / `"clockwise"`，或 `"ccw"` / `"counterclockwise"`
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.rotate_selection_quarter("cw");
     * ```
     */
    rotate_selection_quarter(dir: string): boolean;
    /**
     * 采样世界坐标上最上层可见叶子的填充和描边。
     *
     * 返回对象：
     * - `id`：命中的元素。空白处为 null
     * - `fill` / `stroke`：`#rrggbbaa`。元素没有该通道，或没有命中时为 null
     *
     * 文本的 `fill` 是文字颜色。图片命中时 `id` 有值，两个颜色都是 null。
     * 锁定元素仍可采样。隐藏元素（含被隐藏的祖先）会跳过，露出下面的叶子。
     * 不读像素。
     *
     * # 示例（JavaScript）
     * ```js
     * const sample = canvas.sample_style_at(worldX, worldY);
     * if (sample.fill) colorInput.value = sample.fill.slice(0, 7);
     * ```
     */
    sample_style_at(x: number, y: number): any;
    /**
     * 可见叶子的世界包围盒。
     *
     * 组不计入，隐藏元素及其子孙也不计入。空场景返回 `null`。
     *
     * # 返回
     * `{ x, y, width, height }`，或 `null`。
     */
    scene_bounds(): any;
    /**
     * 屏幕坐标转世界坐标。拖放和粘贴图片时用它定位左上角。
     *
     * # 返回
     * `[x, y]`
     */
    screen_to_world(x: number, y: number): Float32Array;
    /**
     * 选中顶层的可见元素。
     *
     * 组只选中组本身，不选中组内元素。隐藏元素不选。锁定的元素，以及被锁定祖先挡住的组，也不选。
     *
     * # 返回
     * 选中数量。空场景返回 0。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.select_all();
     * ```
     */
    select_all(): number;
    select_element(element_id: bigint, additive: boolean): boolean;
    /**
     * 把相机中心移到世界坐标。
     *
     * 点击小地图时，把这里传入 [`Self::minimap_to_world`] 的结果。
     * 非有限值会被忽略。
     *
     * # 参数
     * - `x`, `y`: 世界坐标
     *
     * # 示例（JavaScript）
     * ```js
     * const world = canvas.minimap_to_world(px, py, 200, 150, 8);
     * canvas.set_camera_center(world[0], world[1]);
     * ```
     */
    set_camera_center(x: number, y: number): void;
    /**
     * 设置元素锁定，并记入一条撤销记录。
     *
     * 与图层面板的锁按钮、[`Self::toggle_selection_locked`] 共用 [`infinitecanvas_engine::SetLockedCommand`]。
     * 锁定后组和平移、变换会拒绝修改它以及它的子孙。元素仍留在选择集里，图层面板可以继续选中它；
     * 画布命中、框选和全选会跳过。元素不存在时返回 `false`。已经是目标值时不入栈，仍返回 `true`。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.set_element_locked(id, true);
     * canvas.undo();
     * ```
     */
    set_element_locked(id: bigint, locked: boolean): boolean;
    /**
     * 设置元素可见性，并记入一条撤销记录。
     *
     * 与图层面板的眼睛按钮、[`Self::toggle_selection_visible`] 共用 [`infinitecanvas_engine::SetVisibilityCommand`]。
     * 隐藏组不改子孙自己的标志。元素不存在时返回 `false`。已经是目标值时不入栈，仍返回 `true`。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.set_element_visible(id, false);
     * canvas.undo();
     * ```
     */
    set_element_visible(id: bigint, visible: boolean): boolean;
    /**
     * 显示或隐藏背景网格。
     *
     * 默认可见，和第一次打开时的表现一致。下一帧 `render` 生效，不进撤销栈。
     *
     * # 参数
     * - `visible`: `true` 显示网格
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.set_grid_visible(false);
     * canvas.grid_visible();
     * ```
     */
    set_grid_visible(visible: boolean): void;
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
     * 显示或隐藏标尺和手动参考线。
     *
     * 隐藏后参考线仍留在场景 JSON 里，但不再命中、不再吸附。
     * 正在拖的参考线会取消，场景回到按下前的参考线。
     *
     * # 参数
     * - `visible`: `true` 显示
     */
    set_rulers_visible(visible: boolean): void;
    /**
     * 把补丁写进当前选择并记一条撤销记录。
     *
     * 对象里出现的字段才会修改。`fill` / `stroke` 传 `null` 或 `""` 表示清掉。
     * `strokeWidth`、`opacity`、`fontSize`、`cornerRadius`、`sides`、`innerRadius` 是数字。
     * `cornerRadius` 也可以写成 `corner_radius`。`sides` 也可以写成 `points`，并钳到 3–64。
     * `innerRadius` 也可以写成 `inner_radius`，只作用于星形，并钳到 0.05–0.95。
     * `textColor` 是十六进制颜色。圆角只写到矩形上。
     * `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowOpacity` 是数字，也可以写成 `shadow_offset_x` 等蛇形名字。
     * `shadowColor` 只改 RGB，浓度用 `shadowOpacity`。形状、文本和图片都接受投影。空选择或没有变化时返回 false。
     * `strokeDash` 是数字数组（空数组改回实线）。`strokeCap` / `strokeJoin` 是小写名字。
     * `fillKind` 是 `solid` / `linear` / `radial`。`fillStops` 是 `{offset, color}` 数组。
     * `fillAngle`、`fillRadius` 是数字。`fillCenter` 是 `{x, y}` 或 `[x, y]`。
     * 也接受 `fill_kind`、`fill_stops`、`fill_angle`、`fill_center`、`fill_radius`。
     * 渐变只改形状，描边保持纯色。线型和线帽线结只改形状，文本描边保持原来的光栅描边。
     * 没有任何变化时返回 false，也不入栈。
     */
    set_selection_style(patch: any): boolean;
    /**
     * 开启或关闭拖拽吸附。
     *
     * 关闭后元素跟着指针走，不再产生对齐参考线。
     *
     * # 参数
     * - `enabled`: `true` 开启吸附
     */
    set_snap_enabled(enabled: boolean): void;
    /**
     * 设置吸附阈值（屏幕像素）。
     *
     * 非有限值会被忽略。有效值限制在 0 到 64 之间。
     * 世界距离会除以当前缩放，因此任意缩放下抓取范围保持同样的屏幕像素。
     *
     * # 参数
     * - `threshold_px`: 屏幕像素阈值
     */
    set_snap_threshold(threshold_px: number): void;
    /**
     * 编辑时隐藏 GPU 文本，避免和 DOM 输入框叠两层字。
     */
    set_text_editing(id: bigint, editing: boolean): boolean;
    /**
     * 设置当前工具
     *
     * # 参数
     * - `tool_name`: 工具名称 ("select", "rectangle", "circle", "polygon", "star", "text", "pan", "line", "pen", "eyedropper")
     *
     * 直线快捷键 `l`，画笔快捷键 `p`，文本快捷键 `t`，吸管快捷键 `i`。
     * 吸管不带 Shift、Ctrl 或 Alt，避免和 V / L / P / R / O / T / H 以及翻转键抢键。
     * 切换工具会丢掉尚未抬笔的预览。
     *
     * # 返回
     * 是否成功设置
     */
    set_tool(tool_name: string): boolean;
    /**
     * 把缩放设为给定倍数，视口中心保持不变。
     *
     * 结果会被限制在 0.1 到 10 之间。非有限值会被忽略。
     *
     * # 参数
     * - `zoom`: 缩放倍数，`1` 表示 100%
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.set_zoom(1);
     * ```
     */
    set_zoom(zoom: number): void;
    /**
     * 取走最近一次吸管点击的结果。没有新点击时返回 null。
     *
     * - `hit`：点到了可见叶子
     * - `applied`：颜色已经写进当前选择，并占一条撤销记录
     *
     * 点在空白处时工具保持吸管。点到元素后，工具回到进入吸管之前的那个。
     */
    take_eyedropper_notice(): any;
    /**
     * 取走 T 工具留下的编辑请求。没有请求时返回 `null`。
     */
    take_text_edit_request(): any;
    /**
     * 取出当前缩放档下还没光栅化的文本。
     *
     * 每个元素在同一内容版本和 LOD 上只返回一次。前端用 Canvas2D 画完后调用
     * [`Self::commit_text_raster`]。失败时调用 [`Self::abandon_text_raster`] 以便重试。
     *
     * # 返回
     * 对象数组：`id`, `revision`, `lod`, `rasterScale`, `content`, `fontSizePx`,
     * `color`, `align`, `fontFamily`, `paddingWorld`, `textureId`
     */
    take_text_raster_jobs(): Array<any>;
    /**
     * 切换当前选中元素的锁定，并记入一条撤销记录。
     *
     * 可改的选中项里只要有一个未锁定，就把它们锁定；全部已锁定则解锁。
     * 祖先已锁定、自己未锁定的元素跳过。锁定后仍保持选中，但不能平移或变换。
     * 空选时不入栈。
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.toggle_selection_locked();
     * canvas.undo();
     * ```
     */
    toggle_selection_locked(): boolean;
    /**
     * 切换当前选中元素的可见性，并记入一条撤销记录。
     *
     * 全部已经隐藏时改为显示，否则隐藏仍可见的选中项。选中组时只改组自己的标志，
     * 子孙的命中和框选按有效可见性跳过。空选或不存在的 id 不入栈。
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.toggle_selection_visible();
     * canvas.undo();
     * ```
     */
    toggle_selection_visible(): boolean;
    /**
     * 平移一个元素。目标是组时，子孙的世界坐标一起移动。
     */
    translate_element(id: bigint, dx: number, dy: number): boolean;
    /**
     * 撤销上一个操作
     *
     * # 返回
     * 是否成功撤销
     */
    undo(): boolean;
    /**
     * 解散组。子元素留在场景里。
     */
    ungroup(group_id: bigint): boolean;
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
    /**
     * 绕视口中心按倍数缩放。
     *
     * 与 [`Self::zoom_at`] 同一条钳制路径，上下限是视口的最小/最大缩放，
     * 也是 [`infinitecanvas_engine::camera_for_bounds`] 使用的那一组。
     * 锚点是画布中心，相机中心不动。非有限或非正倍数会被忽略。
     * 键盘放大传入 [`Self::zoom_step`]，缩小传入它的倒数。
     *
     * # 参数
     * - `factor`: 相对倍数
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.zoom_by(canvas.zoom_step());
     * ```
     */
    zoom_by(factor: number): void;
    /**
     * 滚轮和键盘共用的一步缩放倍数。
     *
     * 大于 1 表示放大。Demo 滚轮把指针交给 [`Self::zoom_at`]，
     * 键盘把返回值交给 [`Self::zoom_by`]。
     */
    zoom_step(): number;
    /**
     * 把相机对准全部可见叶子，使整张场景落在视口内并留出 10% 边距。
     *
     * 包围盒与 [`Self::scene_bounds`] 相同。不改元素几何，也不写入撤销栈。
     * 没有可见叶子时把相机放回默认姿态（原点、缩放 1）并返回 `false`。
     * [`Self::fit_scene`] 是同一个命令。
     *
     * # 返回
     * 有内容并完成适应时为 `true`；场景为空（相机已回到默认）时为 `false`。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.zoom_to_fit();
     * ```
     */
    zoom_to_fit(): boolean;
    /**
     * 把相机对准当前选中内容。
     *
     * 只用可见、未锁定的选中元素。选中组时用组根的世界包围盒，不把子元素再展开。
     * 每边留出视口的 10%，然后设置中心和缩放。不改元素几何，也不写入撤销栈。
     * 空选，或过滤后没有可用包围盒时不改相机。
     *
     * # 返回
     * 已经对准选中内容时为 `true`；没有可对准的内容时为 `false`。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.zoom_to_selection();
     * ```
     */
    zoom_to_selection(): boolean;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_infinitecanvas_free: (a: number, b: number) => void;
    readonly infinitecanvas_abandon_text_raster: (a: number, b: bigint) => void;
    readonly infinitecanvas_add_circle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => bigint;
    readonly infinitecanvas_add_image: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => bigint;
    readonly infinitecanvas_add_image_bytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [bigint, number, number];
    readonly infinitecanvas_add_line: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => bigint;
    readonly infinitecanvas_add_path: (a: number, b: number, c: number, d: number, e: number, f: number) => bigint;
    readonly infinitecanvas_add_polygon: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => bigint;
    readonly infinitecanvas_add_rectangle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => bigint;
    readonly infinitecanvas_add_star: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => bigint;
    readonly infinitecanvas_add_text: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => bigint;
    readonly infinitecanvas_align_selection: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_apply_sampled_style_at: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_are_rulers_visible: (a: number) => number;
    readonly infinitecanvas_begin_text_edit_at: (a: number, b: number, c: number) => any;
    readonly infinitecanvas_can_redo: (a: number) => number;
    readonly infinitecanvas_can_undo: (a: number) => number;
    readonly infinitecanvas_clear: (a: number) => void;
    readonly infinitecanvas_clear_scene: (a: number) => void;
    readonly infinitecanvas_clear_selection: (a: number) => void;
    readonly infinitecanvas_commit_style_preview: (a: number) => number;
    readonly infinitecanvas_commit_text_content: (a: number, b: bigint, c: number, d: number) => number;
    readonly infinitecanvas_commit_text_raster: (a: number, b: bigint, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly infinitecanvas_copy_selection_json: (a: number) => [number, number, number, number];
    readonly infinitecanvas_cursor_at: (a: number, b: number, c: number) => [number, number];
    readonly infinitecanvas_debug_clear_events: (a: number) => void;
    readonly infinitecanvas_debug_pending_event_count: (a: number) => number;
    readonly infinitecanvas_delete_guide_at: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_delete_selected_guide: (a: number) => number;
    readonly infinitecanvas_delete_selection: (a: number) => number;
    readonly infinitecanvas_deselect_element: (a: number, b: bigint) => number;
    readonly infinitecanvas_distribute_selection: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_duplicate_selection: (a: number, b: number, c: number) => any;
    readonly infinitecanvas_enter_group: (a: number, b: bigint, c: bigint) => number;
    readonly infinitecanvas_export_json: (a: number) => [number, number, number, number];
    readonly infinitecanvas_export_png: (a: number, b: number) => [number, number, number, number];
    readonly infinitecanvas_export_svg: (a: number, b: number) => [number, number];
    readonly infinitecanvas_fit_scene: (a: number) => number;
    readonly infinitecanvas_flip_selection: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_get_alignment_guides: (a: number) => any;
    readonly infinitecanvas_get_current_tool: (a: number) => [number, number];
    readonly infinitecanvas_get_element_count: (a: number) => number;
    readonly infinitecanvas_get_element_info: (a: number, b: bigint) => any;
    readonly infinitecanvas_get_flags_epoch: (a: number) => bigint;
    readonly infinitecanvas_get_frame_count: (a: number) => bigint;
    readonly infinitecanvas_get_last_draw_calls: (a: number) => number;
    readonly infinitecanvas_get_last_vertex_count: (a: number) => number;
    readonly infinitecanvas_get_manual_guides: (a: number) => any;
    readonly infinitecanvas_get_performance_stats: (a: number) => any;
    readonly infinitecanvas_get_redo_count: (a: number) => number;
    readonly infinitecanvas_get_ruler_major_step: (a: number) => number;
    readonly infinitecanvas_get_ruler_thickness: (a: number) => number;
    readonly infinitecanvas_get_ruler_ticks: (a: number, b: number, c: number) => any;
    readonly infinitecanvas_get_scene_tree: (a: number) => any;
    readonly infinitecanvas_get_selected_ids: (a: number) => any;
    readonly infinitecanvas_get_selection_bounds: (a: number) => any;
    readonly infinitecanvas_get_selection_box_bounds: (a: number) => any;
    readonly infinitecanvas_get_selection_count: (a: number) => number;
    readonly infinitecanvas_get_selection_style: (a: number) => any;
    readonly infinitecanvas_get_snap_threshold: (a: number) => number;
    readonly infinitecanvas_get_state: (a: number) => any;
    readonly infinitecanvas_get_structure_epoch: (a: number) => bigint;
    readonly infinitecanvas_get_text_editor_metrics: (a: number, b: bigint) => any;
    readonly infinitecanvas_get_tool_cursor: (a: number) => [number, number];
    readonly infinitecanvas_get_undo_count: (a: number) => number;
    readonly infinitecanvas_get_viewport_center_x: (a: number) => number;
    readonly infinitecanvas_get_viewport_center_y: (a: number) => number;
    readonly infinitecanvas_get_zoom: (a: number) => number;
    readonly infinitecanvas_grid_visible: (a: number) => number;
    readonly infinitecanvas_group_elements: (a: number, b: any) => bigint;
    readonly infinitecanvas_handle_keyboard_event: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly infinitecanvas_handle_pointer_event: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => void;
    readonly infinitecanvas_handle_wheel_event: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly infinitecanvas_hit_test_screen_point: (a: number, b: number, c: number, d: number) => any;
    readonly infinitecanvas_hit_test_world_point: (a: number, b: number, c: number, d: number) => any;
    readonly infinitecanvas_import_json: (a: number, b: number, c: number) => [number, number];
    readonly infinitecanvas_import_svg: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly infinitecanvas_init: (a: number, b: any) => any;
    readonly infinitecanvas_is_dragging: (a: number) => number;
    readonly infinitecanvas_is_selection_box_active: (a: number) => number;
    readonly infinitecanvas_is_snap_enabled: (a: number) => number;
    readonly infinitecanvas_leave_group: (a: number, b: bigint) => number;
    readonly infinitecanvas_load_stress_scene: (a: number, b: number) => void;
    readonly infinitecanvas_minimap_element_rects: (a: number, b: number, c: number, d: number) => any;
    readonly infinitecanvas_minimap_epoch: (a: number) => bigint;
    readonly infinitecanvas_minimap_to_world: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly infinitecanvas_minimap_viewport_rect: (a: number, b: number, c: number, d: number) => any;
    readonly infinitecanvas_move_selection: (a: number, b: number, c: number) => void;
    readonly infinitecanvas_new: () => [number, number, number];
    readonly infinitecanvas_nudge_selection: (a: number, b: number, c: number, d: number) => number;
    readonly infinitecanvas_pan: (a: number, b: number, c: number) => void;
    readonly infinitecanvas_paste_elements: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly infinitecanvas_pop_temporary_tool: (a: number) => void;
    readonly infinitecanvas_preview_selection_style: (a: number, b: any) => number;
    readonly infinitecanvas_push_hand_tool: (a: number) => void;
    readonly infinitecanvas_redo: (a: number) => number;
    readonly infinitecanvas_remove_element: (a: number, b: bigint) => number;
    readonly infinitecanvas_render: (a: number) => [number, number];
    readonly infinitecanvas_reorder_element: (a: number, b: bigint, c: bigint, d: number) => number;
    readonly infinitecanvas_reorder_selection: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_rotate_selection_quarter: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_sample_style_at: (a: number, b: number, c: number) => any;
    readonly infinitecanvas_scene_bounds: (a: number) => any;
    readonly infinitecanvas_screen_to_world: (a: number, b: number, c: number) => [number, number];
    readonly infinitecanvas_select_all: (a: number) => number;
    readonly infinitecanvas_select_element: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_camera_center: (a: number, b: number, c: number) => void;
    readonly infinitecanvas_set_element_locked: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_element_visible: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_grid_visible: (a: number, b: number) => void;
    readonly infinitecanvas_set_image_alpha: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_rulers_visible: (a: number, b: number) => void;
    readonly infinitecanvas_set_selection_style: (a: number, b: any) => number;
    readonly infinitecanvas_set_snap_enabled: (a: number, b: number) => void;
    readonly infinitecanvas_set_snap_threshold: (a: number, b: number) => void;
    readonly infinitecanvas_set_text_editing: (a: number, b: bigint, c: number) => number;
    readonly infinitecanvas_set_tool: (a: number, b: number, c: number) => number;
    readonly infinitecanvas_set_zoom: (a: number, b: number) => void;
    readonly infinitecanvas_take_eyedropper_notice: (a: number) => any;
    readonly infinitecanvas_take_text_edit_request: (a: number) => any;
    readonly infinitecanvas_take_text_raster_jobs: (a: number) => any;
    readonly infinitecanvas_toggle_selection_locked: (a: number) => number;
    readonly infinitecanvas_toggle_selection_visible: (a: number) => number;
    readonly infinitecanvas_translate_element: (a: number, b: bigint, c: number, d: number) => number;
    readonly infinitecanvas_undo: (a: number) => number;
    readonly infinitecanvas_ungroup: (a: number, b: bigint) => number;
    readonly infinitecanvas_upload_texture: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly infinitecanvas_zoom_at: (a: number, b: number, c: number, d: number) => void;
    readonly infinitecanvas_zoom_by: (a: number, b: number) => void;
    readonly infinitecanvas_zoom_step: (a: number) => number;
    readonly infinitecanvas_zoom_to_fit: (a: number) => number;
    readonly infinitecanvas_zoom_to_selection: (a: number) => number;
    readonly wasm_bindgen_413c9591f4645d29___convert__closures_____invoke___js_sys_9cc5f2a5e2dd9295___Function_fn_wasm_bindgen_413c9591f4645d29___JsValue_____wasm_bindgen_413c9591f4645d29___sys__Undefined___js_sys_9cc5f2a5e2dd9295___Function_fn_wasm_bindgen_413c9591f4645d29___JsValue_____wasm_bindgen_413c9591f4645d29___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_413c9591f4645d29___convert__closures_____invoke___wasm_bindgen_413c9591f4645d29___JsValue__core_608f92abc48d28da___result__Result_____wasm_bindgen_413c9591f4645d29___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_413c9591f4645d29___convert__closures_____invoke___wasm_bindgen_413c9591f4645d29___JsValue______true_: (a: number, b: number, c: any) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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

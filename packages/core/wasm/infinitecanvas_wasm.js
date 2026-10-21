/* @ts-self-types="./infinitecanvas_wasm.d.ts" */

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
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        InfiniteCanvasFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_infinitecanvas_free(ptr, 0);
    }
    /**
     * 光栅化失败后允许下一帧重新取任务。
     * @param {bigint} id
     */
    abandon_text_raster(id) {
        wasm.infinitecanvas_abandon_text_raster(this.__wbg_ptr, id);
    }
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
     * @param {number} x
     * @param {number} y
     * @param {number} radius
     * @param {string | null | undefined} fill_color
     * @param {string | null | undefined} stroke_color
     * @param {number} stroke_width
     * @returns {bigint}
     */
    add_circle(x, y, radius, fill_color, stroke_color, stroke_width) {
        var ptr0 = isLikeNone(fill_color) ? 0 : passStringToWasm0(fill_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(stroke_color) ? 0 : passStringToWasm0(stroke_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_circle(this.__wbg_ptr, x, y, radius, ptr0, len0, ptr1, len1, stroke_width);
        return BigInt.asUintN(64, ret);
    }
    /**
     * 添加画板，并记入撤销栈。
     *
     * 中心是 `(x, y)`。空名称写成 `"画板"`。宽或高不是正数时返回 0，不入栈。
     *
     * # 示例（JavaScript）
     * ```js
     * const id = canvas.add_frame(40, 30, 80, 60, "封面");
     * canvas.rename_frame(id, "内页");
     * ```
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {string | null} [name]
     * @returns {bigint}
     */
    add_frame(x, y, width, height, name) {
        var ptr0 = isLikeNone(name) ? 0 : passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_frame(this.__wbg_ptr, x, y, width, height, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {string} texture_id
     * @returns {bigint}
     */
    add_image(x, y, width, height, texture_id) {
        const ptr0 = passStringToWasm0(texture_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_image(this.__wbg_ptr, x, y, width, height, ptr0, len0);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {Uint8Array} bytes
     * @param {string} mime
     * @param {number} x
     * @param {number} y
     * @returns {bigint}
     */
    add_image_bytes(bytes, mime, x, y) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(mime, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_image_bytes(this.__wbg_ptr, ptr0, len0, ptr1, len1, x, y);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return BigInt.asUintN(64, ret[0]);
    }
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
     * @param {number} x1
     * @param {number} y1
     * @param {number} x2
     * @param {number} y2
     * @param {string | null | undefined} color
     * @param {number} stroke_width
     * @param {boolean} arrow
     * @returns {bigint}
     */
    add_line(x1, y1, x2, y2, color, stroke_width, arrow) {
        var ptr0 = isLikeNone(color) ? 0 : passStringToWasm0(color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_line(this.__wbg_ptr, x1, y1, x2, y2, ptr0, len0, stroke_width, arrow);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {Float32Array} points
     * @param {string | null | undefined} color
     * @param {number} stroke_width
     * @returns {bigint}
     */
    add_path(points, color, stroke_width) {
        const ptr0 = passArrayF32ToWasm0(points, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(color) ? 0 : passStringToWasm0(color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_path(this.__wbg_ptr, ptr0, len0, ptr1, len1, stroke_width);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {number} x
     * @param {number} y
     * @param {number} radius
     * @param {number} sides
     * @param {string | null | undefined} fill_color
     * @param {string | null | undefined} stroke_color
     * @param {number} stroke_width
     * @returns {bigint}
     */
    add_polygon(x, y, radius, sides, fill_color, stroke_color, stroke_width) {
        var ptr0 = isLikeNone(fill_color) ? 0 : passStringToWasm0(fill_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(stroke_color) ? 0 : passStringToWasm0(stroke_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_polygon(this.__wbg_ptr, x, y, radius, sides, ptr0, len0, ptr1, len1, stroke_width);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {string | null | undefined} fill_color
     * @param {string | null | undefined} stroke_color
     * @param {number} stroke_width
     * @returns {bigint}
     */
    add_rectangle(x, y, width, height, fill_color, stroke_color, stroke_width) {
        var ptr0 = isLikeNone(fill_color) ? 0 : passStringToWasm0(fill_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(stroke_color) ? 0 : passStringToWasm0(stroke_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_rectangle(this.__wbg_ptr, x, y, width, height, ptr0, len0, ptr1, len1, stroke_width);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {number} x
     * @param {number} y
     * @param {number} radius
     * @param {number} points
     * @param {number} inner_radius
     * @param {string | null | undefined} fill_color
     * @param {string | null | undefined} stroke_color
     * @param {number} stroke_width
     * @returns {bigint}
     */
    add_star(x, y, radius, points, inner_radius, fill_color, stroke_color, stroke_width) {
        var ptr0 = isLikeNone(fill_color) ? 0 : passStringToWasm0(fill_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(stroke_color) ? 0 : passStringToWasm0(stroke_color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_star(this.__wbg_ptr, x, y, radius, points, inner_radius, ptr0, len0, ptr1, len1, stroke_width);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {number} x
     * @param {number} y
     * @param {string} content
     * @param {number} font_size
     * @param {string | null} [color]
     * @param {string | null} [align]
     * @returns {bigint}
     */
    add_text(x, y, content, font_size, color, align) {
        const ptr0 = passStringToWasm0(content, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        var ptr1 = isLikeNone(color) ? 0 : passStringToWasm0(color, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len1 = WASM_VECTOR_LEN;
        var ptr2 = isLikeNone(align) ? 0 : passStringToWasm0(align, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        var len2 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_add_text(this.__wbg_ptr, x, y, ptr0, len0, font_size, ptr1, len1, ptr2, len2);
        return BigInt.asUintN(64, ret);
    }
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
     * @param {string} mode
     * @returns {boolean}
     */
    align_selection(mode) {
        const ptr0 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_align_selection(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
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
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    apply_sampled_style_at(x, y) {
        const ret = wasm.infinitecanvas_apply_sampled_style_at(this.__wbg_ptr, x, y);
        return ret !== 0;
    }
    /**
     * 标尺和手动参考线是否显示。
     * @returns {boolean}
     */
    are_rulers_visible() {
        const ret = wasm.infinitecanvas_are_rulers_visible(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 双击命中文本时返回编辑框几何。没命中文本返回 `null`。
     *
     * # 参数
     * - `x`, `y`: 屏幕像素坐标（和 `handle_pointer_event` 用的是同一套）
     * @param {number} x
     * @param {number} y
     * @returns {any}
     */
    begin_text_edit_at(x, y) {
        const ret = wasm.infinitecanvas_begin_text_edit_at(this.__wbg_ptr, x, y);
        return ret;
    }
    /**
     * 对当前选择做布尔运算，并记入一条撤销记录。
     *
     * `op` 为 `union`、`subtract`、`intersect` 或 `xor`。相减是最上层减去其余形状（前 − 后）。
     * 结果是一条路径，替换参与运算的叶子，样式取最上层。空选、单选或不支持的选择不入栈。
     * 成功后选择集只留下结果路径。
     *
     * # 参数
     * - `op`: `union` / `subtract` / `intersect` / `xor`
     *
     * # 返回
     * 是否写入了撤销栈
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.boolean_selection("subtract");
     * canvas.undo();
     * ```
     * @param {string} op
     * @returns {boolean}
     */
    boolean_selection(op) {
        const ptr0 = passStringToWasm0(op, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_boolean_selection(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * 当前选择是否至少有两个可做布尔运算的叶子。
     *
     * 锁定、隐藏、组、直线、文本、图片和开口路径不算。工具栏用它决定按钮是否可点。
     *
     * # 返回
     * 可以执行并集 / 相减 / 交集 / 异或时为 true
     *
     * # 示例（JavaScript）
     * ```js
     * if (canvas.boolean_selection_ready()) {
     *   canvas.boolean_selection("union");
     * }
     * ```
     * @returns {boolean}
     */
    boolean_selection_ready() {
        const ret = wasm.infinitecanvas_boolean_selection_ready(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 是否可以重做
     * @returns {boolean}
     */
    can_redo() {
        const ret = wasm.infinitecanvas_can_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 是否可以撤销
     * @returns {boolean}
     */
    can_undo() {
        const ret = wasm.infinitecanvas_can_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 清空场景中所有元素
     *
     * # JavaScript 示例
     * ```js
     * canvas.clear(); // 清空所有元素
     * ```
     */
    clear() {
        wasm.infinitecanvas_clear(this.__wbg_ptr);
    }
    /**
     * 清空场景
     */
    clear_scene() {
        wasm.infinitecanvas_clear_scene(this.__wbg_ptr);
    }
    clear_selection() {
        wasm.infinitecanvas_clear_selection(this.__wbg_ptr);
    }
    /**
     * 把进行中的裁剪预览收成一条撤销记录。
     *
     * 场景已经是预览后的值，所以只入栈、不再执行。没有变化时返回 false。
     * @returns {boolean}
     */
    commit_image_crop() {
        const ret = wasm.infinitecanvas_commit_image_crop(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 把进行中的样式预览收成一条撤销记录。
     *
     * 场景已经是预览后的值，所以只入栈、不再执行。没有变化时返回 false。
     * @returns {boolean}
     */
    commit_style_preview() {
        const ret = wasm.infinitecanvas_commit_style_preview(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 提交编辑内容。
     *
     * 内容有变化时写入撤销栈。只剩空白时删除元素，撤销会把原文放回来。
     *
     * # 返回
     * 元素存在并且这次调用处理完了编辑状态。
     * @param {bigint} id
     * @param {string} content
     * @returns {boolean}
     */
    commit_text_content(id, content) {
        const ptr0 = passStringToWasm0(content, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_commit_text_content(this.__wbg_ptr, id, ptr0, len0);
        return ret !== 0;
    }
    /**
     * 把已经上传的纹理绑定到文本元素。
     *
     * `revision` 必须等于取任务时的版本，否则结果被丢弃并删除这张纹理。
     * `world_width` / `world_height` 是纹理像素除以 `rasterScale`，用来固定世界尺寸。
     *
     * # 返回
     * 是否接受了这张纹理。
     * @param {bigint} id
     * @param {number} revision
     * @param {number} lod
     * @param {string} texture_id
     * @param {number} world_width
     * @param {number} world_height
     * @returns {boolean}
     */
    commit_text_raster(id, revision, lod, texture_id, world_width, world_height) {
        const ptr0 = passStringToWasm0(texture_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_commit_text_raster(this.__wbg_ptr, id, revision, lod, ptr0, len0, world_width, world_height);
        return ret !== 0;
    }
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
     * @returns {string}
     */
    copy_selection_json() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.infinitecanvas_copy_selection_json(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * 裁剪手柄是否打开。
     * @returns {boolean}
     */
    crop_mode() {
        const ret = wasm.infinitecanvas_crop_mode(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 指针下的光标。选择工具命中缩放/旋转手柄时返回对应光标。
     *
     * # 参数
     * - `x`, `y`: 屏幕坐标（与指针事件相同）
     * @param {number} x
     * @param {number} y
     * @returns {string}
     */
    cursor_at(x, y) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.infinitecanvas_cursor_at(this.__wbg_ptr, x, y);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    debug_clear_events() {
        wasm.infinitecanvas_debug_clear_events(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    debug_pending_event_count() {
        const ret = wasm.infinitecanvas_debug_pending_event_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 双击命中参考线时删除它。
     *
     * # 参数
     * - `x`: 画布像素 x
     * - `y`: 画布像素 y
     *
     * # 返回
     * 命中并删除时为 `true`。标尺隐藏时为 `false`。
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    delete_guide_at(x, y) {
        const ret = wasm.infinitecanvas_delete_guide_at(this.__wbg_ptr, x, y);
        return ret !== 0;
    }
    /**
     * 删除当前选中的参考线，并记入撤销栈。
     *
     * # 返回
     * 确实删掉了一条参考线时为 `true`。
     * @returns {boolean}
     */
    delete_selected_guide() {
        const ret = wasm.infinitecanvas_delete_selected_guide(this.__wbg_ptr);
        return ret !== 0;
    }
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
     * @returns {number}
     */
    delete_selection() {
        const ret = wasm.infinitecanvas_delete_selection(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {bigint} element_id
     * @returns {boolean}
     */
    deselect_element(element_id) {
        const ret = wasm.infinitecanvas_deselect_element(this.__wbg_ptr, element_id);
        return ret !== 0;
    }
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
     * @param {string} axis
     * @returns {boolean}
     */
    distribute_selection(axis) {
        const ptr0 = passStringToWasm0(axis, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_distribute_selection(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
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
     * @param {number} dx
     * @param {number} dy
     * @returns {Array<any>}
     */
    duplicate_selection(dx, dy) {
        const ret = wasm.infinitecanvas_duplicate_selection(this.__wbg_ptr, dx, dy);
        return ret;
    }
    /**
     * 把元素放进组，世界坐标不变。
     * @param {bigint} element_id
     * @param {bigint} group_id
     * @returns {boolean}
     */
    enter_group(element_id, group_id) {
        const ret = wasm.infinitecanvas_enter_group(this.__wbg_ptr, element_id, group_id);
        return ret !== 0;
    }
    /**
     * 把一张画板导出为 PNG。区域是画板的世界包围盒，不含额外留白。
     *
     * 只绘制这张画板和它的子孙。空画板仍按矩形尺寸导出。
     * `scale` 缺省 `1.0`；与 [`Self::export_png`] 相同的倍率语义。
     * @param {bigint} frame_id
     * @param {number | null} [scale]
     * @returns {Uint8Array}
     */
    export_frame_png(frame_id, scale) {
        const ret = wasm.infinitecanvas_export_frame_png(this.__wbg_ptr, frame_id, isLikeNone(scale) ? Number.MAX_SAFE_INTEGER : Math.fround(scale));
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * 把一张画板导出为 SVG。`viewBox` 是画板包围盒，超出部分隐藏。
     * @param {bigint} frame_id
     * @returns {string}
     */
    export_frame_svg(frame_id) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.infinitecanvas_export_frame_svg(this.__wbg_ptr, frame_id);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
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
     * @returns {string}
     */
    export_json() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.infinitecanvas_export_json(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * 把当前视口或选中区域导出为 PNG
     *
     * # 参数
     * - `selection_only`: `true` 时导出选中元素的包围盒；`false` 时导出当前视口
     * - `scale`: 导出倍率，缺省 `1.0`（与旧行为一致）。常用 `1` / `2` / `3`：
     *   像素宽高 ≈ 逻辑区域 × 视口缩放 × scale。2x 面积约 4 倍。按更高分辨率
     *   重新采样，不是把 1x 位图放大。空选区报错；空视口导出空白 PNG。
     *
     * # 返回
     * RGBA PNG 字节。直线、箭头和自由路径按线宽栅格化。文本用 font8x8 点阵，
     * 没有点阵的字符画方框。资源表里有像素的图片按显示矩形采样；只有 GPU 纹理
     * id 的图片仍是纯色占位。分组本身不绘制；隐藏的组会连同子孙一起跳过。
     *
     * # 错误
     * 没有选中元素、区域为空、倍率非法或编码失败
     * @param {boolean} selection_only
     * @param {number | null} [scale]
     * @returns {Uint8Array}
     */
    export_png(selection_only, scale) {
        const ret = wasm.infinitecanvas_export_png(this.__wbg_ptr, selection_only, isLikeNone(scale) ? Number.MAX_SAFE_INTEGER : Math.fround(scale));
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
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
     * @param {boolean} selected_only
     * @returns {string}
     */
    export_svg(selected_only) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.infinitecanvas_export_svg(this.__wbg_ptr, selected_only);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * [`Self::zoom_to_fit`] 的别名。
     * @returns {boolean}
     */
    fit_scene() {
        const ret = wasm.infinitecanvas_fit_scene(this.__wbg_ptr);
        return ret !== 0;
    }
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
     * @param {string} axis
     * @returns {boolean}
     */
    flip_selection(axis) {
        const ptr0 = passStringToWasm0(axis, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_flip_selection(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * 画板名称。不是画板时返回 `null`。
     * @param {bigint} id
     * @returns {string | undefined}
     */
    frame_name(id) {
        const ret = wasm.infinitecanvas_frame_name(this.__wbg_ptr, id);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]);
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * 当前生效的对齐参考线。
     *
     * 只在吸附开启且正在拖拽、并且选区进入阈值时非空。
     * 每条记录包含 `axis`（`"horizontal"` 或 `"vertical"`）和 `position`（世界坐标）。
     *
     * # 返回
     * 参考线数组。没有参考线时为空数组。
     * @returns {Array<any>}
     */
    get_alignment_guides() {
        const ret = wasm.infinitecanvas_get_alignment_guides(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取当前工具名称
     * @returns {string}
     */
    get_current_tool() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.infinitecanvas_get_current_tool(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * 获取元素数量
     * @returns {number}
     */
    get_element_count() {
        const ret = wasm.infinitecanvas_get_element_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 获取元素信息
     *
     * # 返回
     * 如果元素存在，返回元素信息对象；否则返回 null
     * @param {bigint} id
     * @returns {any}
     */
    get_element_info(id) {
        const ret = wasm.infinitecanvas_get_element_info(this.__wbg_ptr, id);
        return ret;
    }
    /**
     * 可见性或锁定的版本。图层面板用它重读眼睛和锁，不代表场景结构变了。
     * @returns {bigint}
     */
    get_flags_epoch() {
        const ret = wasm.infinitecanvas_get_flags_epoch(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * 获取帧计数
     * @returns {bigint}
     */
    get_frame_count() {
        const ret = wasm.infinitecanvas_get_frame_count(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * 读取当前选择里图片的裁剪窗口。
     *
     * 没有未锁定的图片时 `supported` 为 false。多选某一边不一致时对应的 `*Mixed` 为 true，
     * 数值为 null。`x` / `y` / `width` / `height` 都是 0–1，整张图是 `0,0,1,1`。
     * 组会展开到里面的图片。
     *
     * # 示例（JavaScript）
     * ```js
     * const crop = canvas.get_image_crop();
     * if (crop.supported && !crop.widthMixed) widthInput.value = crop.width;
     * ```
     * @returns {any}
     */
    get_image_crop() {
        const ret = wasm.infinitecanvas_get_image_crop(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取上一帧绘制调用数
     * @returns {number}
     */
    get_last_draw_calls() {
        const ret = wasm.infinitecanvas_get_last_draw_calls(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 获取上一帧顶点数
     * @returns {number}
     */
    get_last_vertex_count() {
        const ret = wasm.infinitecanvas_get_last_vertex_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 场景里的手动参考线。
     *
     * 每条记录含 `id`、`axis`（`"horizontal"` / `"vertical"`）、`position`（世界坐标）
     * 和 `selected`。隐藏标尺时仍返回数据，绘制由调用方决定。
     * @returns {Array<any>}
     */
    get_manual_guides() {
        const ret = wasm.infinitecanvas_get_manual_guides(this.__wbg_ptr);
        return ret;
    }
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
     * @returns {any}
     */
    get_performance_stats() {
        const ret = wasm.infinitecanvas_get_performance_stats(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取重做栈大小
     * @returns {number}
     */
    get_redo_count() {
        const ret = wasm.infinitecanvas_get_redo_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 主刻度的世界步长。缩放越大，步长越小。
     * @returns {number}
     */
    get_ruler_major_step() {
        const ret = wasm.infinitecanvas_get_ruler_major_step(this.__wbg_ptr);
        return ret;
    }
    /**
     * 标尺厚度（屏幕像素）。
     * @returns {number}
     */
    get_ruler_thickness() {
        const ret = wasm.infinitecanvas_get_ruler_thickness(this.__wbg_ptr);
        return ret;
    }
    /**
     * 一条标尺上的刻度。
     *
     * `axis` 为 `"horizontal"`（顶边，世界 x）或 `"vertical"`（左边，世界 y）。
     * 每条记录含 `world`、`screen` 和 `major`。
     *
     * # 参数
     * - `axis`: `"horizontal"` 或 `"vertical"`
     * @param {string} axis
     * @returns {Array<any>}
     */
    get_ruler_ticks(axis) {
        const ptr0 = passStringToWasm0(axis, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_get_ruler_ticks(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * 场景树。`roots` 里每个节点含 `id`、`elementType`、`parentId`、`visible`、`locked`、`zIndex`、`children`。
     * @returns {any}
     */
    get_scene_tree() {
        const ret = wasm.infinitecanvas_get_scene_tree(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Array<any>}
     */
    get_selected_ids() {
        const ret = wasm.infinitecanvas_get_selected_ids(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取选中元素的边界框
     *
     * # 返回
     * 如果有选中元素，返回 { x, y, width, height }；否则返回 null
     * @returns {any}
     */
    get_selection_bounds() {
        const ret = wasm.infinitecanvas_get_selection_bounds(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取当前框选矩形（如果正在框选）
     *
     * # 返回
     * 如果正在框选，返回 { x, y, width, height }；否则返回 null
     * @returns {any}
     */
    get_selection_box_bounds() {
        const ret = wasm.infinitecanvas_get_selection_box_bounds(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_selection_count() {
        const ret = wasm.infinitecanvas_get_selection_count(this.__wbg_ptr);
        return ret >>> 0;
    }
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
     *   `strokeJoin` 为 `miter` / `round` / `bevel`，`strokeAlign` 为 `center` / `inside` / `outside`。
     *   这四项只统计形状，文本描边不参与
     * - `cornerRadius` 只在选择里有矩形时出现（`hasRect` / `cornerRadiusSupported`）。多选半径不一致时 `cornerRadiusMixed` 为 true
     * - `sides` 是多边形边数或星形角数（`hasPolygon` / `hasStar` / `sidesSupported`）。`innerRadius` 只统计星形
     * - `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowOpacity` 是数字。`shadowColor` 是 `#rrggbbaa`（面板只用 RGB，浓度在 `shadowOpacity`）。没有形状、文本或图片时 `shadowSupported` 为 false
     * - `fillKind`：`solid` / `linear` / `radial`。`fillStops` 是 `{offset, color}` 数组。
     *   `fillAngle` 为度，`fillCenter` 为 `{x, y}`，`fillRadius` 为局部半径（0 表示自动）。
     *   这几项只统计形状。多选不一致时对应的 `*Mixed` 为 true
     * - `blendMode` 为 `normal` / `multiply` / `screen` / `overlay` / `darken` / `lighten`。
     *   形状、文本、图片和画板都参与。多选不一致时 `blendModeMixed` 为 true。未写出时不改模糊
     * - `blurRadius` 是图层高斯模糊的标准差。没有形状、文本或图片时 `blurSupported` 为 false。多选不一致时 `blurRadiusMixed` 为 true。画板算形状。未写出时不改渐变或混合模式
     * - `textAlign` 为 `left` / `center` / `right`。`fontWeight` 为 `regular` / `bold`。只统计文本。多选不一致时对应的 `*Mixed` 为 true
     * - `startMarker` / `endMarker`：`none` / `arrow` / `triangle`。仅直线（`hasLine`）。多选不一致时对应 `*Mixed` 为 true
     *
     * # 示例（JavaScript）
     * ```js
     * const style = canvas.get_selection_style();
     * if (style.hasShape && !style.fillMixed) colorInput.value = style.fill.slice(0, 7);
     * ```
     * @returns {any}
     */
    get_selection_style() {
        const ret = wasm.infinitecanvas_get_selection_style(this.__wbg_ptr);
        return ret;
    }
    /**
     * 读取当前选择的变换数值。
     *
     * - `supported`：有未锁定的选中根
     * - `sizeSupported`：选择里有非直线目标，可改 W/H
     * - `x` / `y`：本地原点世界坐标（`transform.translation`）
     * - `width` / `height`：世界尺寸 `|local| * |scale|`；直线不参与
     * - `rotation`：旋转角度（度），与旋转手柄同一字段
     * - 多选不一致时对应 `*Mixed` 为 true，数值为 null
     *
     * # 示例（JavaScript）
     * ```js
     * const t = canvas.get_selection_transform();
     * if (t.supported && !t.xMixed) xInput.value = t.x;
     * ```
     * @returns {any}
     */
    get_selection_transform() {
        const ret = wasm.infinitecanvas_get_selection_transform(this.__wbg_ptr);
        return ret;
    }
    /**
     * 当前吸附阈值（屏幕像素）。
     * @returns {number}
     */
    get_snap_threshold() {
        const ret = wasm.infinitecanvas_get_snap_threshold(this.__wbg_ptr);
        return ret;
    }
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
     * @returns {any}
     */
    get_state() {
        const ret = wasm.infinitecanvas_get_state(this.__wbg_ptr);
        return ret;
    }
    /**
     * 场景结构版本。增删元素或大纲重排后变化，图层面板用它决定是否重读场景树。
     * @returns {bigint}
     */
    get_structure_epoch() {
        const ret = wasm.infinitecanvas_get_structure_epoch(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * 查询文本在屏幕上的位置，编辑框跟着平移和缩放移动。
     * @param {bigint} id
     * @returns {any}
     */
    get_text_editor_metrics(id) {
        const ret = wasm.infinitecanvas_get_text_editor_metrics(this.__wbg_ptr, id);
        return ret;
    }
    /**
     * 获取当前工具的光标样式
     * @returns {string}
     */
    get_tool_cursor() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.infinitecanvas_get_tool_cursor(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * 获取撤销栈大小
     * @returns {number}
     */
    get_undo_count() {
        const ret = wasm.infinitecanvas_get_undo_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 获取视口中心 X
     * @returns {number}
     */
    get_viewport_center_x() {
        const ret = wasm.infinitecanvas_get_viewport_center_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取视口中心 Y
     * @returns {number}
     */
    get_viewport_center_y() {
        const ret = wasm.infinitecanvas_get_viewport_center_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * 获取视口缩放
     * @returns {number}
     */
    get_zoom() {
        const ret = wasm.infinitecanvas_get_zoom(this.__wbg_ptr);
        return ret;
    }
    /**
     * 背景网格是否可见。
     *
     * # 返回
     * 可见时为 `true`
     * @returns {boolean}
     */
    grid_visible() {
        const ret = wasm.infinitecanvas_grid_visible(this.__wbg_ptr);
        return ret !== 0;
    }
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
     * @param {Array<any>} ids
     * @returns {bigint}
     */
    group_elements(ids) {
        const ret = wasm.infinitecanvas_group_elements(this.__wbg_ptr, ids);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @param {string} kind
     * @param {string} key
     * @param {string} code
     * @param {boolean} repeat
     * @param {boolean} shift_key
     * @param {boolean} ctrl_key
     * @param {boolean} alt_key
     * @param {boolean} meta_key
     */
    handle_keyboard_event(kind, key, code, repeat, shift_key, ctrl_key, alt_key, meta_key) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(code, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        wasm.infinitecanvas_handle_keyboard_event(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, repeat, shift_key, ctrl_key, alt_key, meta_key);
    }
    /**
     * @param {string} kind
     * @param {number} pointer_id
     * @param {number} x
     * @param {number} y
     * @param {number} movement_x
     * @param {number} movement_y
     * @param {number} button
     * @param {number} buttons_mask
     * @param {boolean} shift_key
     * @param {boolean} ctrl_key
     * @param {boolean} alt_key
     * @param {boolean} meta_key
     */
    handle_pointer_event(kind, pointer_id, x, y, movement_x, movement_y, button, buttons_mask, shift_key, ctrl_key, alt_key, meta_key) {
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.infinitecanvas_handle_pointer_event(this.__wbg_ptr, ptr0, len0, pointer_id, x, y, movement_x, movement_y, button, buttons_mask, shift_key, ctrl_key, alt_key, meta_key);
    }
    /**
     * @param {number} delta_x
     * @param {number} delta_y
     * @param {number} delta_mode
     * @param {boolean} shift_key
     * @param {boolean} ctrl_key
     * @param {boolean} alt_key
     * @param {boolean} meta_key
     */
    handle_wheel_event(delta_x, delta_y, delta_mode, shift_key, ctrl_key, alt_key, meta_key) {
        wasm.infinitecanvas_handle_wheel_event(this.__wbg_ptr, delta_x, delta_y, delta_mode, shift_key, ctrl_key, alt_key, meta_key);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number | null} [max_results]
     * @returns {Array<any>}
     */
    hit_test_screen_point(x, y, max_results) {
        const ret = wasm.infinitecanvas_hit_test_screen_point(this.__wbg_ptr, x, y, isLikeNone(max_results) ? Number.MAX_SAFE_INTEGER : (max_results) >>> 0);
        return ret;
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number | null} [max_results]
     * @returns {Array<any>}
     */
    hit_test_world_point(x, y, max_results) {
        const ret = wasm.infinitecanvas_hit_test_world_point(this.__wbg_ptr, x, y, isLikeNone(max_results) ? Number.MAX_SAFE_INTEGER : (max_results) >>> 0);
        return ret;
    }
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
     * @param {string} json
     */
    import_json(json) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_import_json(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {string} svg
     * @param {number} x
     * @param {number} y
     * @returns {Array<any>}
     */
    import_svg(svg, x, y) {
        const ptr0 = passStringToWasm0(svg, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_import_svg(this.__wbg_ptr, ptr0, len0, x, y);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * 初始化画布（异步，因为 WebGPU 初始化需要异步）
     *
     * # 参数
     * - `canvas`: HTMLCanvasElement
     *
     * # 返回
     * - `Ok(())`: WebGPU 初始化成功
     * - `Err(JsValue)`: WebGPU 不可用或初始化失败
     * @param {HTMLCanvasElement} canvas
     * @returns {Promise<void>}
     */
    init(canvas) {
        const ret = wasm.infinitecanvas_init(this.__wbg_ptr, canvas);
        return ret;
    }
    /**
     * 检查是否正在拖动
     * @returns {boolean}
     */
    is_dragging() {
        const ret = wasm.infinitecanvas_is_dragging(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 检查是否正在框选
     * @returns {boolean}
     */
    is_selection_box_active() {
        const ret = wasm.infinitecanvas_is_selection_box_active(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 拖拽吸附是否开启。
     * @returns {boolean}
     */
    is_snap_enabled() {
        const ret = wasm.infinitecanvas_is_snap_enabled(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 把元素移出当前组，挂回组的父节点或根。
     * @param {bigint} element_id
     * @returns {boolean}
     */
    leave_group(element_id) {
        const ret = wasm.infinitecanvas_leave_group(this.__wbg_ptr, element_id);
        return ret !== 0;
    }
    /**
     * 用 `count` 个矩形替换当前场景，供压测页切换 1k/10k/100k。
     *
     * 元素按网格铺开。第一次渲染会建立顶点缓存，之后平移/缩放只更新 uniform。
     * @param {number} count
     */
    load_stress_scene(count) {
        wasm.infinitecanvas_load_stress_scene(this.__wbg_ptr, count);
    }
    /**
     * 元素在小地图上的简化外框。
     *
     * 同一几何版本和尺寸会复用缓存，避免 10 万元素时每帧重扫。
     * 返回 `Float32Array`，每 4 个数是一个像素矩形 `x, y, width, height`。
     *
     * # 参数
     * - `width`, `height`: 小地图像素尺寸，例如 200×150
     * - `padding`: 四周留白（像素）
     * @param {number} width
     * @param {number} height
     * @param {number} padding
     * @returns {Float32Array}
     */
    minimap_element_rects(width, height, padding) {
        const ret = wasm.infinitecanvas_minimap_element_rects(this.__wbg_ptr, width, height, padding);
        return ret;
    }
    /**
     * 小地图几何版本。
     *
     * 与 [`infinitecanvas_engine::Scene::geometry_epoch`] 相同。
     * 元素增删或几何变化时递增，平移/缩放相机不变。
     * 页面应在版本变化时才调用 [`Self::minimap_element_rects`]，
     * 相机变化只调用 [`Self::minimap_viewport_rect`]。
     * @returns {bigint}
     */
    minimap_epoch() {
        const ret = wasm.infinitecanvas_minimap_epoch(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * 小地图像素坐标反算世界坐标。
     *
     * 点击处要成为相机中心时，把返回值交给 [`Self::set_camera_center`]。
     * 留白和内容区用的是同一个线性映射。
     *
     * # 返回
     * `Float32Array` `[x, y]`。
     * @param {number} x
     * @param {number} y
     * @param {number} width
     * @param {number} height
     * @param {number} padding
     * @returns {Float32Array}
     */
    minimap_to_world(x, y, width, height, padding) {
        const ret = wasm.infinitecanvas_minimap_to_world(this.__wbg_ptr, x, y, width, height, padding);
        return ret;
    }
    /**
     * 当前视口在小地图上的像素矩形。
     *
     * 映射跟元素外框共用缓存，本身不扫描场景。
     *
     * # 返回
     * `Float32Array` `[x, y, width, height]`。
     * @param {number} width
     * @param {number} height
     * @param {number} padding
     * @returns {Float32Array}
     */
    minimap_viewport_rect(width, height, padding) {
        const ret = wasm.infinitecanvas_minimap_viewport_rect(this.__wbg_ptr, width, height, padding);
        return ret;
    }
    /**
     * 移动选中的元素
     *
     * # 参数
     * - `dx`: X 方向移动量（世界坐标）
     * - `dy`: Y 方向移动量（世界坐标）
     * @param {number} dx
     * @param {number} dy
     */
    move_selection(dx, dy) {
        wasm.infinitecanvas_move_selection(this.__wbg_ptr, dx, dy);
    }
    /**
     * 创建新的画布实例（构造器，实际初始化在 init() 中完成）
     *
     * # 返回
     * - `Ok(InfiniteCanvas)`: 成功创建
     * - `Err(JsValue)`: 初始化失败
     */
    constructor() {
        const ret = wasm.infinitecanvas_new();
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        InfiniteCanvasFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
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
     * @param {string} direction
     * @param {boolean} shift
     * @returns {boolean}
     */
    nudge_selection(direction, shift) {
        const ptr0 = passStringToWasm0(direction, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_nudge_selection(this.__wbg_ptr, ptr0, len0, shift);
        return ret !== 0;
    }
    /**
     * 平移视口
     *
     * # 参数
     * - `dx, dy`: 移动量（世界坐标）
     * @param {number} dx
     * @param {number} dy
     */
    pan(dx, dy) {
        wasm.infinitecanvas_pan(this.__wbg_ptr, dx, dy);
    }
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
     * @param {string} json
     * @param {number} dx
     * @param {number} dy
     * @param {number | null} [focus_x]
     * @param {number | null} [focus_y]
     * @returns {Array<any>}
     */
    paste_elements(json, dx, dy, focus_x, focus_y) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_paste_elements(this.__wbg_ptr, ptr0, len0, dx, dy, isLikeNone(focus_x) ? Number.MAX_SAFE_INTEGER : Math.fround(focus_x), isLikeNone(focus_y) ? Number.MAX_SAFE_INTEGER : Math.fround(focus_y));
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * 恢复到之前的工具
     */
    pop_temporary_tool() {
        wasm.infinitecanvas_pop_temporary_tool(this.__wbg_ptr);
    }
    /**
     * 预览裁剪。第一次调用记下基线，之后每次都从基线重放。
     *
     * 不进入撤销栈。松手后调用 [`Self::commit_image_crop`]。
     * @param {any} patch
     * @returns {boolean}
     */
    preview_image_crop(patch) {
        const ret = wasm.infinitecanvas_preview_image_crop(this.__wbg_ptr, patch);
        return ret !== 0;
    }
    /**
     * 预览样式。第一次调用记下基线，之后每次都从基线重放。
     *
     * 不进入撤销栈。松手后调用 [`Self::commit_style_preview`]。
     * @param {any} patch
     * @returns {boolean}
     */
    preview_selection_style(patch) {
        const ret = wasm.infinitecanvas_preview_selection_style(this.__wbg_ptr, patch);
        return ret !== 0;
    }
    /**
     * 临时切换到手型工具（按住空格）
     */
    push_hand_tool() {
        wasm.infinitecanvas_push_hand_tool(this.__wbg_ptr);
    }
    /**
     * 重做上一个撤销的操作
     *
     * # 返回
     * 是否成功重做
     * @returns {boolean}
     */
    redo() {
        const ret = wasm.infinitecanvas_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 删除元素
     *
     * # 参数
     * - `id`: 元素 ID
     *
     * # 返回
     * 是否成功删除
     * @param {bigint} id
     * @returns {boolean}
     */
    remove_element(id) {
        const ret = wasm.infinitecanvas_remove_element(this.__wbg_ptr, id);
        return ret !== 0;
    }
    /**
     * 重命名画板。不是画板，或名称没变时返回 false，不入栈。
     * @param {bigint} id
     * @param {string} name
     * @returns {boolean}
     */
    rename_frame(id, name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_rename_frame(this.__wbg_ptr, id, ptr0, len0);
        return ret !== 0;
    }
    /**
     * 渲染一帧
     *
     * # 返回
     * - `Ok(())`: 渲染成功
     * - `Err(JsValue)`: 渲染失败（渲染器未初始化）
     */
    render() {
        const ret = wasm.infinitecanvas_render(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
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
     * @param {bigint} id
     * @param {bigint} parent_id
     * @param {number} index
     * @returns {boolean}
     */
    reorder_element(id, parent_id, index) {
        const ret = wasm.infinitecanvas_reorder_element(this.__wbg_ptr, id, parent_id, index);
        return ret !== 0;
    }
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
     * @param {string} action
     * @returns {boolean}
     */
    reorder_selection(action) {
        const ptr0 = passStringToWasm0(action, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_reorder_selection(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * 把元素移进画板。空列表、目标不是画板、或没有元素移动时返回 false，不入栈。
     * @param {Array<any>} ids
     * @param {bigint} frame_id
     * @returns {boolean}
     */
    reparent_into_frame(ids, frame_id) {
        const ret = wasm.infinitecanvas_reparent_into_frame(this.__wbg_ptr, ids, frame_id);
        return ret !== 0;
    }
    /**
     * 把当前选择移进画板。空选返回 false，不入栈。
     * @param {bigint} frame_id
     * @returns {boolean}
     */
    reparent_selection_into_frame(frame_id) {
        const ret = wasm.infinitecanvas_reparent_selection_into_frame(this.__wbg_ptr, frame_id);
        return ret !== 0;
    }
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
     * @param {string} dir
     * @returns {boolean}
     */
    rotate_selection_quarter(dir) {
        const ptr0 = passStringToWasm0(dir, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_rotate_selection_quarter(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
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
     * @param {number} x
     * @param {number} y
     * @returns {any}
     */
    sample_style_at(x, y) {
        const ret = wasm.infinitecanvas_sample_style_at(this.__wbg_ptr, x, y);
        return ret;
    }
    /**
     * 可见叶子的世界包围盒。
     *
     * 组不计入，隐藏元素及其子孙也不计入。空场景返回 `null`。
     *
     * # 返回
     * `{ x, y, width, height }`，或 `null`。
     * @returns {any}
     */
    scene_bounds() {
        const ret = wasm.infinitecanvas_scene_bounds(this.__wbg_ptr);
        return ret;
    }
    /**
     * 屏幕坐标转世界坐标。拖放和粘贴图片时用它定位左上角。
     *
     * # 返回
     * `[x, y]`
     * @param {number} x
     * @param {number} y
     * @returns {Float32Array}
     */
    screen_to_world(x, y) {
        const ret = wasm.infinitecanvas_screen_to_world(this.__wbg_ptr, x, y);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
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
     * @returns {number}
     */
    select_all() {
        const ret = wasm.infinitecanvas_select_all(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {bigint} element_id
     * @param {boolean} additive
     * @returns {boolean}
     */
    select_element(element_id, additive) {
        const ret = wasm.infinitecanvas_select_element(this.__wbg_ptr, element_id, additive);
        return ret !== 0;
    }
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
     * @param {number} x
     * @param {number} y
     */
    set_camera_center(x, y) {
        wasm.infinitecanvas_set_camera_center(this.__wbg_ptr, x, y);
    }
    /**
     * 打开或关闭裁剪手柄。
     *
     * 只在选中单张未锁定的可见图片时，缩放手柄改裁剪而不是世界变换。
     * 没有新的快捷键。关闭时先提交未结束的手柄拖动。
     * @param {boolean} enabled
     */
    set_crop_mode(enabled) {
        wasm.infinitecanvas_set_crop_mode(this.__wbg_ptr, enabled);
    }
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
     * @param {bigint} id
     * @param {boolean} locked
     * @returns {boolean}
     */
    set_element_locked(id, locked) {
        const ret = wasm.infinitecanvas_set_element_locked(this.__wbg_ptr, id, locked);
        return ret !== 0;
    }
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
     * @param {bigint} id
     * @param {boolean} visible
     * @returns {boolean}
     */
    set_element_visible(id, visible) {
        const ret = wasm.infinitecanvas_set_element_visible(this.__wbg_ptr, id, visible);
        return ret !== 0;
    }
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
     * @param {boolean} visible
     */
    set_grid_visible(visible) {
        wasm.infinitecanvas_set_grid_visible(this.__wbg_ptr, visible);
    }
    /**
     * 设置图片透明度
     *
     * # 参数
     * - `image_id`: 图片元素 ID
     * - `alpha`: 透明度 [0.0 - 1.0]
     *
     * # 返回
     * 是否成功设置
     * @param {bigint} image_id
     * @param {number} alpha
     * @returns {boolean}
     */
    set_image_alpha(image_id, alpha) {
        const ret = wasm.infinitecanvas_set_image_alpha(this.__wbg_ptr, image_id, alpha);
        return ret !== 0;
    }
    /**
     * 把裁剪补丁写进当前选择并记一条撤销记录。
     *
     * 对象里出现的 `x` / `y` / `width` / `height` 才会修改。空选、锁定或没有变化时
     * 返回 false，也不入栈。不改世界变换。
     * @param {any} patch
     * @returns {boolean}
     */
    set_image_crop(patch) {
        const ret = wasm.infinitecanvas_set_image_crop(this.__wbg_ptr, patch);
        return ret !== 0;
    }
    /**
     * 显示或隐藏标尺和手动参考线。
     *
     * 隐藏后参考线仍留在场景 JSON 里，但不再命中、不再吸附。
     * 正在拖的参考线会取消，场景回到按下前的参考线。
     *
     * # 参数
     * - `visible`: `true` 显示
     * @param {boolean} visible
     */
    set_rulers_visible(visible) {
        wasm.infinitecanvas_set_rulers_visible(this.__wbg_ptr, visible);
    }
    /**
     * 把补丁写进当前选择并记一条撤销记录。
     *
     * 对象里出现的字段才会修改。`fill` / `stroke` 传 `null` 或 `""` 表示清掉。
     * `strokeWidth`、`opacity`、`fontSize`、`cornerRadius`、`sides`、`innerRadius` 是数字。
     * `cornerRadius` 也可以写成 `corner_radius`。`sides` 也可以写成 `points`，并钳到 3–64。
     * `innerRadius` 也可以写成 `inner_radius`，只作用于星形，并钳到 0.05–0.95。
     * `textColor` 是十六进制颜色。`textAlign` 为 `left` / `center` / `right`，也可写成 `text_align` / `align`。
     * `fontWeight` 为 `regular` / `bold`（或 `400` / `700`），也可写成 `font_weight`。对齐与字重只写到文本上。圆角只写到矩形上。
     * `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowOpacity` 是数字，也可以写成 `shadow_offset_x` 等蛇形名字。
     * `shadowColor` 只改 RGB，浓度用 `shadowOpacity`。形状、文本和图片都接受投影。
     * `blurRadius` 也可以写成 `blur_radius`。形状、文本、图片和画板都接受图层模糊。负数收成 0。
     * `strokeDash` 是数字数组（空数组改回实线）。`strokeCap` / `strokeJoin` / `strokeAlign` / `blendMode` 是小写名字。
     * `fillKind` 是 `solid` / `linear` / `radial`。`fillStops` 是 `{offset, color}` 数组。
     * `fillAngle`、`fillRadius` 是数字。`fillCenter` 是 `{x, y}` 或 `[x, y]`。
     * 也接受 `fill_kind`、`fill_stops`、`fill_angle`、`fill_center`、`fill_radius`、`blend_mode`。
     * `blendMode` 也可以写成 `blend_mode`。空选、锁定或没有变化时返回 false，不入栈。
     * 渐变只改形状，描边保持纯色。线型和线帽线结只改形状，文本描边保持原来的光栅描边。
     * 没有任何变化时返回 false，也不入栈。
     * @param {any} patch
     * @returns {boolean}
     */
    set_selection_style(patch) {
        const ret = wasm.infinitecanvas_set_selection_style(this.__wbg_ptr, patch);
        return ret !== 0;
    }
    /**
     * 把变换补丁写进当前选择并记一条撤销记录。
     *
     * 对象里出现的 `x` / `y` / `width`（或 `w`）/ `height`（或 `h`）/ `rotation`
     * （或 `rotationDeg` / `rotation_deg`）才会修改。宽高必须为正有限数。
     * 直线忽略宽高。多选时每个根写入同一绝对值。
     * 空选、锁定、非法输入或没有变化时返回 false，不入栈。
     *
     * # 示例（JavaScript）
     * ```js
     * canvas.set_selection_transform({ x: 100, rotation: 15 });
     * ```
     * @param {any} patch
     * @returns {boolean}
     */
    set_selection_transform(patch) {
        const ret = wasm.infinitecanvas_set_selection_transform(this.__wbg_ptr, patch);
        return ret !== 0;
    }
    /**
     * 开启或关闭拖拽吸附。
     *
     * 关闭后元素跟着指针走，不再产生对齐参考线。
     *
     * # 参数
     * - `enabled`: `true` 开启吸附
     * @param {boolean} enabled
     */
    set_snap_enabled(enabled) {
        wasm.infinitecanvas_set_snap_enabled(this.__wbg_ptr, enabled);
    }
    /**
     * 设置吸附阈值（屏幕像素）。
     *
     * 非有限值会被忽略。有效值限制在 0 到 64 之间。
     * 世界距离会除以当前缩放，因此任意缩放下抓取范围保持同样的屏幕像素。
     *
     * # 参数
     * - `threshold_px`: 屏幕像素阈值
     * @param {number} threshold_px
     */
    set_snap_threshold(threshold_px) {
        wasm.infinitecanvas_set_snap_threshold(this.__wbg_ptr, threshold_px);
    }
    /**
     * 编辑时隐藏 GPU 文本，避免和 DOM 输入框叠两层字。
     * @param {bigint} id
     * @param {boolean} editing
     * @returns {boolean}
     */
    set_text_editing(id, editing) {
        const ret = wasm.infinitecanvas_set_text_editing(this.__wbg_ptr, id, editing);
        return ret !== 0;
    }
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
     * @param {string} tool_name
     * @returns {boolean}
     */
    set_tool(tool_name) {
        const ptr0 = passStringToWasm0(tool_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_set_tool(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
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
     * @param {number} zoom
     */
    set_zoom(zoom) {
        wasm.infinitecanvas_set_zoom(this.__wbg_ptr, zoom);
    }
    /**
     * 取走最近一次吸管点击的结果。没有新点击时返回 null。
     *
     * - `hit`：点到了可见叶子
     * - `applied`：颜色已经写进当前选择，并占一条撤销记录
     *
     * 点在空白处时工具保持吸管。点到元素后，工具回到进入吸管之前的那个。
     * @returns {any}
     */
    take_eyedropper_notice() {
        const ret = wasm.infinitecanvas_take_eyedropper_notice(this.__wbg_ptr);
        return ret;
    }
    /**
     * 取走 T 工具留下的编辑请求。没有请求时返回 `null`。
     * @returns {any}
     */
    take_text_edit_request() {
        const ret = wasm.infinitecanvas_take_text_edit_request(this.__wbg_ptr);
        return ret;
    }
    /**
     * 取出当前缩放档下还没光栅化的文本。
     *
     * 每个元素在同一内容版本和 LOD 上只返回一次。前端用 Canvas2D 画完后调用
     * [`Self::commit_text_raster`]。失败时调用 [`Self::abandon_text_raster`] 以便重试。
     *
     * # 返回
     * 对象数组：`id`, `revision`, `lod`, `rasterScale`, `content`, `fontSizePx`,
     * `color`, `align`, `fontWeight`, `fontFamily`, `paddingWorld`, `textureId`
     * @returns {Array<any>}
     */
    take_text_raster_jobs() {
        const ret = wasm.infinitecanvas_take_text_raster_jobs(this.__wbg_ptr);
        return ret;
    }
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
     * @returns {boolean}
     */
    toggle_selection_locked() {
        const ret = wasm.infinitecanvas_toggle_selection_locked(this.__wbg_ptr);
        return ret !== 0;
    }
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
     * @returns {boolean}
     */
    toggle_selection_visible() {
        const ret = wasm.infinitecanvas_toggle_selection_visible(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 平移一个元素。目标是组时，子孙的世界坐标一起移动。
     * @param {bigint} id
     * @param {number} dx
     * @param {number} dy
     * @returns {boolean}
     */
    translate_element(id, dx, dy) {
        const ret = wasm.infinitecanvas_translate_element(this.__wbg_ptr, id, dx, dy);
        return ret !== 0;
    }
    /**
     * 撤销上一个操作
     *
     * # 返回
     * 是否成功撤销
     * @returns {boolean}
     */
    undo() {
        const ret = wasm.infinitecanvas_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * 解散组。子元素留在场景里。
     * @param {bigint} group_id
     * @returns {boolean}
     */
    ungroup(group_id) {
        const ret = wasm.infinitecanvas_ungroup(this.__wbg_ptr, group_id);
        return ret !== 0;
    }
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
     * @param {string} texture_id
     * @param {number} width
     * @param {number} height
     * @param {Uint8Array} data
     */
    upload_texture(texture_id, width, height, data) {
        const ptr0 = passStringToWasm0(texture_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.infinitecanvas_upload_texture(this.__wbg_ptr, ptr0, len0, width, height, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * 缩放视口
     *
     * # 参数
     * - `anchor_x, anchor_y`: 缩放中心点（屏幕坐标）
     * - `delta_zoom`: 缩放倍数（例如 1.1 放大 10%）
     * @param {number} anchor_x
     * @param {number} anchor_y
     * @param {number} delta_zoom
     */
    zoom_at(anchor_x, anchor_y, delta_zoom) {
        wasm.infinitecanvas_zoom_at(this.__wbg_ptr, anchor_x, anchor_y, delta_zoom);
    }
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
     * @param {number} factor
     */
    zoom_by(factor) {
        wasm.infinitecanvas_zoom_by(this.__wbg_ptr, factor);
    }
    /**
     * 滚轮和键盘共用的一步缩放倍数。
     *
     * 大于 1 表示放大。Demo 滚轮把指针交给 [`Self::zoom_at`]，
     * 键盘把返回值交给 [`Self::zoom_by`]。
     * @returns {number}
     */
    zoom_step() {
        const ret = wasm.infinitecanvas_zoom_step(this.__wbg_ptr);
        return ret;
    }
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
     * @returns {boolean}
     */
    zoom_to_fit() {
        const ret = wasm.infinitecanvas_zoom_to_fit(this.__wbg_ptr);
        return ret !== 0;
    }
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
     * @returns {boolean}
     */
    zoom_to_selection() {
        const ret = wasm.infinitecanvas_zoom_to_selection(this.__wbg_ptr);
        return ret !== 0;
    }
}
if (Symbol.dispose) InfiniteCanvas.prototype[Symbol.dispose] = InfiniteCanvas.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_BigInt_55476612a84195bb: function() { return handleError(function (arg0) {
            const ret = BigInt(arg0);
            return ret;
        }, arguments); },
        __wbg_Window_e0df001eddf1d3fa: function(arg0) {
            const ret = arg0.Window;
            return ret;
        },
        __wbg_WorkerGlobalScope_d731e9136c6c49a0: function(arg0) {
            const ret = arg0.WorkerGlobalScope;
            return ret;
        },
        __wbg___wbindgen_boolean_get_5b446f51afd21013: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_4687d8d8c2017d52: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_1f9d30630b8b1d3d: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_e343b7d08827ba72: function(arg0) {
            const ret = arg0 === null;
            return ret;
        },
        __wbg___wbindgen_is_object_3c45d4f2dde4e749: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_undefined_8865fb403f8fe9d8: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_number_get_2e0e7dee9f701a71: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_0380ccaa2f57f0d9: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_41e9ee4f547fc59a: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_dcc1a90847f04c41: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_activeTexture_11079673eac5c921: function(arg0, arg1) {
            arg0.activeTexture(arg1 >>> 0);
        },
        __wbg_activeTexture_2fad46c01cafe6b1: function(arg0, arg1) {
            arg0.activeTexture(arg1 >>> 0);
        },
        __wbg_attachShader_8920a5beea54aba7: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_attachShader_9567ae57af6746d5: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_beginQuery_eaee271e7ccd67d1: function(arg0, arg1, arg2) {
            arg0.beginQuery(arg1 >>> 0, arg2);
        },
        __wbg_beginRenderPass_373f34636d157c43: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.beginRenderPass(arg1);
            return ret;
        }, arguments); },
        __wbg_bindAttribLocation_6a6f66cbe96ef24e: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.bindAttribLocation(arg1, arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        },
        __wbg_bindAttribLocation_ade0ee986a734cdf: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.bindAttribLocation(arg1, arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        },
        __wbg_bindBufferRange_a5aef3642afdabc8: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.bindBufferRange(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5);
        },
        __wbg_bindBuffer_0324f62b876c4403: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindBuffer_2195e89b585ce0c3: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindFramebuffer_22e4b8e6e6d8d0e8: function(arg0, arg1, arg2) {
            arg0.bindFramebuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindFramebuffer_378eb18ef219d37a: function(arg0, arg1, arg2) {
            arg0.bindFramebuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindRenderbuffer_db090531fb28de34: function(arg0, arg1, arg2) {
            arg0.bindRenderbuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindRenderbuffer_e0c5aeef245a913a: function(arg0, arg1, arg2) {
            arg0.bindRenderbuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindSampler_9b9743706aee71d1: function(arg0, arg1, arg2) {
            arg0.bindSampler(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_3cf008f256f3ed99: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_613a47b8917a0ccd: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_bindVertexArrayOES_3141815cb8234a6e: function(arg0, arg1) {
            arg0.bindVertexArrayOES(arg1);
        },
        __wbg_bindVertexArray_f58af0a346185ec5: function(arg0, arg1) {
            arg0.bindVertexArray(arg1);
        },
        __wbg_blendColor_68fc6d6454f2128d: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendColor(arg1, arg2, arg3, arg4);
        },
        __wbg_blendColor_a6256f41d1925899: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendColor(arg1, arg2, arg3, arg4);
        },
        __wbg_blendEquationSeparate_0a17fa4ea863d445: function(arg0, arg1, arg2) {
            arg0.blendEquationSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blendEquationSeparate_fa2d4e5dde5ad8b3: function(arg0, arg1, arg2) {
            arg0.blendEquationSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blendEquation_5fb8aee6a55cf900: function(arg0, arg1) {
            arg0.blendEquation(arg1 >>> 0);
        },
        __wbg_blendEquation_d21914e937b14896: function(arg0, arg1) {
            arg0.blendEquation(arg1 >>> 0);
        },
        __wbg_blendFuncSeparate_28abe082febb1a26: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_blendFuncSeparate_2af9d47f7efc0452: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_blendFunc_491ee1105079ce76: function(arg0, arg1, arg2) {
            arg0.blendFunc(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blendFunc_589e4252bb396484: function(arg0, arg1, arg2) {
            arg0.blendFunc(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blitFramebuffer_7b39bfec6cfece56: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.blitFramebuffer(arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0);
        },
        __wbg_bufferData_50f5eb06469330e4: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_53e0f7a06b58fe81: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_5fa44d94fe7a3878: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_bad7ce562b6a9eac: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferSubData_22c4c420e0f67cda: function(arg0, arg1, arg2, arg3) {
            arg0.bufferSubData(arg1 >>> 0, arg2, arg3);
        },
        __wbg_bufferSubData_8609cd8a2c013181: function(arg0, arg1, arg2, arg3) {
            arg0.bufferSubData(arg1 >>> 0, arg2, arg3);
        },
        __wbg_buffer_56ec2905a66f58b9: function(arg0) {
            const ret = arg0.buffer;
            return ret;
        },
        __wbg_call_187d372bd5fdd4aa: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_clearBufferfv_05f7fc7252e623eb: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearBufferfv(arg1 >>> 0, arg2, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_clearBufferiv_e8e740accbd0f2df: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearBufferiv(arg1 >>> 0, arg2, getArrayI32FromWasm0(arg3, arg4));
        },
        __wbg_clearBufferuiv_e7f50f91f2195e38: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearBufferuiv(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4));
        },
        __wbg_clearDepth_4840a5497ae51286: function(arg0, arg1) {
            arg0.clearDepth(arg1);
        },
        __wbg_clearDepth_5a119c24c5d193bc: function(arg0, arg1) {
            arg0.clearDepth(arg1);
        },
        __wbg_clearStencil_1b8b8c727f598eec: function(arg0, arg1) {
            arg0.clearStencil(arg1);
        },
        __wbg_clearStencil_e4383e4a543e7997: function(arg0, arg1) {
            arg0.clearStencil(arg1);
        },
        __wbg_clear_175cf3cb6d98d72b: function(arg0, arg1) {
            arg0.clear(arg1 >>> 0);
        },
        __wbg_clear_cb96796cc568b7c8: function(arg0, arg1) {
            arg0.clear(arg1 >>> 0);
        },
        __wbg_clientWaitSync_73122aa040ce343d: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.clientWaitSync(arg1, arg2 >>> 0, arg3 >>> 0);
            return ret;
        },
        __wbg_colorMask_4d6afbbbc3022fbe: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.colorMask(arg1 !== 0, arg2 !== 0, arg3 !== 0, arg4 !== 0);
        },
        __wbg_colorMask_8c903995f6f873be: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.colorMask(arg1 !== 0, arg2 !== 0, arg3 !== 0, arg4 !== 0);
        },
        __wbg_compileShader_073a75409aaf96d2: function(arg0, arg1) {
            arg0.compileShader(arg1);
        },
        __wbg_compileShader_c2b53ff3a8851f60: function(arg0, arg1) {
            arg0.compileShader(arg1);
        },
        __wbg_compressedTexSubImage2D_4942a783e20221c2: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.compressedTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8);
        },
        __wbg_compressedTexSubImage2D_4c408678ff5f3056: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.compressedTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8, arg9);
        },
        __wbg_compressedTexSubImage2D_80961cdbbd2b3aea: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.compressedTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8);
        },
        __wbg_compressedTexSubImage3D_1e0f86237d82f8b9: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.compressedTexSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10);
        },
        __wbg_compressedTexSubImage3D_682e402a0ac201ba: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.compressedTexSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10, arg11);
        },
        __wbg_configure_b39d6ec9527208fd: function() { return handleError(function (arg0, arg1) {
            arg0.configure(arg1);
        }, arguments); },
        __wbg_copyBufferSubData_e82219c4834bd9ae: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.copyBufferSubData(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5);
        },
        __wbg_copyTexSubImage2D_455fba12cf4a776c: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.copyTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8);
        },
        __wbg_copyTexSubImage2D_fe71e36efa520f37: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.copyTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8);
        },
        __wbg_copyTexSubImage3D_717cd6c97934f2e6: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.copyTexSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9);
        },
        __wbg_createBindGroupLayout_f5bb5a31b2ac11bf: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBindGroupLayout(arg1);
            return ret;
        }, arguments); },
        __wbg_createBindGroup_2290306cfa413c74: function(arg0, arg1) {
            const ret = arg0.createBindGroup(arg1);
            return ret;
        },
        __wbg_createBuffer_d1dd23bf8704564c: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createBuffer_e2b25dd1471f92f7: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBuffer(arg1);
            return ret;
        }, arguments); },
        __wbg_createBuffer_ffe613371d42cfd3: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createCommandEncoder_80578730e7314357: function(arg0, arg1) {
            const ret = arg0.createCommandEncoder(arg1);
            return ret;
        },
        __wbg_createFramebuffer_2c6b5b0f49c88e5d: function(arg0) {
            const ret = arg0.createFramebuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createFramebuffer_d0969af4e9886297: function(arg0) {
            const ret = arg0.createFramebuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createPipelineLayout_0ef251301bed0c34: function(arg0, arg1) {
            const ret = arg0.createPipelineLayout(arg1);
            return ret;
        },
        __wbg_createProgram_cbf448e6f18a9402: function(arg0) {
            const ret = arg0.createProgram();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createProgram_e0c81b0d2b8d0fd0: function(arg0) {
            const ret = arg0.createProgram();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createQuery_ec241083d0b36197: function(arg0) {
            const ret = arg0.createQuery();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createRenderPipeline_f9f8aa23f50f8a9c: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createRenderPipeline(arg1);
            return ret;
        }, arguments); },
        __wbg_createRenderbuffer_2b17e9fe2e6fb4be: function(arg0) {
            const ret = arg0.createRenderbuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createRenderbuffer_c864095d6ebd60d0: function(arg0) {
            const ret = arg0.createRenderbuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createSampler_1b330d9da0c6523f: function(arg0) {
            const ret = arg0.createSampler();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createSampler_27c37a8245da51a4: function(arg0, arg1) {
            const ret = arg0.createSampler(arg1);
            return ret;
        },
        __wbg_createShaderModule_eb21a131dfb0d4dc: function(arg0, arg1) {
            const ret = arg0.createShaderModule(arg1);
            return ret;
        },
        __wbg_createShader_6c1a2295587878a5: function(arg0, arg1) {
            const ret = arg0.createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createShader_7c9d99a21ecb08c0: function(arg0, arg1) {
            const ret = arg0.createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createTexture_01e8e591b64237de: function(arg0) {
            const ret = arg0.createTexture();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createTexture_284160f981e0075f: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createTexture(arg1);
            return ret;
        }, arguments); },
        __wbg_createTexture_5f2583508ebe379b: function(arg0) {
            const ret = arg0.createTexture();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createVertexArrayOES_a8dc8498b56df937: function(arg0) {
            const ret = arg0.createVertexArrayOES();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createVertexArray_b223ea69dd08fcab: function(arg0) {
            const ret = arg0.createVertexArray();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createView_b09749798973b0f5: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createView(arg1);
            return ret;
        }, arguments); },
        __wbg_cullFace_12123c1c97f1ba2c: function(arg0, arg1) {
            arg0.cullFace(arg1 >>> 0);
        },
        __wbg_cullFace_1a01f0b6d5aeb687: function(arg0, arg1) {
            arg0.cullFace(arg1 >>> 0);
        },
        __wbg_debug_52bda05ddf50b736: function(arg0) {
            console.debug(arg0);
        },
        __wbg_deleteBuffer_070e58aaaf2a7a8b: function(arg0, arg1) {
            arg0.deleteBuffer(arg1);
        },
        __wbg_deleteBuffer_7775ee9e248c8468: function(arg0, arg1) {
            arg0.deleteBuffer(arg1);
        },
        __wbg_deleteFramebuffer_3f8ab330098bf8e6: function(arg0, arg1) {
            arg0.deleteFramebuffer(arg1);
        },
        __wbg_deleteFramebuffer_570d3b214a509fa2: function(arg0, arg1) {
            arg0.deleteFramebuffer(arg1);
        },
        __wbg_deleteProgram_822906f0368175d0: function(arg0, arg1) {
            arg0.deleteProgram(arg1);
        },
        __wbg_deleteProgram_b524b30ad176cdb4: function(arg0, arg1) {
            arg0.deleteProgram(arg1);
        },
        __wbg_deleteQuery_266895df9763e0b3: function(arg0, arg1) {
            arg0.deleteQuery(arg1);
        },
        __wbg_deleteRenderbuffer_826c9b43b7ea68f5: function(arg0, arg1) {
            arg0.deleteRenderbuffer(arg1);
        },
        __wbg_deleteRenderbuffer_f4e121a3ab21c605: function(arg0, arg1) {
            arg0.deleteRenderbuffer(arg1);
        },
        __wbg_deleteSampler_f8afded8bf215d53: function(arg0, arg1) {
            arg0.deleteSampler(arg1);
        },
        __wbg_deleteShader_a70f6cc1918d67eb: function(arg0, arg1) {
            arg0.deleteShader(arg1);
        },
        __wbg_deleteShader_da18a294158c2d15: function(arg0, arg1) {
            arg0.deleteShader(arg1);
        },
        __wbg_deleteSync_4f0be28b03f50495: function(arg0, arg1) {
            arg0.deleteSync(arg1);
        },
        __wbg_deleteTexture_78c170241204a466: function(arg0, arg1) {
            arg0.deleteTexture(arg1);
        },
        __wbg_deleteTexture_9f42750adb092e4f: function(arg0, arg1) {
            arg0.deleteTexture(arg1);
        },
        __wbg_deleteVertexArrayOES_e01f31634d466176: function(arg0, arg1) {
            arg0.deleteVertexArrayOES(arg1);
        },
        __wbg_deleteVertexArray_ca9768805d051d49: function(arg0, arg1) {
            arg0.deleteVertexArray(arg1);
        },
        __wbg_depthFunc_92e96d8f46a23300: function(arg0, arg1) {
            arg0.depthFunc(arg1 >>> 0);
        },
        __wbg_depthFunc_a41fb5adacb22e09: function(arg0, arg1) {
            arg0.depthFunc(arg1 >>> 0);
        },
        __wbg_depthMask_06ecff4d4514c38f: function(arg0, arg1) {
            arg0.depthMask(arg1 !== 0);
        },
        __wbg_depthMask_0d7c3dec3b8405ee: function(arg0, arg1) {
            arg0.depthMask(arg1 !== 0);
        },
        __wbg_depthRange_24c7969249461e8e: function(arg0, arg1, arg2) {
            arg0.depthRange(arg1, arg2);
        },
        __wbg_depthRange_6a4e169ccf314148: function(arg0, arg1, arg2) {
            arg0.depthRange(arg1, arg2);
        },
        __wbg_destroy_ebf527bbd86ae58b: function(arg0) {
            arg0.destroy();
        },
        __wbg_disableVertexAttribArray_11c9f11bee30c9d8: function(arg0, arg1) {
            arg0.disableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_disableVertexAttribArray_c4c8af1f9e4120d4: function(arg0, arg1) {
            arg0.disableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_disable_19fe787eccdf0272: function(arg0, arg1) {
            arg0.disable(arg1 >>> 0);
        },
        __wbg_disable_81b2cb5f8cea15ae: function(arg0, arg1) {
            arg0.disable(arg1 >>> 0);
        },
        __wbg_document_9854e03c05fc8834: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_drawArraysInstancedANGLE_b76512b5abba6668: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.drawArraysInstancedANGLE(arg1 >>> 0, arg2, arg3, arg4);
        },
        __wbg_drawArraysInstanced_9b44aa78a087154d: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.drawArraysInstanced(arg1 >>> 0, arg2, arg3, arg4);
        },
        __wbg_drawArrays_08872e02fb57b847: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_drawArrays_741af5c3f8db636f: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_drawBuffersWEBGL_c1bc0bbfd584c9b2: function(arg0, arg1) {
            arg0.drawBuffersWEBGL(arg1);
        },
        __wbg_drawBuffers_7a58bd4729f7a1fc: function(arg0, arg1) {
            arg0.drawBuffers(arg1);
        },
        __wbg_drawElementsInstancedANGLE_c3fc0cfa9e29a1bc: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawElementsInstancedANGLE(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_drawElementsInstanced_d7c396aa034159b6: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawElementsInstanced(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_draw_bcc050d6677121b5: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.draw(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_enableVertexAttribArray_1496b78a8dbab9fd: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_enableVertexAttribArray_d52693866bc8b8dd: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_enable_6eeb9c0e9b483cee: function(arg0, arg1) {
            arg0.enable(arg1 >>> 0);
        },
        __wbg_enable_cb4fc19b51148c37: function(arg0, arg1) {
            arg0.enable(arg1 >>> 0);
        },
        __wbg_endQuery_73863d04a795d520: function(arg0, arg1) {
            arg0.endQuery(arg1 >>> 0);
        },
        __wbg_end_c269ebd826210ed1: function(arg0) {
            arg0.end();
        },
        __wbg_error_757e9472f8410341: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_error_c9cf3fc2064683a9: function(arg0) {
            console.error(arg0);
        },
        __wbg_fenceSync_330293024adb66bc: function(arg0, arg1, arg2) {
            const ret = arg0.fenceSync(arg1 >>> 0, arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_finish_073e2bc456a4b625: function(arg0) {
            const ret = arg0.finish();
            return ret;
        },
        __wbg_finish_e43b1b48427f2db0: function(arg0, arg1) {
            const ret = arg0.finish(arg1);
            return ret;
        },
        __wbg_flush_13b162660ad850d2: function(arg0) {
            arg0.flush();
        },
        __wbg_flush_a5ad38200881ac35: function(arg0) {
            arg0.flush();
        },
        __wbg_framebufferRenderbuffer_1c6337e0030411f3: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.framebufferRenderbuffer(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4);
        },
        __wbg_framebufferRenderbuffer_f019e9231b142c0d: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.framebufferRenderbuffer(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4);
        },
        __wbg_framebufferTexture2D_73c52f23ec33aa22: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTexture2D(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5);
        },
        __wbg_framebufferTexture2D_caddefa1f767646e: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTexture2D(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5);
        },
        __wbg_framebufferTextureLayer_1d5c0f0e3234e521: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTextureLayer(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5);
        },
        __wbg_framebufferTextureMultiviewOVR_77f19224ff7be6ef: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.framebufferTextureMultiviewOVR(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5, arg6);
        },
        __wbg_from_296ca31f8d0f1c52: function(arg0) {
            const ret = Array.from(arg0);
            return ret;
        },
        __wbg_frontFace_6751b4b97f997aeb: function(arg0, arg1) {
            arg0.frontFace(arg1 >>> 0);
        },
        __wbg_frontFace_b064adb15b6f3772: function(arg0, arg1) {
            arg0.frontFace(arg1 >>> 0);
        },
        __wbg_getBindGroupLayout_5e833af835634af6: function(arg0, arg1) {
            const ret = arg0.getBindGroupLayout(arg1 >>> 0);
            return ret;
        },
        __wbg_getBufferSubData_dd23246099b166d1: function(arg0, arg1, arg2, arg3) {
            arg0.getBufferSubData(arg1 >>> 0, arg2, arg3);
        },
        __wbg_getContext_051324720aa5b7e8: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2), arg3);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_189ce278aa985704: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2), arg3);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_635e36719cad2623: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_e567868594d2e0c6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getCurrentTexture_7edbea16b438c9fc: function() { return handleError(function (arg0) {
            const ret = arg0.getCurrentTexture();
            return ret;
        }, arguments); },
        __wbg_getExtension_abfda2c5db286b07: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getExtension(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getIndexedParameter_0f49864473e337fa: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getIndexedParameter(arg1 >>> 0, arg2 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getMappedRange_191c0084744858f0: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getMappedRange(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_getParameter_974595a6624c6c10: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.getParameter(arg1 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getParameter_a97f491753b05668: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.getParameter(arg1 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getPreferredCanvasFormat_56e30944cc798353: function(arg0) {
            const ret = arg0.getPreferredCanvasFormat();
            return (__wbindgen_enum_GpuTextureFormat.indexOf(ret) + 1 || 96) - 1;
        },
        __wbg_getProgramInfoLog_208fb1ff31006495: function(arg0, arg1, arg2) {
            const ret = arg1.getProgramInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_getProgramInfoLog_3276b294d6d42fe2: function(arg0, arg1, arg2) {
            const ret = arg1.getProgramInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_getProgramParameter_50bafab01cf88652: function(arg0, arg1, arg2) {
            const ret = arg0.getProgramParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getProgramParameter_fc7124bb9fb7c675: function(arg0, arg1, arg2) {
            const ret = arg0.getProgramParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getQueryParameter_937ba13f77db778f: function(arg0, arg1, arg2) {
            const ret = arg0.getQueryParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getShaderInfoLog_7e0cbba353ca95da: function(arg0, arg1, arg2) {
            const ret = arg1.getShaderInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_getShaderInfoLog_bb64c5a52df0b7a3: function(arg0, arg1, arg2) {
            const ret = arg1.getShaderInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_getShaderParameter_745acf10b8e45e31: function(arg0, arg1, arg2) {
            const ret = arg0.getShaderParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getShaderParameter_e55ea9c456df5484: function(arg0, arg1, arg2) {
            const ret = arg0.getShaderParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getSupportedExtensions_1459bad37860c0c3: function(arg0) {
            const ret = arg0.getSupportedExtensions();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getSupportedProfiles_7b2acfa66b565d24: function(arg0) {
            const ret = arg0.getSupportedProfiles();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getSyncParameter_bb4be47b67bfa2f8: function(arg0, arg1, arg2) {
            const ret = arg0.getSyncParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getUniformBlockIndex_5e2252ceb34531a5: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformBlockIndex(arg1, getStringFromWasm0(arg2, arg3));
            return ret;
        },
        __wbg_getUniformLocation_50ba45dade572703: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getUniformLocation_cb4e3e121235f91a: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_31af05bd4842a84f: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_6c896e0571ddae51: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_get_fd12a9100976c296: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_unchecked_288889d017702237: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_gpu_7c0927abcc96dd45: function(arg0) {
            const ret = arg0.gpu;
            return ret;
        },
        __wbg_has_5d6706e5209576c1: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.has(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_height_fc97e1a0c2e7331f: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_includes_9b0dfa946c313621: function(arg0, arg1, arg2) {
            const ret = arg0.includes(arg1, arg2);
            return ret;
        },
        __wbg_info_26925f73a2eee895: function(arg0) {
            console.info(arg0);
        },
        __wbg_instanceof_GpuAdapter_5e451ad6596e2784: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUAdapter;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuCanvasContext_f70ee27f49f4f884: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUCanvasContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_0a30c11fbbf41841: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WebGl2RenderingContext_64c994c14c0790fb: function(arg0) {
            let result;
            try {
                result = arg0 instanceof WebGL2RenderingContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_82d71df4eddf88bc: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_invalidateFramebuffer_4f576db65bb37d6a: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.invalidateFramebuffer(arg1 >>> 0, arg2);
        }, arguments); },
        __wbg_isArray_e15a2ff68ffdbef2: function(arg0) {
            const ret = Array.isArray(arg0);
            return ret;
        },
        __wbg_is_4b278c0bd3caba97: function(arg0, arg1) {
            const ret = Object.is(arg0, arg1);
            return ret;
        },
        __wbg_label_0abc44bf8d3a3e99: function(arg0, arg1) {
            const ret = arg1.label;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_length_58572db4c38f3c3e: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_d4bdea10311bd9cf: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_linkProgram_571a9ea00d38ff83: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_linkProgram_acdc299c43d734a0: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_log_17c30ef363c61cf4: function(arg0) {
            console.log(arg0);
        },
        __wbg_mapAsync_1be2f9e8f464f69e: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.mapAsync(arg1 >>> 0, arg2, arg3);
            return ret;
        },
        __wbg_navigator_2156486643462a87: function(arg0) {
            const ret = arg0.navigator;
            return ret;
        },
        __wbg_navigator_c9bce48d4b578c9c: function(arg0) {
            const ret = arg0.navigator;
            return ret;
        },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_new_617a8cdb8bb1130e: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_ee2291f50781bf1d: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_from_slice_9a868026ffa4208a: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_typed_b01cb72a8af741a3: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___js_sys_4cd4ef52bf73eaa4___Function_fn_wasm_bindgen_fe1d14e6b7d09dd3___JsValue_____wasm_bindgen_fe1d14e6b7d09dd3___sys__Undefined___js_sys_4cd4ef52bf73eaa4___Function_fn_wasm_bindgen_fe1d14e6b7d09dd3___JsValue_____wasm_bindgen_fe1d14e6b7d09dd3___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_new_with_byte_offset_and_length_2f5d7fc2a828b74d: function(arg0, arg1, arg2) {
            const ret = new Uint8Array(arg0, arg1 >>> 0, arg2 >>> 0);
            return ret;
        },
        __wbg_new_with_length_cc0362bfe8499e5a: function(arg0) {
            const ret = new Float32Array(arg0 >>> 0);
            return ret;
        },
        __wbg_now_aa4ccb83129e9e55: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_now_e7c6795a7f81e10f: function(arg0) {
            const ret = arg0.now();
            return ret;
        },
        __wbg_of_d5dbecdc59e02e34: function(arg0) {
            const ret = Array.of(arg0);
            return ret;
        },
        __wbg_onSubmittedWorkDone_7d532ba1f20a64b3: function(arg0) {
            const ret = arg0.onSubmittedWorkDone();
            return ret;
        },
        __wbg_performance_3fcf6e32a7e1ed0a: function(arg0) {
            const ret = arg0.performance;
            return ret;
        },
        __wbg_pixelStorei_af18e636b3e7b725: function(arg0, arg1, arg2) {
            arg0.pixelStorei(arg1 >>> 0, arg2);
        },
        __wbg_pixelStorei_cb4e43466f5b8703: function(arg0, arg1, arg2) {
            arg0.pixelStorei(arg1 >>> 0, arg2);
        },
        __wbg_polygonOffset_81dc8ca84e29572b: function(arg0, arg1, arg2) {
            arg0.polygonOffset(arg1, arg2);
        },
        __wbg_polygonOffset_8f922d9173d8849d: function(arg0, arg1, arg2) {
            arg0.polygonOffset(arg1, arg2);
        },
        __wbg_push_2baf45db356cf468: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_queryCounterEXT_9016a6ffa350b4b2: function(arg0, arg1, arg2) {
            arg0.queryCounterEXT(arg1, arg2 >>> 0);
        },
        __wbg_querySelectorAll_6aebfe3df1fb2014: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelectorAll(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_querySelector_ecbc2550705b235f: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelector(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_queueMicrotask_9833f9a49df95a49: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_a72f977e97f23c5f: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_queue_5eda23116e5d3adb: function(arg0) {
            const ret = arg0.queue;
            return ret;
        },
        __wbg_readBuffer_b5d9a34a2f3d20d5: function(arg0, arg1) {
            arg0.readBuffer(arg1 >>> 0);
        },
        __wbg_readPixels_31e44e70558ac590: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7);
        }, arguments); },
        __wbg_readPixels_3321086a7818a739: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7);
        }, arguments); },
        __wbg_readPixels_eb030cd9976eeca5: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7);
        }, arguments); },
        __wbg_renderbufferStorageMultisample_ed16f3ef446e3bc7: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.renderbufferStorageMultisample(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_renderbufferStorage_52b131bbf3112b3a: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.renderbufferStorage(arg1 >>> 0, arg2 >>> 0, arg3, arg4);
        },
        __wbg_renderbufferStorage_bca487b0dea425b8: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.renderbufferStorage(arg1 >>> 0, arg2 >>> 0, arg3, arg4);
        },
        __wbg_requestAdapter_8efca1b953fd13aa: function(arg0, arg1) {
            const ret = arg0.requestAdapter(arg1);
            return ret;
        },
        __wbg_requestDevice_290c73161fe959d5: function(arg0, arg1) {
            const ret = arg0.requestDevice(arg1);
            return ret;
        },
        __wbg_resolve_0076e10020304ede: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_samplerParameterf_2d62cb907392e2d6: function(arg0, arg1, arg2, arg3) {
            arg0.samplerParameterf(arg1, arg2 >>> 0, arg3);
        },
        __wbg_samplerParameteri_a5615e67ed0b654b: function(arg0, arg1, arg2, arg3) {
            arg0.samplerParameteri(arg1, arg2 >>> 0, arg3);
        },
        __wbg_scissor_12ddc324094acd50: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.scissor(arg1, arg2, arg3, arg4);
        },
        __wbg_scissor_a9468482697c2bdb: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.scissor(arg1, arg2, arg3, arg4);
        },
        __wbg_setBindGroup_29f4a44dff76f1a4: function(arg0, arg1, arg2) {
            arg0.setBindGroup(arg1 >>> 0, arg2);
        },
        __wbg_setBindGroup_35a4830ac2c27742: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setBindGroup(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4), arg5, arg6 >>> 0);
        }, arguments); },
        __wbg_setPipeline_e6ea6756d71b19a7: function(arg0, arg1) {
            arg0.setPipeline(arg1);
        },
        __wbg_setVertexBuffer_58f30a4873b36907: function(arg0, arg1, arg2, arg3) {
            arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3);
        },
        __wbg_setVertexBuffer_7aa508f017477005: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3, arg4);
        },
        __wbg_set_070bd465f1c195a4: function(arg0, arg1, arg2) {
            arg0.set(getArrayF32FromWasm0(arg1, arg2));
        },
        __wbg_set_145a351398b48c65: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_a_6f1653ca7319cdcf: function(arg0, arg1) {
            arg0.a = arg1;
        },
        __wbg_set_access_cbee993a36feed10: function(arg0, arg1) {
            arg0.access = __wbindgen_enum_GpuStorageTextureAccess[arg1];
        },
        __wbg_set_address_mode_u_38e255cd89ce1977: function(arg0, arg1) {
            arg0.addressModeU = __wbindgen_enum_GpuAddressMode[arg1];
        },
        __wbg_set_address_mode_v_513f843d6e3c9dbd: function(arg0, arg1) {
            arg0.addressModeV = __wbindgen_enum_GpuAddressMode[arg1];
        },
        __wbg_set_address_mode_w_801f70901a90ed5a: function(arg0, arg1) {
            arg0.addressModeW = __wbindgen_enum_GpuAddressMode[arg1];
        },
        __wbg_set_alpha_0a28ffc800461787: function(arg0, arg1) {
            arg0.alpha = arg1;
        },
        __wbg_set_alpha_mode_55b4f33e93691fe8: function(arg0, arg1) {
            arg0.alphaMode = __wbindgen_enum_GpuCanvasAlphaMode[arg1];
        },
        __wbg_set_alpha_to_coverage_enabled_ec44695cc0d0e961: function(arg0, arg1) {
            arg0.alphaToCoverageEnabled = arg1 !== 0;
        },
        __wbg_set_array_layer_count_e774b6d4a5334e63: function(arg0, arg1) {
            arg0.arrayLayerCount = arg1 >>> 0;
        },
        __wbg_set_array_stride_11c840b41b728354: function(arg0, arg1) {
            arg0.arrayStride = arg1;
        },
        __wbg_set_aspect_2503cdfcdcc17373: function(arg0, arg1) {
            arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
        },
        __wbg_set_attributes_ac1030b589bf253a: function(arg0, arg1) {
            arg0.attributes = arg1;
        },
        __wbg_set_b_d5b23064b0492744: function(arg0, arg1) {
            arg0.b = arg1;
        },
        __wbg_set_base_array_layer_f64cdadf250d1a9b: function(arg0, arg1) {
            arg0.baseArrayLayer = arg1 >>> 0;
        },
        __wbg_set_base_mip_level_74fc97c2aaf8fc33: function(arg0, arg1) {
            arg0.baseMipLevel = arg1 >>> 0;
        },
        __wbg_set_beginning_of_pass_write_index_348e7f2f53a86db0: function(arg0, arg1) {
            arg0.beginningOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_bind_group_layouts_6f13eb021a550053: function(arg0, arg1) {
            arg0.bindGroupLayouts = arg1;
        },
        __wbg_set_binding_2240d98479c0c256: function(arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        },
        __wbg_set_binding_5296904f2a4c7e25: function(arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        },
        __wbg_set_blend_4aea897cd7d3c0f8: function(arg0, arg1) {
            arg0.blend = arg1;
        },
        __wbg_set_buffer_ba8ed06078d347f7: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_buffer_fc9285180932669f: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_buffers_72754529595d4bc0: function(arg0, arg1) {
            arg0.buffers = arg1;
        },
        __wbg_set_bytes_per_row_5fedf5a2d44b8482: function(arg0, arg1) {
            arg0.bytesPerRow = arg1 >>> 0;
        },
        __wbg_set_clear_value_1171de96edbc21fe: function(arg0, arg1) {
            arg0.clearValue = arg1;
        },
        __wbg_set_code_27a25a855d3fbc6d: function(arg0, arg1, arg2) {
            arg0.code = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_color_attachments_4516b6dfb4ad987b: function(arg0, arg1) {
            arg0.colorAttachments = arg1;
        },
        __wbg_set_color_f2ac28bdc576c010: function(arg0, arg1) {
            arg0.color = arg1;
        },
        __wbg_set_compare_2c8ee8ccaa2b6b5d: function(arg0, arg1) {
            arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_compare_cbf49b43d3211833: function(arg0, arg1) {
            arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_count_53854513da5c0e04: function(arg0, arg1) {
            arg0.count = arg1 >>> 0;
        },
        __wbg_set_cull_mode_3852dd4cff56dd90: function(arg0, arg1) {
            arg0.cullMode = __wbindgen_enum_GpuCullMode[arg1];
        },
        __wbg_set_depth_bias_c20861a58fc2b8d9: function(arg0, arg1) {
            arg0.depthBias = arg1;
        },
        __wbg_set_depth_bias_clamp_eecc04d702f9402e: function(arg0, arg1) {
            arg0.depthBiasClamp = arg1;
        },
        __wbg_set_depth_bias_slope_scale_b2a251d3d4c65018: function(arg0, arg1) {
            arg0.depthBiasSlopeScale = arg1;
        },
        __wbg_set_depth_clear_value_fca9e379a0cdff8f: function(arg0, arg1) {
            arg0.depthClearValue = arg1;
        },
        __wbg_set_depth_compare_7883e52aad39b925: function(arg0, arg1) {
            arg0.depthCompare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_depth_fail_op_1d11c8e03d061484: function(arg0, arg1) {
            arg0.depthFailOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_depth_load_op_7e95e67c69e09c5e: function(arg0, arg1) {
            arg0.depthLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_depth_or_array_layers_36ef1df107b6b651: function(arg0, arg1) {
            arg0.depthOrArrayLayers = arg1 >>> 0;
        },
        __wbg_set_depth_read_only_0c5e726b56520b08: function(arg0, arg1) {
            arg0.depthReadOnly = arg1 !== 0;
        },
        __wbg_set_depth_stencil_17e2d1710f4e07ae: function(arg0, arg1) {
            arg0.depthStencil = arg1;
        },
        __wbg_set_depth_stencil_attachment_a7b5eca74b7ddcfb: function(arg0, arg1) {
            arg0.depthStencilAttachment = arg1;
        },
        __wbg_set_depth_store_op_1b4cc257f121a4e7: function(arg0, arg1) {
            arg0.depthStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_depth_write_enabled_1551f99ae66d959e: function(arg0, arg1) {
            arg0.depthWriteEnabled = arg1 !== 0;
        },
        __wbg_set_device_846227515bb0301a: function(arg0, arg1) {
            arg0.device = arg1;
        },
        __wbg_set_dimension_7454baa9c745cf06: function(arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureDimension[arg1];
        },
        __wbg_set_dimension_9d314669636abc65: function(arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_dst_factor_8397030245674624: function(arg0, arg1) {
            arg0.dstFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        },
        __wbg_set_end_of_pass_write_index_4600a261d0317ecb: function(arg0, arg1) {
            arg0.endOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_entries_4d13c932343146c3: function(arg0, arg1) {
            arg0.entries = arg1;
        },
        __wbg_set_entries_7e6b569918b11bf4: function(arg0, arg1) {
            arg0.entries = arg1;
        },
        __wbg_set_entry_point_7248ed25fb9070c7: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_entry_point_b01eb3970a1dcb95: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_external_texture_cf6cf39036321145: function(arg0, arg1) {
            arg0.externalTexture = arg1;
        },
        __wbg_set_fail_op_ac8f2b4c077715b1: function(arg0, arg1) {
            arg0.failOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_format_12bcbdd3428cd4b5: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_1fc8a436841b29c8: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_2a42ed14de233ae5: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuVertexFormat[arg1];
        },
        __wbg_set_format_3759d043ddc658d4: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_b08e529cc1612d7b: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_e0cf5a237864edb6: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_ffa0a97f114a945a: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_fragment_703ddd6f5db6e4af: function(arg0, arg1) {
            arg0.fragment = arg1;
        },
        __wbg_set_front_face_17a3723085696d9a: function(arg0, arg1) {
            arg0.frontFace = __wbindgen_enum_GpuFrontFace[arg1];
        },
        __wbg_set_g_4cc3b3e3231ca6f8: function(arg0, arg1) {
            arg0.g = arg1;
        },
        __wbg_set_has_dynamic_offset_dc25aba64b9bd3ff: function(arg0, arg1) {
            arg0.hasDynamicOffset = arg1 !== 0;
        },
        __wbg_set_height_ac705ece3aa08c95: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_height_c9789c1c77eaedff: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_height_fde391767df1ce27: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_label_10bd19b972ff1ba6: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_16cff4ff3c381368: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_343ceab4761679d7: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_403725ced930414e: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_62b82f9361718fb9: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_7d448e8a777d0d37: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_900e563567315063: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_98bef61fcbcecdde: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_9d2ce197e447a967: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_b5d7ff5f8e4fbaac: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_ba288fbac1259847: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_e1bd2437f39d21f3: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_e4debe6dc9ea319b: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_layout_53be3643dc5dbbbe: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_layout_ca5f863d331bb6b4: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_load_op_91d2cbf2912c96fd: function(arg0, arg1) {
            arg0.loadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_lod_max_clamp_01800ff5df00cc8e: function(arg0, arg1) {
            arg0.lodMaxClamp = arg1;
        },
        __wbg_set_lod_min_clamp_fe71be084b04bd97: function(arg0, arg1) {
            arg0.lodMinClamp = arg1;
        },
        __wbg_set_mag_filter_a6df09d1943d5caa: function(arg0, arg1) {
            arg0.magFilter = __wbindgen_enum_GpuFilterMode[arg1];
        },
        __wbg_set_mapped_at_creation_eb954cf5fdb9bc25: function(arg0, arg1) {
            arg0.mappedAtCreation = arg1 !== 0;
        },
        __wbg_set_mask_47a41aae6631771f: function(arg0, arg1) {
            arg0.mask = arg1 >>> 0;
        },
        __wbg_set_max_anisotropy_418bd200a56097a0: function(arg0, arg1) {
            arg0.maxAnisotropy = arg1;
        },
        __wbg_set_min_binding_size_d0315b751370234c: function(arg0, arg1) {
            arg0.minBindingSize = arg1;
        },
        __wbg_set_min_filter_5b27a7eb3f5ea88a: function(arg0, arg1) {
            arg0.minFilter = __wbindgen_enum_GpuFilterMode[arg1];
        },
        __wbg_set_mip_level_b50dccbd04935c98: function(arg0, arg1) {
            arg0.mipLevel = arg1 >>> 0;
        },
        __wbg_set_mip_level_count_307eb64d9d29e3a6: function(arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        },
        __wbg_set_mip_level_count_fe7f73daa6021aaa: function(arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        },
        __wbg_set_mipmap_filter_e1543204e8199db0: function(arg0, arg1) {
            arg0.mipmapFilter = __wbindgen_enum_GpuMipmapFilterMode[arg1];
        },
        __wbg_set_module_9afd1b80ff72cee9: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_module_ffe8f8e909e9fdcf: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_multisample_957afdd96685c6f5: function(arg0, arg1) {
            arg0.multisample = arg1;
        },
        __wbg_set_multisampled_84e304d3a68838ea: function(arg0, arg1) {
            arg0.multisampled = arg1 !== 0;
        },
        __wbg_set_offset_3e78f3e530cf8049: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_offset_616ad7dfa51d50e0: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_offset_bea112c360dc7f2b: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_operation_6c5fd88df90bc7b2: function(arg0, arg1) {
            arg0.operation = __wbindgen_enum_GpuBlendOperation[arg1];
        },
        __wbg_set_origin_dec4f4c36f9f79f6: function(arg0, arg1) {
            arg0.origin = arg1;
        },
        __wbg_set_pass_op_461dabd5ee4ea1b7: function(arg0, arg1) {
            arg0.passOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_power_preference_a4ce891b22ea2b05: function(arg0, arg1) {
            arg0.powerPreference = __wbindgen_enum_GpuPowerPreference[arg1];
        },
        __wbg_set_primitive_eb8abbc5e7f278a4: function(arg0, arg1) {
            arg0.primitive = arg1;
        },
        __wbg_set_query_set_849fb32875f137d7: function(arg0, arg1) {
            arg0.querySet = arg1;
        },
        __wbg_set_r_5fa0f548248c394c: function(arg0, arg1) {
            arg0.r = arg1;
        },
        __wbg_set_required_features_98a83c7003fd73d5: function(arg0, arg1) {
            arg0.requiredFeatures = arg1;
        },
        __wbg_set_resolve_target_1ff405e060e2d32e: function(arg0, arg1) {
            arg0.resolveTarget = arg1;
        },
        __wbg_set_resource_1409c14d4d6b5a50: function(arg0, arg1) {
            arg0.resource = arg1;
        },
        __wbg_set_rows_per_image_8104dfe1b042a530: function(arg0, arg1) {
            arg0.rowsPerImage = arg1 >>> 0;
        },
        __wbg_set_sample_count_95a9892a60894677: function(arg0, arg1) {
            arg0.sampleCount = arg1 >>> 0;
        },
        __wbg_set_sample_type_f8f7b39d62e7b29c: function(arg0, arg1) {
            arg0.sampleType = __wbindgen_enum_GpuTextureSampleType[arg1];
        },
        __wbg_set_sampler_a2277e90dfe7395f: function(arg0, arg1) {
            arg0.sampler = arg1;
        },
        __wbg_set_shader_location_cdbcf5cf84a6cbcb: function(arg0, arg1) {
            arg0.shaderLocation = arg1 >>> 0;
        },
        __wbg_set_size_6f271c4c28c18e1b: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_size_7ec162511b3bad1f: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_size_ca765d983baccefd: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_src_factor_e96f05a25f8383ed: function(arg0, arg1) {
            arg0.srcFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        },
        __wbg_set_stencil_back_5c8971274cbcddcf: function(arg0, arg1) {
            arg0.stencilBack = arg1;
        },
        __wbg_set_stencil_clear_value_89ba97b367fa1385: function(arg0, arg1) {
            arg0.stencilClearValue = arg1 >>> 0;
        },
        __wbg_set_stencil_front_69f85bf4a6f02cb2: function(arg0, arg1) {
            arg0.stencilFront = arg1;
        },
        __wbg_set_stencil_load_op_a3e2c3a6f20d4da5: function(arg0, arg1) {
            arg0.stencilLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_stencil_read_mask_86a08afb2665c29b: function(arg0, arg1) {
            arg0.stencilReadMask = arg1 >>> 0;
        },
        __wbg_set_stencil_read_only_dd058fe8c6a1f6ae: function(arg0, arg1) {
            arg0.stencilReadOnly = arg1 !== 0;
        },
        __wbg_set_stencil_store_op_87c97415636844c9: function(arg0, arg1) {
            arg0.stencilStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_stencil_write_mask_7844d8a057a87a58: function(arg0, arg1) {
            arg0.stencilWriteMask = arg1 >>> 0;
        },
        __wbg_set_step_mode_285f2e428148f3b4: function(arg0, arg1) {
            arg0.stepMode = __wbindgen_enum_GpuVertexStepMode[arg1];
        },
        __wbg_set_storage_texture_373b9fc0e534dd33: function(arg0, arg1) {
            arg0.storageTexture = arg1;
        },
        __wbg_set_store_op_94575f47253d270d: function(arg0, arg1) {
            arg0.storeOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_strip_index_format_aeb7aa0e95e6285d: function(arg0, arg1) {
            arg0.stripIndexFormat = __wbindgen_enum_GpuIndexFormat[arg1];
        },
        __wbg_set_targets_93553735385af349: function(arg0, arg1) {
            arg0.targets = arg1;
        },
        __wbg_set_texture_6003a9e79918bf8a: function(arg0, arg1) {
            arg0.texture = arg1;
        },
        __wbg_set_texture_c5a457625c071b25: function(arg0, arg1) {
            arg0.texture = arg1;
        },
        __wbg_set_timestamp_writes_0603b32a31ee6205: function(arg0, arg1) {
            arg0.timestampWrites = arg1;
        },
        __wbg_set_topology_5e4eb809635ea291: function(arg0, arg1) {
            arg0.topology = __wbindgen_enum_GpuPrimitiveTopology[arg1];
        },
        __wbg_set_type_0e707d4c06fc2b7b: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuSamplerBindingType[arg1];
        },
        __wbg_set_type_6fe4c5f460401ee0: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuBufferBindingType[arg1];
        },
        __wbg_set_unclipped_depth_e9a2451e4fa0277a: function(arg0, arg1) {
            arg0.unclippedDepth = arg1 !== 0;
        },
        __wbg_set_usage_5abd566becc087bb: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_61967f166fba5e13: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_d0a75d4429098a06: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_f0bb325677668e77: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_vertex_2525cfcd959b2add: function(arg0, arg1) {
            arg0.vertex = arg1;
        },
        __wbg_set_view_57d232eea19739c3: function(arg0, arg1) {
            arg0.view = arg1;
        },
        __wbg_set_view_dimension_49cfda500f1dea55: function(arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_view_dimension_a669c29ec3b0813a: function(arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_view_ffadd767d5e9b839: function(arg0, arg1) {
            arg0.view = arg1;
        },
        __wbg_set_view_formats_70a1fcabcd34282a: function(arg0, arg1) {
            arg0.viewFormats = arg1;
        },
        __wbg_set_view_formats_83865b9cdfda5cb6: function(arg0, arg1) {
            arg0.viewFormats = arg1;
        },
        __wbg_set_visibility_088046ee77c33b1d: function(arg0, arg1) {
            arg0.visibility = arg1 >>> 0;
        },
        __wbg_set_width_0ca908d167690992: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_width_b0e1267db4b196b5: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_width_e96e07f8255ad913: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_write_mask_76041c03688571cd: function(arg0, arg1) {
            arg0.writeMask = arg1 >>> 0;
        },
        __wbg_set_x_fdd6aca9a2390926: function(arg0, arg1) {
            arg0.x = arg1 >>> 0;
        },
        __wbg_set_y_410a18c5811abf4c: function(arg0, arg1) {
            arg0.y = arg1 >>> 0;
        },
        __wbg_set_z_f7f1ae8afd3a9308: function(arg0, arg1) {
            arg0.z = arg1 >>> 0;
        },
        __wbg_shaderSource_06379dac249994dd: function(arg0, arg1, arg2, arg3) {
            arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
        },
        __wbg_shaderSource_c8812639b47d87b9: function(arg0, arg1, arg2, arg3) {
            arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_static_accessor_GLOBAL_266715b9d96ba635: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_10fb7dc1ae063179: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_0b583911f537483a: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_d7f903d1508cbdc4: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_stencilFuncSeparate_3f31dcd12a0d4785: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3, arg4 >>> 0);
        },
        __wbg_stencilFuncSeparate_f168b66c597aa85f: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3, arg4 >>> 0);
        },
        __wbg_stencilMaskSeparate_b30c097ea6b98504: function(arg0, arg1, arg2) {
            arg0.stencilMaskSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_stencilMaskSeparate_ff739f8711e9fd42: function(arg0, arg1, arg2) {
            arg0.stencilMaskSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_stencilMask_9e18cc018897c747: function(arg0, arg1) {
            arg0.stencilMask(arg1 >>> 0);
        },
        __wbg_stencilMask_a22be64c66abd825: function(arg0, arg1) {
            arg0.stencilMask(arg1 >>> 0);
        },
        __wbg_stencilOpSeparate_7d01185cbdb89d99: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilOpSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_stencilOpSeparate_c5af5f5dbd237bea: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilOpSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_submit_21302eebe551e30d: function(arg0, arg1) {
            arg0.submit(arg1);
        },
        __wbg_texImage2D_82bf2b4c8aa9a1db: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texImage2D_a856af552acdc9d9: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texImage2D_ca3d0fe24620ec85: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texImage3D_29a847f33877ed12: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.texImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8 >>> 0, arg9 >>> 0, arg10);
        }, arguments); },
        __wbg_texImage3D_78fb39719c620063: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.texImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8 >>> 0, arg9 >>> 0, arg10);
        }, arguments); },
        __wbg_texParameteri_647fdb1426809cb5: function(arg0, arg1, arg2, arg3) {
            arg0.texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        },
        __wbg_texParameteri_7864076abcdb03e0: function(arg0, arg1, arg2, arg3) {
            arg0.texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        },
        __wbg_texStorage2D_cf2bd991072d9efb: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.texStorage2D(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_texStorage3D_74b701c2b176380c: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.texStorage3D(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5, arg6);
        },
        __wbg_texSubImage2D_07337d0974e99c39: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_196472d4111ce5b6: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_24cf5097ebaa84cc: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_31a0c2797aa04f32: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_5e90c7a719d40256: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_64e36398b8f046ac: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_bb322aae90f7657f: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_d7bffff06a3b8315: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage3D_001f2e62ed945b72: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_5143d5af7e182ddc: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_53c6014e2d129891: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_5d773fc7a8cd4d08: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_adb96e37e6949ec9: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_d4e74595e01f7f71: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_e0d113b7047a5130: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_then_4e48da6c0a644c56: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_c949d5a25a4e78f8: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_e71170d78fcf8954: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_toString_3e46331018353800: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.toString(arg1);
            return ret;
        }, arguments); },
        __wbg_uniform1f_9cfecfaeb8cf43c1: function(arg0, arg1, arg2) {
            arg0.uniform1f(arg1, arg2);
        },
        __wbg_uniform1f_b5856e35a43b65a1: function(arg0, arg1, arg2) {
            arg0.uniform1f(arg1, arg2);
        },
        __wbg_uniform1i_531b98c4a7177366: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform1i_fc2dc8cadc49d8ec: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform1ui_353c67ec681b540c: function(arg0, arg1, arg2) {
            arg0.uniform1ui(arg1, arg2 >>> 0);
        },
        __wbg_uniform2fv_319c49f0ce340f26: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2fv_a7d3c1830d18c1b2: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2iv_65f36fc9777697d7: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2iv_ca9cb507409b9ad1: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2uiv_c95af5fb653ccb86: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2uiv(arg1, getArrayU32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3fv_055ea5ac38af4fce: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3fv_58520ffe22a6e27c: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3iv_0590e8b7eff2a791: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3iv_e91b1410dc299f50: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3uiv_9992491000463a52: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3uiv(arg1, getArrayU32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4f_22bfec882308eec2: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.uniform4f(arg1, arg2, arg3, arg4, arg5);
        },
        __wbg_uniform4f_8dd8f311e656ca0d: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.uniform4f(arg1, arg2, arg3, arg4, arg5);
        },
        __wbg_uniform4fv_3ef2f5121fcb78ae: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4fv_cf8ff646c36232c8: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4iv_6fc5436b4f876039: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4iv_99c2b07787d362d6: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4uiv_a6fb3237d9df7ed4: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4uiv(arg1, getArrayU32FromWasm0(arg2, arg3));
        },
        __wbg_uniformBlockBinding_07116582301fb019: function(arg0, arg1, arg2, arg3) {
            arg0.uniformBlockBinding(arg1, arg2 >>> 0, arg3 >>> 0);
        },
        __wbg_uniformMatrix2fv_34b2245136106a2f: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix2fv_611c7fcb378f057e: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix2x3fv_6b33a76d09b6c00f: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2x3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix2x4fv_66c787e2d0a5dd0a: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2x4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3fv_472e7824735acce1: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3fv_83fdc9c3c0d43204: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3x2fv_33df849b78b8556e: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3x2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3x4fv_8914dc5121a813b4: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3x4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4fv_dff9738d995fd0b1: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4fv_e691d7f14d3609f7: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4x2fv_1c9ccf4427b82548: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4x2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4x3fv_a84b16e0a7abee2c: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4x3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_unmap_b819b8b402db13cc: function(arg0) {
            arg0.unmap();
        },
        __wbg_useProgram_5bdae58fd9f9826f: function(arg0, arg1) {
            arg0.useProgram(arg1);
        },
        __wbg_useProgram_e075c2902e420761: function(arg0, arg1) {
            arg0.useProgram(arg1);
        },
        __wbg_vertexAttribDivisorANGLE_659fb027198a47c3: function(arg0, arg1, arg2) {
            arg0.vertexAttribDivisorANGLE(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_vertexAttribDivisor_7575d382215322f7: function(arg0, arg1, arg2) {
            arg0.vertexAttribDivisor(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_vertexAttribIPointer_fc8057c62cfcc63c: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.vertexAttribIPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_vertexAttribPointer_24b495e18df325dc: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        },
        __wbg_vertexAttribPointer_54d56b18f71a946d: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        },
        __wbg_viewport_16936692526e2ce6: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.viewport(arg1, arg2, arg3, arg4);
        },
        __wbg_viewport_e35d16c542d2ce70: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.viewport(arg1, arg2, arg3, arg4);
        },
        __wbg_warn_13abc63e0d4b3527: function(arg0) {
            console.warn(arg0);
        },
        __wbg_width_3d0dce3d9892e35e: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_writeBuffer_c6919ed0c4aaeef5: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.writeBuffer(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_writeTexture_340cfbecd9544755: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.writeTexture(arg1, arg2, arg3, arg4);
        }, arguments); },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 2912, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___wasm_bindgen_fe1d14e6b7d09dd3___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_fe1d14e6b7d09dd3___JsError___true_);
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 504, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___wasm_bindgen_fe1d14e6b7d09dd3___JsValue______true_);
            return ret;
        },
        __wbindgen_generic_0000000000000003: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_generic_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(F32)) -> NamedExternref("Float32Array")`.
            const ret = getArrayF32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(I16)) -> NamedExternref("Int16Array")`.
            const ret = getArrayI16FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000006: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(I32)) -> NamedExternref("Int32Array")`.
            const ret = getArrayI32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000007: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(I8)) -> NamedExternref("Int8Array")`.
            const ret = getArrayI8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000008: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U16)) -> NamedExternref("Uint16Array")`.
            const ret = getArrayU16FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000009: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U32)) -> NamedExternref("Uint32Array")`.
            const ret = getArrayU32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_000000000000000a: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_000000000000000b: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_000000000000000c: function(arg0) {
            // Cast intrinsic for `U64 -> Externref`.
            const ret = BigInt.asUintN(64, arg0);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./infinitecanvas_wasm_bg.js": import0,
    };
}

function wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___wasm_bindgen_fe1d14e6b7d09dd3___JsValue______true_(arg0, arg1, arg2) {
    wasm.wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___wasm_bindgen_fe1d14e6b7d09dd3___JsValue______true_(arg0, arg1, arg2);
}

function wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___wasm_bindgen_fe1d14e6b7d09dd3___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_fe1d14e6b7d09dd3___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___wasm_bindgen_fe1d14e6b7d09dd3___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_fe1d14e6b7d09dd3___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___js_sys_4cd4ef52bf73eaa4___Function_fn_wasm_bindgen_fe1d14e6b7d09dd3___JsValue_____wasm_bindgen_fe1d14e6b7d09dd3___sys__Undefined___js_sys_4cd4ef52bf73eaa4___Function_fn_wasm_bindgen_fe1d14e6b7d09dd3___JsValue_____wasm_bindgen_fe1d14e6b7d09dd3___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen_fe1d14e6b7d09dd3___convert__closures_____invoke___js_sys_4cd4ef52bf73eaa4___Function_fn_wasm_bindgen_fe1d14e6b7d09dd3___JsValue_____wasm_bindgen_fe1d14e6b7d09dd3___sys__Undefined___js_sys_4cd4ef52bf73eaa4___Function_fn_wasm_bindgen_fe1d14e6b7d09dd3___JsValue_____wasm_bindgen_fe1d14e6b7d09dd3___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_GpuAddressMode = ["clamp-to-edge", "repeat", "mirror-repeat"];


const __wbindgen_enum_GpuBlendFactor = ["zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"];


const __wbindgen_enum_GpuBlendOperation = ["add", "subtract", "reverse-subtract", "min", "max"];


const __wbindgen_enum_GpuBufferBindingType = ["uniform", "storage", "read-only-storage"];


const __wbindgen_enum_GpuCanvasAlphaMode = ["opaque", "premultiplied"];


const __wbindgen_enum_GpuCompareFunction = ["never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"];


const __wbindgen_enum_GpuCullMode = ["none", "front", "back"];


const __wbindgen_enum_GpuFilterMode = ["nearest", "linear"];


const __wbindgen_enum_GpuFrontFace = ["ccw", "cw"];


const __wbindgen_enum_GpuIndexFormat = ["uint16", "uint32"];


const __wbindgen_enum_GpuLoadOp = ["load", "clear"];


const __wbindgen_enum_GpuMipmapFilterMode = ["nearest", "linear"];


const __wbindgen_enum_GpuPowerPreference = ["low-power", "high-performance"];


const __wbindgen_enum_GpuPrimitiveTopology = ["point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"];


const __wbindgen_enum_GpuSamplerBindingType = ["filtering", "non-filtering", "comparison"];


const __wbindgen_enum_GpuStencilOperation = ["keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"];


const __wbindgen_enum_GpuStorageTextureAccess = ["write-only", "read-only", "read-write"];


const __wbindgen_enum_GpuStoreOp = ["store", "discard"];


const __wbindgen_enum_GpuTextureAspect = ["all", "stencil-only", "depth-only"];


const __wbindgen_enum_GpuTextureDimension = ["1d", "2d", "3d"];


const __wbindgen_enum_GpuTextureFormat = ["r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"];


const __wbindgen_enum_GpuTextureSampleType = ["float", "unfilterable-float", "depth", "sint", "uint"];


const __wbindgen_enum_GpuTextureViewDimension = ["1d", "2d", "2d-array", "cube", "cube-array", "3d"];


const __wbindgen_enum_GpuVertexFormat = ["uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"];


const __wbindgen_enum_GpuVertexStepMode = ["vertex", "instance"];
const InfiniteCanvasFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_infinitecanvas_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayI16FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}

function getArrayI32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayI8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getArrayU16FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

let cachedInt16ArrayMemory0 = null;
function getInt16ArrayMemory0() {
    if (cachedInt16ArrayMemory0 === null || cachedInt16ArrayMemory0.byteLength === 0) {
        cachedInt16ArrayMemory0 = new Int16Array(wasm.memory.buffer);
    }
    return cachedInt16ArrayMemory0;
}

let cachedInt32ArrayMemory0 = null;
function getInt32ArrayMemory0() {
    if (cachedInt32ArrayMemory0 === null || cachedInt32ArrayMemory0.byteLength === 0) {
        cachedInt32ArrayMemory0 = new Int32Array(wasm.memory.buffer);
    }
    return cachedInt32ArrayMemory0;
}

let cachedInt8ArrayMemory0 = null;
function getInt8ArrayMemory0() {
    if (cachedInt8ArrayMemory0 === null || cachedInt8ArrayMemory0.byteLength === 0) {
        cachedInt8ArrayMemory0 = new Int8Array(wasm.memory.buffer);
    }
    return cachedInt8ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint16ArrayMemory0 = null;
function getUint16ArrayMemory0() {
    if (cachedUint16ArrayMemory0 === null || cachedUint16ArrayMemory0.byteLength === 0) {
        cachedUint16ArrayMemory0 = new Uint16Array(wasm.memory.buffer);
    }
    return cachedUint16ArrayMemory0;
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedInt16ArrayMemory0 = null;
    cachedInt32ArrayMemory0 = null;
    cachedInt8ArrayMemory0 = null;
    cachedUint16ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('infinitecanvas_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };

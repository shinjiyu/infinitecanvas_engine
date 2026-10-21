# 项目进度追踪

**更新时间**: 2026-10-21

## Issue #107: SVG 导出增强（渐变 / 投影 / 模糊保真）

- [x] 线性渐变改为 `userSpaceOnUse`，起止点与画布投影轴一致（非正方形包围盒不再被 objectBoundingBox 拉伸）
- [x] 径向渐变保持 `userSpaceOnUse`；色标 / 圆心 / 半径与 `ElementStyle` 相同
- [x] 外投影 `feDropShadow`、图层模糊 `feGaussianBlur`、两者链式组合；旧文档无字段仍合法
- [x] viewBox 为投影伸出与模糊光晕留边（与 PNG 导出思路对齐）；画板裁剪仍裁到画板盒
- [x] README / scene-persistence / property-panel / demo 注明近似边界；内阴影（#89 已合入）用 `feComposite` 链导出，模糊打开时省略内阴影滤镜
- [x] 不新增快捷键

## Issue #109: Yjs 协同 polish（远端光标与选区）

- [x] `PresenceState` / `CollabSession::{set_local_presence,clear_local_presence,remote_presences,encode/apply_awareness_update}`
- [x] `MemoryProvider::sync` 交换 awareness（含离开 null）；场景 JSON / elements 不含 awareness
- [x] Demo overlay：远端光标标签 + 选区轮廓（`pointer-events: none`）；不画本地远端副本
- [x] 与 WebSocket `RemoteUser` 软协调（同 JSON 形状）；文档注明权限/冲突 UI 不在范围；无新快捷键

```text
PresenceState ── yrs Awareness ── MemoryProvider / overlay
                      │
                      └── 不进场景 JSON
```

## Issue #110: Path 节点编辑（锚点/控制柄，可撤销）

- [x] Engine：`PathPointHandles` / `stroke_polyline` / `path_edit`（命中、拖拽、`EditPathCommand`）；旧 Path JSON 无 `point_handles` 仍可读
- [x] 单一 Path 节点模式：双击或面板按钮进入；拖拽更新几何 + `mark_dirty`；Esc / 点空白退出（与选择态协调）
- [x] 空选/锁定/非 Path / 无变化不推历史
- [x] WASM 光标反馈；无冲突快捷键
- [x] PNG/SVG 走编辑后几何（展平折线）

```text
双击 / 面板 ── path_node_mode ── EditPath ── mark_dirty ── WebGPU / PNG / SVG
```
```

## 属性面板数值变换 (#111)

- [x] Engine：`TransformNumericPatch` / `selection_transform` / `apply_selection_transform`，复用 `TransformElementsCommand`；空选/锁定/非法为 no-op；`mark_dirty`
- [x] 语义：X/Y = 本地原点（`translation`，与手柄中心一致）；W/H = 世界尺寸（改 scale）；直线禁用 W/H；旋转与手柄同一字段（度）
- [x] 多选 mixed；批量写同一绝对值；无长宽比锁定
- [x] WASM + 属性面板数字框，Enter/blur 提交；非法不推历史；无新快捷键

```text
属性面板 X/Y/W/H/旋转 ── set_selection_transform ──► TransformElementsCommand
```

## Issue #80: 图层高斯模糊

- [x] 叶子 `blur_radius`（f32，缺省 0，`serde(default)`）。旧 JSON 仍有效。0 不写入文件
- [x] 形状（含画板）用轮廓 CDF 环；图片和文本用 5×5 纹理核。图片绕裁剪窗口外扩 3σ，不清 `crop`。命中仍是未模糊包围盒。布尔填充路径的孔不被扇形填实
- [x] 一次 `SetStyle` 改模糊。空选、锁定、没有变化不入栈。文本不重做光栅。`mark_dirty` 后顶点重写。不改渐变、混合模式或裁剪
- [x] WASM 与属性面板「图层模糊」滑块和数字框。多选不一致为 mixed。不新增快捷键
- [x] PNG 按同一近似采样并放大导出范围。图片核在裁剪窗口外的样本是透明。画板导出区域不外扩。SVG 用 `feGaussianBlur`，只有投影时仍是 `e{id}-shadow`。渐变同时打开时软边用包围盒中心的一个颜色

```text
面板滑块松手
        │
        ▼
StylePatch.blur_radius ── SetStyle ── 一条历史
        │
        ├─ 形状 / 画板：填充缓冲里的高斯环
        └─ 图片 / 文本：外扩四边形 + fs_blur
命中 AABB 不变；JSON 字段 blur_radius，默认省略
```

## Issue #90: 图片裁剪

- [x] `Image::crop` 是归一化窗口，`serde(default)` 为整张图，与缺省值相同时不写入。旧 JSON 仍有效
- [x] 裁剪不改 `props.transform`、显示尺寸、资源像素和 `blur_radius`。命中、选中框、顶点和投影都跟窗口走
- [x] 一条 `SetImageCrop`。空选、锁定或没有变化不入栈。执行和撤销 `mark_dirty`
- [x] WebGPU 采样合成后的 UV。PNG 只采样窗口。SVG 把嵌套 `<svg>` 收到窗口上，变换矩阵不动
- [x] 属性面板给出 0–1 的左/上/宽/高，多选不一致为 mixed。预览松手一条历史。「裁剪手柄」复用缩放手柄，不新增快捷键。Esc 先退出裁剪模式

```text
面板数字 / 裁剪手柄
        │
        ▼
CropPatch ── SetImageCrop ── 一条历史
        │
        ├─ 局部窗口 = crop × size（变换不动）
        ├─ 顶点 / 命中 / 投影
        └─ PNG 采样窗口，SVG 嵌套 svg 偏移
```

## Issue #76: 混合模式

- [x] `BlendMode`：`normal` / `multiply` / `screen` / `overlay` / `darken` / `lighten`。`serde(default = Normal)`，Normal 不写入 JSON
- [x] 形状写在 `ElementStyle`，文本和图片各有字段。组展开到子孙叶子。画板用自己的样式
- [x] 改混合模式是一条 `SetStyle`。空选、锁定、无变化不入栈。`mark_dirty` 刷新绘制区间。和投影、渐变、图层模糊一样，未写出的字段保持原值
- [x] WebGPU：全部为 Normal 时仍批量绘制。有非 Normal 叶子时按 z 逐叶子合成（填充+描边+纹理一层，再 W3C 混合）
- [x] 属性面板下拉。多选不一致为 mixed。不新增快捷键
- [x] PNG 使用同一套公式。SVG 映射到 CSS `mix-blend-mode`，Normal 省略

```text
属性面板 blendMode ── SetStyle ──► 叶子 blend_mode
                                      │ mark_dirty
                                      ▼
                         VertexCache 绘制区间 ──► WebGPU / PNG / SVG
```

## Issue #73: 线性 / 径向渐变填充

- [x] `ElementStyle` 增加 `fill_kind` / `fill_stops` / `fill_angle` / `fill_center` / `fill_radius`，全部 `serde(default)`，缺省纯色
- [x] WebGPU 把渐变烤进填充顶点（线性按色标切条，径向为射线近似）；描边保持纯色；不透明度、隐藏和绘制顺序不变
- [x] 改渐变走一条 `SetStyle`；空选和无变化不入栈；执行和撤销 `mark_dirty`
- [x] JSON 往返；旧文件没有这些字段仍能打开。PNG 按像素采样，SVG 写 `linearGradient` / `radialGradient`
- [x] 属性面板可选纯色、线性、径向和色标。多选不一致为 mixed。不新增快捷键。`Ctrl/Cmd+'` 仍是网格，`G` 仍是标尺。投影字段保持可选，未写出则不动

```text
属性面板类型 / 色标
        │
        ▼
StylePatch ── SetStyleCommand ── 一条历史
        │
        ├─ 填充顶点（线性条带 / 径向射线）
        ├─ PNG 采样，SVG linearGradient / radialGradient
        └─ 描边仍是纯色
```

## Issue #79: 布尔运算 Union / Subtract / Intersect / XOR

- [x] `BooleanOp`：并集、相减（最上层 − 其余）、交集、异或。可运算叶子少于 2 个不入栈
- [x] 矩形、圆、多边形、已有填充路径，以及首尾闭合的自由路径。锁定、隐藏、组、画板、直线、文本、图片跳过
- [x] 结果是一条叶子 `Path`（`shapes`：外轮廓 + 孔），替换操作数。样式（含投影，以及 `serde(default)` 的渐变字段）和 `z_index` 取最上层。`shapes` 为 `serde(default)`，旧文档仍有效
- [x] 一条 `BooleanElementsCommand`。`mark_dirty`。组或画板里的结果留在原父节点，不改画板尺寸，不重写其他元素的 `z_index`。Ctrl+Z 一次还原
- [x] WASM：`boolean_selection` / `boolean_selection_ready`。Demo 工具栏和属性面板四个按钮，无新快捷键

```text
选中 ≥2 个闭合叶子
        │  跳过锁定 / 隐藏 / 画板 / 不支持
        ▼
i_overlay ── Union / Subtract(前−后) / Intersect / XOR
        │
        ▼
一条叶子 Path.shapes ── 替换操作数 ── 一条历史
```

## Issue #74: 画板 / Frame

- [x] `Frame`：名称、尺寸、样式、子元素。JSON `type: frame`，字段 `serde(default)`。旧文档仍有效
- [x] 子元素优先命中；Alt 选中画板。画布不裁剪；PNG / SVG 导出裁到画板世界包围盒，空画板仍导出矩形尺寸
- [x] 创建、重命名、移入画板、缩放都可撤销。空操作不入栈，成功路径 `mark_dirty`
- [x] Demo：工具栏画板（F）。F 不占用 V/L/P/R/O/T/H/I/M/G/N/K。双击空白处或属性面板改名称。图层面板把画板当容器

```text
F 拖拽 ──► AddElementCommand ──► Frame
重命名 / 移入 ──► RenameFrame / ReparentIntoFrame ──► mark_dirty
导出 ──► 世界包围盒 + 子树；画布绘制不裁剪
```

## Issue #89: 内阴影

- [x] 形状（含画板）、文本、图片各有 `inner_shadow`（偏移、模糊、扩展、颜色含浓度）。全零默认不写入 JSON
- [x] 填充缓冲在本体之后画向内羽化环；与外投影可同时开。命中和包围盒不含内阴影
- [x] 一次 `SetStyle` 改内阴影；空选与无变化不入栈；不丢文本光栅
- [x] WASM + 属性面板（紧挨投影）；无新快捷键
- [x] PNG 含内阴影；SVG 为 `feComposite` 近似

## Issue #70: 投影

- [x] 形状（含多边形和星形）、文本、图片各有 `shadow`（局部偏移、模糊、颜色含浓度）。全零默认值不写入 JSON，旧文档仍能打开
- [x] 填充缓冲在元素本体前画偏移轮廓；模糊用外扩羽化环。命中和包围盒不包含投影
- [x] 一次 `SetStyle` 改投影，空选和不改变的补丁不入栈。文本只改投影时保留光栅。边数和内径仍是同一条命令上的可选字段，未写出则保持原值，方便渐变和混合模式以后加自己的字段
- [x] WASM 与属性面板：偏移、模糊、颜色、浓度。多选不一致为 mixed。勾选一次写入默认投影，取消只把浓度写成 0
- [x] 吸管只写填充和描边。导入 SVG 不读 `feDropShadow`，新元素投影关闭

```text
面板勾选 / 滑块松手
        │
        ▼
StylePatch.shadow_* ── SetStyle ── 一条历史
        │
        ▼
mark_dirty ── 填充缓冲：羽化轮廓，然后元素本体
        │
        └─ 命中 AABB 不变；JSON 嵌套 shadow，默认省略
```

## Issue #63: 导入 SVG

- [x] `import_svg`：rect / circle / ellipse / line / polyline / polygon / path / text，嵌套 `g` 的变换压进子元素
- [x] 一次导入一条 `ImportSvgCommand`，撤销删掉整批
- [x] fill / stroke / stroke-width / opacity，以及线帽、线结、虚线、圆角（能映射时）
- [x] 空文本、非法 XML、没有可识别图元返回错误，不改场景
- [x] Demo：「导入 SVG」和拖放 `.svg`。viewBox 原点落在指针；没有指针时用 (24, 24)。PNG/JPG 拖放和粘贴仍走图片
- [x] SVG 的 `polygon` 写成自由路径，不占用 N / K 的正多边形和星形

```text
.svg / 导入 SVG
    │ anchor = 指针世界坐标，或 (24, 24)
    ▼
import_svg ── ImportSvgCommand（一条历史）
    ├─ rect / circle / ellipse / line / text / image(data URL)
    └─ path / polygon / polyline → Path
```

## Issue #65: 多边形与星形

- [x] `Polygon`：`kind` 为正多边形或星形。默认六边形（6）和五角星（5）。边数钳到 3–64
- [x] 星形 `inner_radius` 是内点相对外接圆的比例，默认 0.5，钳到 0.05–0.95
- [x] 填充扇形、描边、命中和轴对齐包围盒。旋转和缩放走现有 `transform`
- [x] 创建走 `AddElementCommand` 并 `Scene::mark_dirty`。改边数或内径是一条 `SetStyle`
- [x] JSON `type: polygon`，`kind` / `sides` / `inner_radius` 都有 `serde(default)`。`type: star` 可导入。旧文档仍有效
- [x] Demo：工具栏多边形（N）和星形（K）。属性面板在选中时显示边数，星形另显示内径。N / K 不占用 V/L/P/R/O/T/H/I

```text
N / K 拖拽 ──► AddElementCommand ──► Polygon
属性面板边数 / 内径 ── preview / commit ──► SetStyle.sides / inner_radius
                                              ├─ 边数 3–64，内径只写星形
                                              └─ mark_dirty ──► 扇形顶点
```

## Issue #64: 背景网格开关

- [x] 世界坐标网格，间距 20；缩小到不足 8px 时间距 ×5，仍落在 20 的倍数上
- [x] WASM：`set_grid_visible` / `grid_visible`，默认可见，不进撤销栈
- [x] 视口不变时不重传网格顶点；平移缩放后线的世界位置不变
- [x] Demo：工具栏「网格」。`Ctrl/Cmd+'` 登记在 `www/shortcuts.js`，`?` 帮助列出。`G` 仍是标尺，`I` 仍是吸管，`Ctrl/Cmd+=` / `-` / `0` 仍是缩放
- [x] `localStorage` 键 `infinitecanvas.gridVisible`，只有 `"false"` 才在下次打开时隐藏

```text
Ctrl/Cmd+'  /  工具栏
        │
        ▼
set_grid_visible ──► 下一帧画或不画网格
        │
        └─ localStorage infinitecanvas.gridVisible
```

## Issue #60: 吸管取色

- [x] `sample_paint_at`：最上层可见叶子的填充/描边；空白、隐藏为无命中；图片两个通道都是空
- [x] `apply_sampled_paint`：经 `SetStyle` 写到可见且未锁定的目标，多选一条撤销；空选、锁定、全隐藏、无颜色不入栈
- [x] 填充同时写形状 fill 和文本颜色；缺的通道不清掉目标上的颜色
- [x] WASM：`sample_style_at`、`apply_sampled_style_at`；吸管工具单击采样，松开后回到之前的工具
- [x] Demo：`I` 与工具栏按钮。不带 Shift / Ctrl / Alt，不占用 V/L/P/R/O/T/H/G。`?` 帮助面板列出吸管

```text
I / 工具栏
    │
    ▼
世界点 ── sample_paint_at（可见叶子，z 然后 id）
    │
    ▼
当前选择里可见且未锁定的叶子 ── SetStyle ── 一条历史
```

## Issue #61: 键盘缩放

- [x] `ZOOM_STEP`（1.1）是滚轮和键盘共用的一步。`zoom_by` 绕视口中心调用 `zoom_at`，上下限仍是视口 0.1×–10×
- [x] `Ctrl/Cmd+=` 或 `++` 放大，`Ctrl/Cmd+-` 缩小，`Ctrl/Cmd+0` 与 `Shift+0` 一样 `set_zoom(1)`，中心不动
- [x] 和弦在 `SHORTCUTS` 里，经 `dispatchShortcut` 进 WASM。`?` 面板列出放大、缩小，以及 100% 的两套键
- [x] 页脚把 `get_zoom()` 显示成百分比，键盘和滚轮每帧都会刷新
- [x] 输入框不触发。会挡住浏览器缩放的三组键只在画布 / 舞台 / `body` 上 `preventDefault`

```text
Ctrl/Cmd+=  Ctrl/Cmd+-  Ctrl/Cmd+0
        │
        ▼
shouldApplyShortcut（画布 / 舞台 / body；输入框和按钮不接管）
        │
        ▼
zoom_by(zoom_step) / set_zoom(1) ──► zoom_at 钳制到 min_zoom..max_zoom
        │
        ▼
页脚 formatZoomPercent(get_zoom())
```

## Issue #56: 标尺与手动参考线

- [x] 顶边 / 左边标尺，刻度随平移和缩放更新（世界单位，1/2/5 步长）
- [x] 从标尺拖出水平 / 垂直参考线；拖动改位置；拖回标尺、双击或 Delete 删除
- [x] 参考线进场景 JSON（`guides[]`，空则省略）；旧文档仍能打开
- [x] 拖拽和缩放吸附到最近参考线，阈值沿用 #14 的屏幕像素
- [x] PNG / SVG 默认不画参考线，`include_guides` 才画
- [x] Demo：标尺参考线按钮。`G` 登记在 `www/shortcuts.js`，帮助面板列出「显示或隐藏标尺和参考线」。`Ctrl+G` 仍是成组

```text
顶标尺 / 左标尺
    │ 拖出
    ▼
Scene.guides ── JSON guides[]
    │
    ├─ 拖拽：snap_drag_with_guides（和元素边缘比一次最近距离）
    └─ 缩放：移动中的元素边，斜率修正指针
```

## Issue #57: 锁定/隐藏快捷键

- [x] `SetLockedCommand` / `SetVisibilityCommand`：当前选择或指定 id，一条历史
- [x] 选中组只改组自己的标志。隐藏后子孙命中和框选按 `is_effectively_visible` 跳过
- [x] 锁定后不可平移/变换，选择保留；画布命中、框选、全选仍跳过。祖先已锁定时不越过祖先改子元素
- [x] `flags_epoch` 前进，图层面板重读眼睛和锁；不推进 `structure_epoch`
- [x] WASM：`toggle_selection_locked` / `toggle_selection_visible`，`set_element_locked` / `set_element_visible` 走同一条命令
- [x] Demo：`Ctrl/Cmd+L`、`Ctrl/Cmd+Shift+H` 已登记到 `www/shortcuts.js`，帮助面板列出锁定和隐藏

```text
Ctrl/Cmd+L  /  Ctrl/Cmd+Shift+H
        │
        ▼
toggle_selection_* ──► SetLockedCommand / SetVisibilityCommand
                              ├─ 一条历史
                              └─ flags_epoch（图层面板重读，顶点缓存不整表重排）
```

## Issue #55: 右键上下文菜单

- [x] Demo 在画布上拦截 `contextmenu`：空白处与命中元素弹出不同菜单
- [x] 选中菜单：复制、粘贴（剪贴板有文本）、克隆、删除、成组 / 解组（适用时）、置顶 / 置底 / 前移 / 后移
- [x] 空白菜单：粘贴、全选、缩放到适合
- [x] 每一项调用 `dispatchShortcut`，和 `SHORTCUTS`、工具栏同一条入口
- [x] 键帽用 `formatChord` 从 `SHORTCUTS` 取出，和 `?` 面板同一组字符串
- [x] Esc 或点击菜单外关闭；菜单打开时这一下指针不进入引擎。Esc 只关菜单，不取消选择

```text
右键
  ├─ 命中已选中（或选中组的子孙）──► 选中菜单
  ├─ 命中未选中 ──► select_element ──► 选中菜单
  └─ 空白 ──► 空白菜单
        │
        └─ dispatchShortcut ──► 复制 / 粘贴 / 克隆 / 删除 / 成组 / 层级 / 全选 / 缩放到适合
```

## Issue #50: 圆角矩形

- [x] `Rectangle::corner_radius`，默认 0，`serde(default)`；0 不写出。超过短边一半时钳制
- [x] WebGPU 填充和描边走圆角轮廓；半径 0 仍是原来的 6 个填充顶点。虚线细描边沿同一轮廓切开，不盖一条实线
- [x] 命中保持轴对齐外接矩形；包围盒不随圆角缩小
- [x] `SetStyle` 补丁 `corner_radius`，一条撤销记录，并 `mark_dirty`
- [x] `selection_style` / `set_selection_style` 暴露 `cornerRadius`；多选不一致为 mixed；非矩形为 absent
- [x] Demo 属性面板：选中矩形时显示圆角滑条和数字框，否则隐藏

```text
属性面板圆角 ── preview / commit ──► SetStyle.corner_radius
                                          ├─ 只写矩形，钳到短边一半
                                          └─ mark_dirty ──► 圆角轮廓顶点
```

## Issue #54: 缩放到选中 / 适应全部

- [x] `camera_for_bounds`：每边留视口 10%，缩放限制在 0.1×–10×
- [x] `zoom_to_selection`：可见、未锁定；组用组根包围盒。空选不改相机
- [x] `zoom_to_fit` / `fit_scene`：可见叶子，与小地图 `scene_bounds` 相同。空场景回到原点、缩放 1
- [x] 只改相机，不改几何，不进撤销栈，不推进 `geometry_epoch`
- [x] Demo：工具栏「适应全部」「缩放到选中」
- [x] `Shift+1`（`zoomFit`）和 `Shift+2`（`zoomSelection`）在 `SHORTCUTS` 里，`?` 帮助面板列出
- [x] `zoomFit` 调用 `zoom_to_fit` / `fit_scene`；`zoomSelection` 调用 `zoom_to_selection`
- [x] `Shift+0` 仍是 100%。`Shift+H` / `Shift+V` / `Shift+R` / `Shift+Alt+R` 仍是翻转和 90° 旋转

```text
选中 / 可见叶子 ──► camera_for_bounds ──► Viewport center + zoom
空选 ──► 不动
空场景 ──► 原点，1×
```

## Issue #49: 翻转与 90° 旋转

- [x] `OrientSelectionCommand`：水平/垂直翻转，顺时针/逆时针 90°，各一条历史
- [x] 绕选区中心；单选即元素中心。只改平移、旋转和缩放符号，并标脏
- [x] 选中组只写组根，子孙跟着走，不拆组。空选、锁定、隐藏不入栈
- [x] 图片翻转不改 `size`，绝对缩放比不变
- [x] WASM：`flip_selection(axis)`、`rotate_selection_quarter(dir)`
- [x] Demo：工具栏和属性面板按钮；`Shift+H` / `Shift+V` / `Shift+R` / `Shift+Alt+R`
- [x] `Shift+H` / `Shift+V` / `Shift+R` / `Shift+Alt+R` 已登记到 `www/shortcuts.js`，帮助面板列出水平翻转、垂直翻转、顺时针 90°、逆时针 90°

```text
Shift+H / Shift+V / Shift+R / Shift+Alt+R
        │
        ▼
nudge_targets ──► OrientSelectionCommand ──► set_world_transform
                                               ├─ 一条历史
                                               └─ mark_dirty
```

## Issue #45: 层级快捷键

- [x] `ReorderSelectionCommand`：置顶 / 置底 / 前移 / 后移，同一父节点下的兄弟，一条历史
- [x] 选中组只移动组根；跨父或空选不入栈；锁定（含祖先）跳过，隐藏仍可重排
- [x] `structure_epoch` 前进，顶点缓存整表重排；不把元素标成几何脏
- [x] WASM：`reorder_selection(action)`，`front` / `back` / `forward` / `backward`
- [x] Demo：图层面板四个按钮，以及 `]` / `[` / Ctrl+] / Ctrl+[（或 Cmd）
- [x] `]` / `[` / Ctrl+] / Ctrl+[ 已登记到 `www/shortcuts.js`，帮助面板列出前移、后移、置顶、置底

```text
] / [ / Ctrl+] / Ctrl+[
        │
        ▼
reorder_selection ──► ReorderSelectionCommand ──► arrange_siblings
                                                      ├─ 一条历史
                                                      └─ structure_epoch（顶点缓存重排）
```

## Issue #40: 多选对齐与分布

- [x] `AlignElementsCommand`：左 / 水平居中 / 右 / 顶 / 垂直居中 / 底，参考框是目标世界包围盒的并集
- [x] `DistributeElementsCommand`：水平、垂直均匀间隙，两端不动
- [x] 只改平移；选中组移动组根；标脏；一条撤销记录。单选、空选、不足目标数不入栈
- [x] WASM：`align_selection(mode)`、`distribute_selection(axis)`
- [x] Demo：工具栏和属性面板按钮，多选时才可点。不改拖拽吸附

```text
多选 ──► nudge_targets（可见、未锁定、选中子树的根）
              │
              ├─ AlignElementsCommand ── 贴齐并集边或中心
              └─ DistributeElementsCommand ── 中间元素拉开相等间隙
                        │
                        └─ translate_element ── mark_dirty，一条历史
```

## Issue #43: 方向键微调

- [x] `NudgeElementsCommand`：顶层可见、未锁定；选中组则平移组根。只改平移，标脏并入撤销栈
- [x] 步长 1 世界单位，与缩放无关；Shift ×10。多选一条历史
- [x] 单选 / 多选 / 组、Shift、撤销重做、锁定或空选 no-op
- [x] Demo：非输入框方向键调用 `nudge_selection`；不抢空格平移、撤销、复制粘贴和工具键
- [x] 方向键和 Shift+方向键已登记到 `www/shortcuts.js`，帮助面板列出 1 倍和 10 倍

```text
方向键 ──► nudge_targets ──► NudgeElementsCommand ──► translate_ids
                                                      ├─ 一条历史
                                                      └─ mark_dirty
```

## Issue #32: 图片元素

- [x] Image：资源 id、原始像素尺寸、显示尺寸、变换
- [x] 场景资源表：同一张图只存一份像素，最后一个引用释放
- [x] 纹理四边形沿用现有管线；GPU 纹理随资源引用上传和释放
- [x] 命中测试、选中框、变换手柄；图片默认锁定宽高比，Shift 自由缩放
- [x] 移动或变换只 `mark_dirty` 该元素
- [x] JSON 以 data URL 存一份资源，元素引用 asset id；PNG 导出采样像素
- [x] SVG：有像素时嵌 `<image>` data URL（变换、透明度、UV 裁剪）；组的可见性与选区和其他叶子一致。没有字节的纹理仍跳过
- [x] wasm `add_image_bytes`；Demo 拖放文件。Ctrl+V 有图片数据时粘贴图片，否则走 #33 的元素 JSON
- [x] 边长超过 4096 先降采样
- [x] 不透明度走 `SetStyle`（`Image::alpha`），和形状、文本一起改；资源表不因改透明度而变化

## Issue #29: 小地图

- [x] 场景包围盒 → 小地图像素，以及像素反算世界坐标（空场景、单元素、极端宽高比）
- [x] 缩略图只画可见叶子的矩形外框；组不重复绘制
- [x] 点击跳转相机中心，拖拽视口框按像素偏移平移
- [x] `geometry_epoch` 变化才重算外框；平移/缩放只更新视口框
- [x] WASM：`scene_bounds`、`set_camera_center`、`minimap_epoch`、`minimap_element_rects`、`minimap_viewport_rect`、`minimap_to_world`
- [x] Demo 右下角小地图，`M` 显示/隐藏。滚轮停在小地图上，不缩放主画布

## 属性面板 (#35)

- [x] `SetStyle`：填充、描边、线宽、不透明度；文本额外字号和颜色
- [x] 执行 / 撤销 / 重做调用 `Scene::mark_dirty`；文本字号、颜色、描边会重新光栅化
- [x] 多选不一致时 `selection_style` 返回 mixed，一次补丁写到全部目标
- [x] 滑块预览只在松手时入栈一条历史
- [x] 样式经场景 JSON 往返；旧 JSON 用 `serde(default)` 导入
- [x] Demo 右侧属性面板，未选中时隐藏
- [x] 图片参与不透明度；填充、描边、线宽仍只作用于形状和文本

```text
属性面板 ──preview / commit──► SetStyle ──► mark_dirty
                                    │
                                    ├─ 形状：ElementStyle → 顶点缓存
                                    ├─ 文本：字号/颜色/描边 → 重新光栅
                                    └─ 图片：不透明度 → Image::alpha（资源表不动）
```

## Issue #33: 复制 / 粘贴 / 克隆

- [x] `Scene::duplicate`：新 ID、样式和变换保留、z 整块高于现有元素
- [x] 组子树一起复制，`parent` / `children` 按新 ID 重映射；子元素克隆仍挂在原组下
- [x] 剪贴板复用 version 1 JSON（含选中图片的 `assets[]`）；`paste_json` 可把包围盒中心对准焦点。粘贴和克隆增加资源引用，撤销归零时像素随命令暂存
- [x] `DuplicateElementsCommand` / `PasteElementsCommand`：撤销删除，重做按原 ID 恢复
- [x] 新元素进入 `Scene::mark_dirty`
- [x] Demo：Ctrl+C / Ctrl+V / Ctrl+D；系统剪贴板失败退回内存，不抛错
- [x] Ctrl+C / Ctrl+V / Ctrl+D 已登记到 `www/shortcuts.js`，和帮助面板共用这一张表

---

## Issue #4: 文本元素

- [x] Text 元素：内容、字号、颜色、对齐
- [x] Canvas2D / OffscreenCanvas 光栅化，复用 TextureManager
- [x] 按缩放 LOD 重新光栅化
- [x] T 工具单击创建，双击 DOM 编辑
- [x] 命中测试 + 创建/改内容撤销
- [x] Headless Chrome 光栅化：`Hello` / `你` / `好` 都有墨迹，中英文字形不同，1× 与 2× 的世界尺寸一致
- [x] 版本 1 JSON 往返：内容、字号、颜色、对齐、旋转、缩放；光栅字段 `#[serde(skip)]`
- [x] PNG 用 font8x8 点阵画文本（无点阵的字符画方框）
- [x] 双击计时改用 `web_time::Instant`
- [x] 创建、改内容、光栅提交、撤销/重做调用 `Scene::mark_dirty`，顶点缓存在文本变化后更新

---

## M1: 核心渲染引擎 (11/11) - ✅ 已完成

- [x] M1.1: Rust WASM 基础框架
- [x] M1.2: 基础数学库
- [x] M1.3: 视口管理器
- [x] M1.4: Canvas 2D 渲染器（临时实现）
- [x] M1.5: 元素数据结构
- [x] M1.6: 场景容器
- [x] M1.7: 渲染循环
- [x] M1.8: 背景网格
- [x] M1.9: WASM-JS 接口
- [x] M1.10: Web 集成测试
- [x] M1.11: WebGPU 渲染器完整迁移

**WebGPU 迁移进度**:
- [x] 阶段 1: 修复核心数据结构（Color, Vertex, Mat4）
- [x] 阶段 2: 添加顶点生成（Rectangle, Circle）
- [x] 阶段 3: WebGPU 渲染器框架（已测试，60 FPS）
- [x] 阶段 4: 基础 Shader (basic.wgsl)（已测试，矩形和圆形正常渲染）
- [x] 阶段 5: 图片/纹理支持（✅ 已完成）
  - [x] 设计架构文档
  - [x] 修复 wgpu 27 API 兼容性（write_texture）✅ 解题记录已添加
  - [x] TextureManager 完整实现
  - [x] 纹理采样器创建
  - [x] 纹理 BindGroup 布局
  - [x] 纹理 Pipeline 创建
  - [x] 纹理 Shader (textured.wgsl)
  - [x] Vertex 扩展（支持 UV 坐标）
  - [x] Image 元素类型（含顶点生成）
  - [x] AnyElement 和 Scene 更新
  - [x] JS 互操作接口 (upload_texture, add_image)
  - [x] 纹理渲染逻辑集成
  - [x] 测试纹理渲染（✅ 60 FPS，所有测试通过）
- [x] 阶段 6: 测试优化（✅ vertex_offset 修复，Vite 配置优化，测试完全通过）

**状态**: ✅ M1 完成 - WebGPU 渲染引擎（含纹理支持）全部实现并通过测试

---

## M2: 交互系统 (10/10) - ✅ 已完成

- [x] M2.1: 事件系统
- [x] M2.2: 命中测试
- [x] M2.3: 选择管理器
- [x] M2.4: 框选（视觉反馈：选中高亮边框 + 框选矩形）
- [x] M2.5: 元素拖动（选中后拖动移动位置）
- [x] M2.6: 命令系统（Command trait, CommandHistory）
- [x] M2.7: 撤销/重做（Ctrl+Z/Ctrl+Y）
- [x] M2.8: 工具系统（选择/矩形/圆形/平移工具）
- [x] M2.9: 键盘快捷键（V/R/O/H + 空格临时切换）
- [x] M2.10: UI 层集成（get_state, get_selection_bounds, get_element_info）

**状态**: ✅ M2 完成 - 完整的交互系统已实现

---

## 变换手柄 (#7)

- [x] 选中框 8 个缩放手柄 + 1 个旋转手柄（单选跟元素旋转对齐，多选整组变换）
- [x] Shift 缩放保持比例，Shift 旋转按 15° 吸附
- [x] `TransformElementsCommand` 进入 `CommandHistory`，Ctrl+Z 可撤销
- [x] 元素 `rotation` 字段（`ElementProps::rotation`），命中测试走逆变换，覆盖矩形、圆、直线、路径和图片

## 直线 / 自由画笔 (#8)

- [x] `Line`（可选箭头）与 `Path`：沿采样点生成带线宽的三角形带，斜接拐角，过长则斜角
- [x] 命中测试使用点到线段距离（细线另加少量点选余量）
- [x] 工具 `L` / `P`，抬笔后写入撤销栈，可选中并拖动
- [x] WASM：`add_line`、`add_path`、`set_tool("line" | "pen")`

**依赖**: M1 完成

## 快捷键 (#26)

- [x] `www/shortcuts.js` 一张表同时驱动按键分发和 `?` 帮助面板
- [x] 保留 V/L/P/R/O/T/H、空格临时平移、Ctrl/Cmd+Z/Y/Shift+Z、Ctrl/Cmd+S/O
- [x] 补齐删除（Delete/Backspace）、全选（Ctrl/Cmd+A）、Esc 取消选择、Shift+1 缩放到适合、Shift+0 为 100%
- [x] 输入框、textarea、select、contenteditable 聚焦时不触发；Mac 显示 ⌘，其他平台显示 Ctrl
- [x] Ctrl/Cmd+C / V / D 复制、粘贴、原地克隆，走同一张表
- [x] Ctrl/Cmd+V 与图片粘贴共用：剪贴板有 PNG/JPEG 时粘贴图片，否则粘贴元素 JSON
- [x] 方向键微调选中元素（1 世界单位），Shift+方向键为 10 倍；带 Ctrl / Alt / ⌘ 不触发
- [x] `]` / `[` 前移 / 后移，Ctrl/Cmd+] / Ctrl/Cmd+[ 置顶 / 置底
- [x] Shift+H / Shift+V 水平或垂直翻转，Shift+R 顺时针 90°，Shift+Alt+R 逆时针 90°；不带 Shift 的 H / V / R 仍是工具键
- [x] WASM 只新增 `delete_selection`、`select_all`、`zoom_to_fit`、`set_zoom`
- [x] #4 文本已合并：`T` 切换文本工具，编辑框里 Enter 确认、Esc 取消
- [x] #13 已合并：`Ctrl+G` 成组（`group_elements`），`Ctrl+Shift+G` 取消分组（`ungroup`）
- [x] 删除组会快照整棵子树，撤销后子孙还在；全选只选顶层可见元素，并跳过锁定元素；`content_bounds` 跳过组节点和隐藏元素

```text
SHORTCUTS ──► matchShortcut ──► 画布动作
     └──────► helpEntries ──► ? 帮助面板
```

## 对齐与吸附 (#14)

- [x] 拖拽时用选区包围盒对齐其他可见元素的左/右/上/下边缘和水平/垂直中心
- [x] 屏幕像素阈值，除以缩放后得到世界距离；可开关
- [x] 参考线可查询（`axis` + `position`），Demo 画在画布上
- [x] 几何单测覆盖吸附结果、参考线和多选并集

```text
pointer delta ──► 未吸附包围盒 ──► snap_bounds ──► 修正 + 参考线
                                      │
                                      ▼
                              其他可见元素包围盒
```

---

## 场景持久化 (#6) - ✅ 已完成

- [x] 版本化场景 JSON（`version`、`next_id`、`elements`），缺省字段用 serde default，未知字段忽略
- [x] 覆盖矩形、圆、图片、直线（含箭头）、自由路径，以及 `transform.rotation` / `scale`
- [x] WASM `export_json` / `import_json` / `export_png`
- [x] 视口或选中区域 PNG
- [x] Demo 保存/打开与 localStorage 自动保存
- [x] JSON round-trip 与 PNG 单元测试

**依赖**: M1 完成。#7 已合并：`rotation` / `scale` 写在 `Transform` 上并参与往返。Text（#4）以 `type: "text"` 进入同一版本，`parent` / `locked` 一起往返。`dirty` 和文本光栅字段都不写入 JSON。分组（#13）用 `type: group` 写入 `children` 和 `parent`。未知字段仍会被忽略。

---

## 分组与场景树 (#13)

- [x] `Group` 元素，按顺序保存子元素 ID；`ElementProps.parent` / `locked`
- [x] `Scene::group` / `ungroup` / `enter_group` / `leave_group`
- [x] 平移组时子孙世界坐标同步；旋转和均匀缩放用 `Transform::rebase_between`（与 #7 手柄同一条写回路径）
- [x] `Scene::scene_tree()`：根 → 组 → 叶子，含可见性与锁定
- [x] 单测：成组/解散、子元素命中、组变换、场景 JSON 往返（`type: group` + `children` / `parent`）
- [x] 组平移、旋转、缩放和显隐会把子树标脏，顶点缓存只重写这些元素
- [x] 拖拽吸附修正走 `translate_ids`；选中组用叶子包围盒并集，不吸附到组本身或正在拖动的子孙
- [x] Text 可以进组，随组平移/缩放/旋转并标脏；JSON 写出 `parent` / `locked`，光栅字段仍 `serde(skip)`

---

## SVG 导出 (#25) - ✅ 已完成

- [x] `export_svg`：矩形、圆、直线/箭头、自由路径、文本，复用 PNG 的 `elements_in_paint_order`
- [x] 平移 / 缩放 / 旋转、填充、描边、线宽、透明度、z 顺序
- [x] 默认导出全部可见叶子的包围盒；没有叶子时 viewBox 为 `0 0 1 1`
- [x] `selected_only` 导出自身被选中的叶子，以及选中组下的全部可见子孙
- [x] WASM `export_svg(selected_only)`，Demo「导出 SVG」/「选区 SVG」下载 `.svg`
- [x] 有资源字节的图片写成 `<image>` data URL（#32）；没有字节的纹理跳过并写注释。文本写成 `<text>` / `<tspan>`
- [x] 组不画几何；叶子保持世界变换。连成一段的组写成不带 transform 的 `<g id="eN">`。隐藏组通过 `is_effectively_visible` 藏住子孙，子孙不计入 viewBox

**依赖**: #6 的元素遍历。#4 已合并，文本一并导出。#13 的可见性复用 `Scene::is_effectively_visible`。选区导出会展开选中组，这一点和 PNG 只看元素自身 `selected` 不同。

---

## CRDT 骨架 (#15)

- [x] `infinitecanvas-collab`：yrs 文档，元素进根 `Y.Map` `elements`（键是 id，值是场景 JSON 的单个元素）
- [x] 本地 `insert` / `update` / `delete` 写成 Yjs 事务，远端 v1 更新物化回 `Scene`
- [x] `MemoryProvider` 做双连接 y-sync；`encode_state_v1` 可回放
- [x] 冲突策略见 `docs/design/crdt.md`（整元素 LWW；选区 / 光标 / 房间服务不在这次）
- [x] Text 经 `MemoryProvider` 同步（内容、字号、颜色），对端 `texture_id` 为空
- [x] `peer_slot` 限制在 21 位，元素 id 不超过 JS 安全整数 `2^53-1`
- [x] Group 经 `MemoryProvider` 同步：`children` / `parent` / `locked`，场景树一致；组的世界变换写到子孙；`ungroup` 后组消失、子元素世界坐标保留

```text
Scene ── 事务 ──▶ Y.Doc elements ── SyncStep1/2 ──▶ 另一份 Y.Doc ──▶ Scene
```

未接到 WASM、`www` 和 `packages/server`。Text 随场景 JSON v1 同步（内容、字号、颜色）；`texture_id` 不进文档，对端物化后为空。图片的 `asset_id` 随元素 JSON 同步；`assets[]` 像素还不在 Y.Doc 里，对端没有字节。`CollabSession::group` / `ungroup` / `set_locked` / `set_world_transform` 把组的 `children`、元素的 `parent` / `locked` 和世界变换写进同一条整段 JSON。组 id 用 `allocate_id`。`peer_slot` 限制在 21 位，元素 id 不超过 JS 安全整数 `2^53-1`。未知种类仍会使物化失败。整场景重物化是 O(n)。

---

## 图层面板 (#27)

- [x] `Scene::move_in_outline`：兄弟换序或移入/移出组，重写 `z_index` 并推进 `structure_epoch`
- [x] `ReorderOutlineCommand` 撤销/重做父指针、子列表和绘制顺序
- [x] 锁定沿祖先生效：命中、点选、框选、全选和拖动都会跳过
- [x] WASM：`reorder_element`、`get_structure_epoch`；`set_element_locked` 取消选中子树
- [x] Demo 图层面板：树列表、选中双向同步、拖拽、显隐/锁定、虚拟滚动
- [x] Rust 单测覆盖显隐、锁定、重排和重入组

## M3: 场景管理 (9/9) - ✅ 已完成

- [x] M3.1: 场景图树结构（#13：Group + 场景树；世界变换由组增量传播）
- [x] M3.2: 空间索引 - QuadTree（基础实现，支持点查询和范围查询）
- [x] M3.3: 视口裁剪（跳过渲染视口外元素）
- [x] M3.4: 批量渲染（顶点合并到单个 draw call，60 FPS @ 1100 元素）
- [x] M3.5: 脏标记（只重生成变化元素；平移/缩放只更新 uniform）
- [x] M3.6: 顶点缓存（持久 GPU buffer + 增量 write_buffer，含直线/路径）
- [x] M3.7: 性能监控（get_performance_stats, frame_count, vertex_count）
- [x] M3.8: 大量元素压力测试（test-stress.html 可切换 1k/10k/100k，输出帧时间）
- [x] M3.9: 虚拟化渲染（✓ 视口裁剪已提供类似功能）

### M3 详细子任务规划

**M3.1 场景图树结构**（#13）
- [x] M3.1.1: `SceneTree` / `Group`
- [x] M3.1.2: 父子关系（成组、解散、进组、出组）
- [x] M3.1.3: 组变换传播到子元素世界坐标（平移、旋转、均匀缩放）
- [x] M3.1.4: 分组 / 解组 API，以及 WASM `group_elements` / `get_scene_tree`

**M3.5 脏标记**
- [x] M3.5.1: 元素 dirty 标记，场景维护待更新 id
- [x] M3.5.2: 只重生成变化元素（矩形/圆/图片/直线/箭头/路径，以及组移动后的子孙；手柄旋转/缩放、多选与撤销）
- [x] M3.5.4: `dirty` 不进 JSON；`import_json` 后整表重建顶点
- [x] M3.5.3: 平移/缩放不重建顶点，只更新投影 uniform

**M3.6 顶点缓存**
- [x] M3.6.1: 持久 CPU/GPU 顶点缓冲，增量 write_buffer
- [x] M3.6.2: 图片纹理顶点同样走缓存

**依赖**: M1 完成

---

## M4: 多人协同 (5/10) - 🔄 进行中

- [x] M4.1: 服务端基础（Node.js + ws，房间管理，用户会话）
- [x] M4.2: 客户端 WebSocket（连接管理器，自动重连，状态UI）
- [ ] M4.3: CRDT 集成 - Yjs（#15 骨架在 `crates/collab`；y-websocket / 房间服务仍未接）
- [x] M4.4: 操作同步（创建元素广播，远程操作应用）
- [x] M4.5: 光标同步（位置广播，远程光标渲染，用户标签）
- [x] M4.6: 选择同步（状态广播，远程选择显示）
- [ ] M4.7: 权限控制
- [ ] M4.8: 冲突提示
- [ ] M4.9: 历史回放 (可选)
- [ ] M4.10: 多人测试

### M4 详细子任务规划

**M4.1 服务端基础**
- M4.1.1: 选择技术栈（Node.js + ws / Rust + tokio-tungstenite）
- M4.1.2: 创建基础 WebSocket 服务器
- M4.1.3: 实现房间（Room）管理
- M4.1.4: 实现用户会话管理

**M4.2 客户端 WebSocket**
- M4.2.1: 创建 WebSocket 连接管理器
- M4.2.2: 实现自动重连逻辑
- M4.2.3: 添加连接状态 UI 指示
- M4.2.4: 实现消息队列和缓冲

**M4.3 CRDT 集成 - Yjs**
- [ ] M4.3.1: 集成 y-websocket 或 y-webrtc
- [x] M4.3.2: 设计 Y.Doc 结构映射 Scene（`elements` 地图 + 场景 JSON 字符串）
- [x] M4.3.3: 实现元素增删的 CRDT 同步（`MemoryProvider`，本地双连接）
- [x] M4.3.4: 实现元素属性的 CRDT 同步（整份元素 JSON 替换，不是字段级合并）

**M4.4 操作同步**
- M4.4.1: 定义操作消息格式（JSON Schema）
- M4.4.2: 实现本地操作广播
- M4.4.3: 实现远程操作应用
- M4.4.4: 处理操作冲突合并

**M4.5 光标同步**
- M4.5.1: 设计光标位置消息
- M4.5.2: 实现光标位置广播（节流处理）
- M4.5.3: 渲染远程用户光标
- M4.5.4: 显示用户名称标签

**M4.6 选择同步**
- M4.6.1: 广播当前选择状态
- M4.6.2: 显示其他用户选中的元素
- M4.6.3: 实现选择锁定（防止同时编辑）

**M4.7 权限控制**
- M4.7.1: 定义权限级别（只读/编辑/管理员）
- M4.7.2: 实现服务端权限验证
- M4.7.3: 客户端 UI 权限限制

**M4.8 冲突提示**
- M4.8.1: 检测编辑冲突
- M4.8.2: 显示冲突提示 UI
- M4.8.3: 提供冲突解决选项

**M4.9 历史回放（可选）**
- M4.9.1: 服务端操作日志持久化
- M4.9.2: 实现时间线回放 UI
- M4.9.3: 支持状态快照恢复

**M4.10 多人测试**
- M4.10.1: 创建多人协同测试页面
- M4.10.2: 测试 2-5 人同时编辑
- M4.10.3: 测试网络延迟和断线重连
- M4.10.4: 性能和稳定性测试

**依赖**: M1, M2 完成

---

## 总体进度

- 已完成任务: 34 / 40（主任务）
- 总体进度: 85%
- M1 核心渲染: ✅ 100% 完成
- M2 交互系统: ✅ 100% 完成
- M3 场景管理: 🔄 89% 完成（8/9）
- M4 多人协同: 🔄 50% 进行中（5/10）

---

## 测试验收指南

### 测试页面

| 页面 | URL | 用途 |
|------|-----|------|
| 框选测试 | http://localhost:5179/test-selection.html | 交互功能测试 |
| 压力测试 | http://localhost:5179/test-stress.html | 性能测试 |
| 纹理测试 | http://localhost:5179/test-texture.html | 图片渲染测试 |
| 协同测试 | http://localhost:5179/test-collaboration.html | 多人协同测试 |
| 协同画布 | http://localhost:5179/test-collab-canvas.html | 完整协同画布 |

### 运行测试

```bash
# 1. 编译 WASM
cd infinitecanvas_engine
./build-wasm.sh

# 2. 启动前端开发服务器
cd www
npm run dev  # 端口 5179

# 3. 启动协同服务器（可选，测试多人协同时需要）
cd packages/server
npm run dev  # 端口 8080

# 4. 打开浏览器访问测试页面
open http://localhost:5179/test-selection.html
open http://localhost:5179/test-collaboration.html  # 多人协同
```

### 验收标准

**M1 核心渲染（已完成 ✅）**
- [x] 矩形和圆形正确渲染
- [x] 图片纹理正确渲染
- [x] 60 FPS 稳定运行
- [x] 视口平移和缩放正常

**M2 交互系统（已完成 ✅）**
- [x] 点击选中元素
- [x] 框选多个元素
- [x] 拖动移动元素
- [x] 撤销/重做功能
- [x] 工具切换（V/R/O/H）
- [x] 空格临时切换手型工具

**M3 场景管理（进行中 🔄）**
- [x] 1000+ 元素 60 FPS
- [x] 视口外元素不渲染
- [x] 性能统计显示
- [ ] 场景图树结构（待定）

**M4 多人协同（待开始 📋）**
- [ ] 多人同时编辑
- [ ] 光标实时同步
- [ ] 操作冲突处理

---

## 修复记录

### 2025-12-21: 基本元素绘制修复

**问题**: 矩形和圆形在画布上不显示，但顶点统计正常。

**原因**: `renderer.rs` 的 `render()` 方法中缺少了 fill_pipeline 和 stroke_pipeline 的 draw 调用，只渲染了纹理元素和选择覆盖层。

**修复**: 在 render_pass 中添加了缺失的 draw 调用。

**验证**: 
- test-selection.html: 5 个元素正常渲染
- test-stress.html: 1100 个元素 @ 60 FPS


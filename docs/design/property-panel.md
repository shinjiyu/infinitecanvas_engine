# 属性面板

选中元素后可以改样式，并撤销。

```text
www 右侧面板
    │ get_selection_style / preview_selection_style / commit_style_preview
    ▼
WASM ── 选择 id（组展开为子孙）──► selection_style
    │
    ▼
SetStyleCommand ── apply / undo ──► Scene::mark_dirty
```

| 元素 | 字段 |
| --- | --- |
| 矩形 / 圆 / 多边形 / 星形 / 直线 / 路径 / 画板 | `ElementStyle` 的填充、描边、线宽、虚线、线帽、线结、不透明度、投影、混合模式、图层模糊 |
| 矩形 | 另有 `corner_radius`（四角相同，默认 0，不超过短边一半）。圆、直线、路径没有这一项 |
| 正多边形 / 星形 | 另有 `sides`（边数或角数，3–64，默认六边形 6、五角星 5）。星形另有 `inner_radius`（内径比 0.05–0.95，默认 0.5）。圆和矩形没有这两项 |
| 文本 | 字号、文字颜色、不透明度、描边色、描边宽度、投影、混合模式、图层模糊。暂不支持虚线、线帽、线结 |
| 图片 | 不透明度（`Image::alpha`）、投影、混合模式、图层模糊，以及裁剪窗口（`Image::crop`，0–1）。填充、描边、线宽不写到图片上。裁剪不进 `SetStyle` |
| 组 | 没有自己的样式，读写都落到子孙 |

多选时字段不一致记为 mixed。一次修改写到所有未锁定的目标。滑块在按下时记下基线，松手收成一条历史；撤销和重做会先提交未结束的预览。


数值变换（X / Y / W / H / 旋转）走另一条 `TransformElementsCommand`，不进 `SetStyle`。`get_selection_transform` / `set_selection_transform` 读写当前选择：

- **X / Y**：元素本地原点的世界坐标（`transform.translation`）。矩形 / 圆 / 多边形 / 星形 / 图片 / 画板的局部包围盒以原点为中心，因此与选中框中心（旋转手柄枢轴）一致。直线是起点；文本原点随对齐变化。
- **W / H**：世界尺寸 `|local_bounds| × |scale|`，与选中框半宽半高的两倍相同。写入只改 `scale`（保留符号），与缩放手柄同一语义。直线没有可编辑宽高，`sizeSupported` 为 false 时输入框禁用。
- **旋转**：度，与 `transform.rotation` / 旋转手柄共用同一字段。
- 多选不一致为 mixed（输入框留空）。批量写入时每个选中根设成同一绝对值（例如都把 X 写成 100）。未做长宽比锁定。
- Enter / blur 提交；空选、锁定、非有限数或非正宽高拒绝且不推历史。变换已随场景 JSON 往返，无新字段。不新增快捷键。


矩形和圆的线宽画成填充缓冲里的三角形带，因为线段管线宽度是 1 像素。实线、平头、斜接仍走原来的三角形带。虚线（空数组为实线，`[8, 4]` 虚线，`[2, 4]` 点线）、圆头/方头和圆角/斜角会重铺粗描边，细描边线段也按同一虚线切开。非均匀缩放的圆就是椭圆，线型在局部空间生成后再变换。不透明度乘进顶点 alpha；文本不透明度乘在纹理 tint 上，不重新光栅化。字号、文字颜色和文本描边会丢掉纹理，下一帧按当前缩放重画；文本光栅暂不读虚线、线帽和线结。图片的不透明度写到 `alpha`，纹理四边形跟着重生成。改透明度不增减资源引用。只选中图片时面板留下不透明度、图层模糊、投影、混合模式和裁剪。未选中可编辑元素时整块面板隐藏；线型控件只在选中形状时出现。

裁剪是另一条 `SetImageCrop`，不进 `SetStyle`，也不改 `blur_radius`。面板给出左、上、宽、高（0–1）。拖动数字框时预览，松手收成一条历史；`commit_style_preview` 会先提交未结束的裁剪预览。多选某一边不一致为 mixed，组展开到里面的图片，锁定的跳过。空选、锁定或没有变化不入栈。「裁剪手柄」打开后，单张未锁定的可见图片用现有缩放手柄改窗口，不改世界变换和显示尺寸；旋转手柄在这种模式下不出现。不新增快捷键。Esc 先退出裁剪模式，再按原来的规则取消选择。重置把窗口写回整张图。手柄和数字框读同一份 `crop`。

圆角写在矩形上，经 `SetStyle` 的 `corner_radius` 补丁和填充、描边一起撤销。半径 0 仍是原来的两个三角形和四条边。大于 0 时 WebGPU 用同一半径的圆弧轮廓铺填充，描边（含虚线）沿这条轮廓走。命中测试保持轴对齐外接矩形（切掉的角仍能点中）；包围盒不随圆角缩小。面板在 `cornerRadiusSupported` 为 false（未选中，或选择里没有矩形）时隐藏圆角滑条和数字框。多选半径不一致时为 mixed。

形状还可以把填充改成线性或径向渐变，仍然是一条 `SetStyle`。字段都带 `serde(default)`：`fill_kind`（`solid` / `linear` / `radial`，缺省纯色）、`fill_stops`（offset 与颜色）、`fill_angle`（度，0 为从左到右，90 为从上到下）、`fill_center`（局部坐标）和 `fill_radius`（0 表示用包围盒对角线的一半）。色标为空时退回纯色 `fill`。多选不一致为 mixed。面板在选中形状时给出类型、角度或圆心/半径，以及色标列表。关掉填充会把种类收回纯色。不另设快捷键。正多边形走凸轮廓切条；星形是凹的，线性按每张中心三角形切条，径向保持扇形并在顶点取色。

WebGPU 把渐变烤进填充顶点：线性按色标把凸轮廓切成条带，条带内插值是准确的；径向从圆心（在形外时改用轮廓重心）发出射线，沿射线取色，射线之间是近似。描边仍是纯色。不透明度乘进顶点 alpha。隐藏元素和被隐藏的组不产生填充三角形，绘制顺序不变。

PNG 按像素采样同一套渐变，描边像素仍是纯色。SVG 写成 `linearGradient` / `radialGradient`（均为 `userSpaceOnUse`，与画布采样同轴）；没有渐变的形状仍是原来的 `fill` 颜色。投影另写 `feDropShadow`；图层模糊另写 `feGaussianBlur`（近似，见 scene-persistence）。

吸管不走面板上的滑块预览。它采样一个叶子的填充和描边，再经同一条 `SetStyle` 写到当前选择。渐变上取的是该点的颜色，写回去时收成纯色。圆角、边数、内径、线宽、不透明度、投影、混合模式和图层模糊都不动。落在光晕上、未模糊包围盒外的点仍然是空白。

边数和星形内径走同一条 `SetStyle`。`sides` 只写到多边形和星形，`inner_radius` 只写到星形。面板在 `sidesSupported` 为 false 时隐藏边数控件；`innerRadiusSupported` 为 false 时隐藏内径。只选中星形时标签写成「角数」。多选不一致为 mixed。拖动松手仍是一条撤销。

投影写在形状的 `ElementStyle::shadow`、文本和图片各自的 `shadow` 上。`StylePatch` 里是可选的 `shadow_offset_x` / `shadow_offset_y` / `shadow_blur` / `shadow_color`（只改 RGB）/ `shadow_opacity`（只改 alpha）。未写出的字段保持原值。渐变和混合模式已经是同一补丁上的可选字段。勾选投影时，若当前是全零默认值，一次 `SetStyle` 写成偏移 (8, 8)、模糊 12、黑色、浓度 0.4；取消只把浓度写成 0。滑块和数字框走预览，松手一条历史。多选不一致时为 mixed。`shadowSupported` 在选择里有形状、文本或图片时为 true。偏移在局部空间，旋转和缩放会带着投影走。模糊是轮廓外扩的羽化环，不是离屏高斯。投影不改变命中和包围盒。隐藏元素不输出投影顶点。多边形和星形的剪影就是局部顶点环。

内阴影写在 `ElementStyle::inner_shadow`、文本和图片各自的 `inner_shadow` 上（`serde(default)`，缺省关闭不写入 JSON）。`StylePatch` 里是可选的 `inner_shadow_offset_x` / `inner_shadow_offset_y` / `inner_shadow_blur` / `inner_shadow_spread` / `inner_shadow_color` / `inner_shadow_opacity`。未写出的字段保持原值。绘制是沿内法线的羽化环，放在填充之后、描边之前；和外投影同时打开时，外投影在本体后、内阴影叠在填充上。命中和包围盒不含内阴影。面板紧挨投影，无新快捷键。PNG 按同一近似采样；SVG 用 `feOffset`+`feGaussianBlur`+`feComposite` 近似，不保证像素级一致。

混合模式写在形状（含画板）的 `ElementStyle.blend_mode`、文本和图片自己的 `blend_mode` 上。取值是 `normal`、`multiply`、`screen`、`overlay`、`darken`、`lighten`，缺省 `normal`。面板在 `blendModeSupported` 为 false 时隐藏下拉；多选不一致为 mixed。改一次是一条 `SetStyle`。空选、锁定、或改成当前值，都不入栈。没有新的快捷键。渐变烤在填充顶点里，合成时跟这个叶子的填充一起进同一层。改混合模式不清图层模糊。

图层模糊写在形状（含画板）的 `ElementStyle::blur_radius`，以及文本和图片各自的 `blur_radius`。`StylePatch.blur_radius` 未写出时保持原值，也不改混合模式。0 是关闭，不写入 JSON。面板标签是「图层模糊」，和投影里的「模糊」分开，没有单独的快捷键。滑块和数字框走预览，松手一条 `SetStyle`。空选择、锁定、以及改成当前值都不入栈。多选不一致为 mixed。`blurSupported` 在选择里有形状、文本或图片时为 true，画板算形状。选中画板只改画板，不写到子元素。命中仍是未模糊包围盒。形状用有符号距离上的高斯 CDF；虚线在模糊打开时收成实心软带。图片和文本用 5×5 纹理核，不重做字形光栅。PNG 按同一近似采样；SVG 用 `feGaussianBlur`，不保证和顶点环一致。渐变和模糊同时打开时，软边用包围盒中心采到的一个颜色，不卷积整段渐变，也不清掉色标。模糊为 0 时渐变顶点恢复。布尔填充路径带孔时，实体是内缩后的耳切三角，孔不会被扇形填实。

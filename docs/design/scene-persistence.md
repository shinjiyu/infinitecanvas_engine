# 场景持久化

场景 JSON、PNG 与 SVG 都从 `infinitecanvas-engine` 产出，WASM 只做转发，Demo 负责保存到本地。

```text
www/main.js
    │ export_json / import_json / export_png / export_svg / import_svg
    ▼
infinitecanvas-wasm::InfiniteCanvas
    │ Scene::to_json / from_json
    │ export_viewport_png / export_selection_png
    │ export_svg
    │ import_svg ── 一条 ImportSvgCommand
    ▼
Scene ── elements: rectangle | circle | polygon | image | line | path | text | group
         assets:   图片像素，data URL，同一张图一份
         PNG 与 SVG 共用 elements_in_paint_order（z_index，再按 id）
```

## JSON（version 1）

| 字段 | 含义 |
| --- | --- |
| `version` | 必须是 `1`，缺失或其它值都拒绝 |
| `next_id` | 可省略。小于等于已有最大 ID 时抬到 `max_id + 1` |
| `assets[]` | 可省略。图片资源 `{ id, mime, width, height, data }`。`data` 是 data URL，同一张图只写一次 |
| `elements[]` | `{ "type", "data" }`。`type` 为 `rectangle`、`circle`、`polygon`、`image`、`line`、`path`、`text`、`group`。`star` 是 `polygon` 的导入别名 |
| `guides[]` | 可省略。手动参考线 `{ id, axis, position }`。`axis` 为 `horizontal`（世界 y）或 `vertical`（世界 x）。没有参考线时不写这个字段 |

`data.props.transform` 含 `translation`、`rotation`（弧度，即 `ElementProps::rotation`）、`scale`。直线的 `arrow` 缺省为 `false`。路径的 `points` 缺省为空。文本的 `content`、`font_size`、`color`、`align`、`font_family`、`size` 可省略；对齐缺省为 `left`，字号缺省为 32，尺寸缺省时按内容估算。`texture_id`、LOD、`revision`、`editing` 用 `#[serde(skip)]`，不进文件。PNG 采样先把世界点逆变换回局部空间，所以旋转和非均匀缩放与命中测试一致。

省略的可选字段使用 serde 默认值（旋转 0、缩放 1、可见、不透明度 1、颜色 alpha 1）。形状的 `style.fill` / `stroke` / `stroke_width` / `opacity` 可省略，缺省与 `ElementStyle::new` 相同。`style.stroke_dash` 缺省为空数组（实线），`style.stroke_cap` 缺省 `butt`，`style.stroke_join` 缺省 `miter`；三者与缺省值相同时不写入文件。矩形的 `corner_radius` 可省略，缺省为 0；为 0 时不写出该字段。多边形的 `kind` 缺省为 `polygon`，`sides`（别名 `points`）缺省为 6 并钳到 3–64，`inner_radius` 缺省为 0.5 并钳到 0.05–0.95。`type: "star"` 读成星形；若同时省略边数，角数用 5。旧文档没有 `polygon` 时不受影响。文本的 `opacity` 缺省 1，`stroke` 缺省无，`stroke_width` 缺省 0。文本描边没有虚线、线帽和线结。`props.parent` 和 `props.locked` 也可省略，缺省为根节点和未锁定，文本与其它元素相同。文档或元素上的未知字段会被忽略，因此以后增加可选字段不必升版本。未知元素种类会报错，避免静默丢掉数据。

## PNG

视口导出使用视口的世界包围盒和当前缩放。选区导出使用选中元素包围盒并外扩 1 个世界单位。手动参考线默认不画。`export_viewport_png_with_guides` 把 `include_guides` 设为 true 时，参考线盖一层青色像素。标尺不进 PNG。直线、箭头和路径按半线宽上色，不含点选余量。文本用 `font8x8` 点阵画该字体覆盖的字符；没有点阵的字符（包括中文）画成方框，因为屏幕上的 Canvas2D 纹理不在场景文件里。图片若在资源表里有像素，PNG 按显示矩形采样这些像素；只有 GPU 纹理 id、没有字节的图片仍用纯色占位。

## SVG

`export_svg(selected_only)` 走和 PNG 相同的 `elements_in_paint_order`，默认不画参考线。`export_svg_with_guides(..., true)` 在现有 viewBox 里追加 `#3db7ff` 的 `<line>`，不改变 viewBox。标尺不进 SVG。自己或祖先不可见（`Scene::is_effectively_visible`）的元素跳过，也不计入 viewBox。`selected_only` 为 false 时 viewBox 是全部导出叶子的世界包围盒（再外扩 2 个世界单位）；为 true 时包含自身 `selected` 的叶子，以及选中组下的全部可见子孙叶子。没有叶子时 viewBox 是 `0 0 1 1`。

| 元素 | SVG |
| --- | --- |
| rectangle | `<rect>`，局部原点在中心。`corner_radius` 大于 0 时写 `rx` / `ry` |
| circle | `<circle>` |
| line | `<line>`，箭头是缩短箭杆后的 `<polygon>`，几何与 `Line::generate_vertices` 相同 |
| path | `<polyline>`，`stroke-miterlimit="4"` |
| image | 资源表里有编码字节时写成 `<image href="data:...">`，局部原点在左上角，宽高是显示尺寸，`preserveAspectRatio="none"`。`alpha` 写在外层 `opacity`。UV 裁剪用嵌套 `<svg viewBox>` 取像素区域。没有字节的旧纹理跳过，文件中留 `<!-- image N skipped: ... -->`，且不计入 viewBox |
| text | `<text>` + `<tspan>`。锚点与 `TextAlign` 一致，`font-size` 是世界单位字号 |
| group | 不画几何。子孙已是世界变换，矩阵不再乘组变换。绘制顺序里连成一段时写成不带 `transform` 的 `<g id="eN">`。隐藏组藏住子孙 |

变换写成 `matrix(a b c d e f)`，与 `Transform::to_matrix` 一致（先缩放，再旋转，后平移）。填充、描边、线宽和 `opacity` 写在图元上；分组的 `opacity` 是 `ElementStyle::opacity`，颜色自身的 alpha 用 `fill-opacity` / `stroke-opacity`。直线和路径的缺省线帽线结仍是 `butt` / `miter`。非默认线帽、线结，以及非空虚线，额外写成 `stroke-linecap`、`stroke-linejoin`、`stroke-dasharray`。图片的 `opacity` 来自 `Image::alpha`。Y 轴向下。文本描边不写虚线、线帽和线结。

## 导入 SVG

`import_svg(svg, anchor)` 把常见子集写进当前场景，整批是一条 `ImportSvgCommand`。`anchor` 是 viewBox 原点的世界坐标；没有 viewBox 时是用户坐标原点。Demo 的「导入 SVG」和拖放 `.svg` 用指针位置；指针还没进入画布时用世界坐标 `(24, 24)`。

| SVG | 场景 |
| --- | --- |
| `rect` | 矩形。`rx` / `ry` 不同时取较小的圆角 |
| `circle` | 圆 |
| `ellipse` | 圆，半径放进非均匀缩放 |
| `line` | 直线，不带箭头 |
| `polyline` / `polygon` / `path` | 自由路径。曲线会折线化。填充改成沿轮廓描边。这不是 N / K 创建的正多边形或星形 |
| `text` | 文本。`tspan` 按行 |
| `image` | 只收 PNG/JPEG data URL |
| `g` / 嵌套 `svg` | 变换压进子元素，不建组 |

空文本、非法 XML、根节点不是 `svg`、没有可识别图元都返回错误，场景不变。滤镜、动画、CSS 类、`<use>`、渐变、裁剪、`preserveAspectRatio`，以及百分比 / `em` 长度都不映射。参考线不参与导入。

## 剪贴板

复制和粘贴复用上面的 version 1 文档，不另起格式。选中的图片会带上它们引用的 `assets[]`（同一张图一份 data URL）。

```text
选中根 ── Scene::copy_elements_json ──▶ { version, next_id, assets[], elements[] }
                                              │
                                              ▼
                                    Scene::paste_json（新 ID，缺的资源并进当前表）
```

`copy_elements_json` 只写入选中根和子孙。指向文档外的 `parent` 变成 `null`，组的 `children` 只留文档内的 ID。`paste_json` 为每个元素分配新 ID，并按原来的 z 顺序把整块放到当前最大 z 之上。目标场景里已经有同 id 的资源时不复制第二份像素，只增加引用。`focus` 有值时，叶子包围盒中心先对准该世界坐标，再加上 `offset`。

`Scene::duplicate` 在同一场景里克隆。父组不在副本里时，新根仍挂在那个组下。图片克隆增加同一资源的引用。Ctrl+C / Ctrl+V / Ctrl+D 分别走 `PasteElementsCommand` 和 `DuplicateElementsCommand`，撤销删掉新元素，重做按原 ID 插回。引用降到 0 时像素留在命令里，重做再放回。新元素调用 `Scene::mark_dirty`。

删除组的撤销仍只恢复根节点自己的图片像素（`parked_asset_for` 在组上不保留子图片）。复制、粘贴和克隆会带上子树里的图片。

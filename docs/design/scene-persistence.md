# 场景持久化

场景 JSON、PNG 与 SVG 都从 `infinitecanvas-engine` 产出，WASM 只做转发，Demo 负责保存到本地。

```text
www/main.js
    │ export_json / import_json / export_png / export_svg
    ▼
infinitecanvas-wasm::InfiniteCanvas
    │ Scene::to_json / from_json
    │ export_viewport_png / export_selection_png
    │ export_svg
    ▼
Scene ── elements: rectangle | circle | image | line | path | text | group
         PNG 与 SVG 共用 elements_in_paint_order（z_index，再按 id）
```

## JSON（version 1）

| 字段 | 含义 |
| --- | --- |
| `version` | 必须是 `1`，缺失或其它值都拒绝 |
| `next_id` | 可省略。小于等于已有最大 ID 时抬到 `max_id + 1` |
| `elements[]` | `{ "type", "data" }`。`type` 为 `rectangle`、`circle`、`image`、`line`、`path`、`text`、`group` |

`data.props.transform` 含 `translation`、`rotation`（弧度，即 `ElementProps::rotation`）、`scale`。直线的 `arrow` 缺省为 `false`。路径的 `points` 缺省为空。文本的 `content`、`font_size`、`color`、`align`、`font_family`、`size` 可省略；对齐缺省为 `left`，字号缺省为 32，尺寸缺省时按内容估算。`texture_id`、LOD、`revision`、`editing` 用 `#[serde(skip)]`，不进文件。PNG 采样先把世界点逆变换回局部空间，所以旋转和非均匀缩放与命中测试一致。

省略的可选字段使用 serde 默认值（旋转 0、缩放 1、可见、不透明度 1、颜色 alpha 1）。`props.parent` 和 `props.locked` 也可省略，缺省为根节点和未锁定，文本与其它元素相同。文档或元素上的未知字段会被忽略，因此以后增加可选字段不必升版本。未知元素种类会报错，避免静默丢掉数据。

## PNG

视口导出使用视口的世界包围盒和当前缩放。选区导出使用选中元素包围盒并外扩 1 个世界单位。直线、箭头和路径按半线宽上色，不含点选余量。文本用 `font8x8` 点阵画该字体覆盖的字符；没有点阵的字符（包括中文）画成方框，因为屏幕上的 Canvas2D 纹理不在场景文件里。图片纹理同样不在文件里，PNG 中为纯色占位。

## SVG

`export_svg(selected_only)` 走和 PNG 相同的 `elements_in_paint_order`。自己或祖先不可见（`Scene::is_effectively_visible`）的元素跳过。`selected_only` 为 false 时 viewBox 是全部导出图元的世界包围盒（再外扩 2 个世界单位）；为 true 时只含自身 `selected` 的元素，不把选中组展开成未选中的子元素。

| 元素 | SVG |
| --- | --- |
| rectangle | `<rect>`，局部原点在中心 |
| circle | `<circle>` |
| line | `<line>`，箭头是缩短箭杆后的 `<polygon>`，几何与 `Line::generate_vertices` 相同 |
| path | `<polyline>`，`stroke-miterlimit="4"` |
| image | 跳过。像素不在场景里，文件中留 `<!-- image N skipped: ... -->` |
| text | `<text>` + `<tspan>`。锚点与 `TextAlign` 一致，`font-size` 是世界单位字号 |
| group | 不输出。子元素已是世界变换，各自写成上面的图元。隐藏组会藏住子孙 |

变换写成 `matrix(a b c d e f)`，与 `Transform::to_matrix` 一致（先缩放，再旋转，后平移）。填充、描边、线宽和 `opacity` 写在图元上；分组的 `opacity` 是 `ElementStyle::opacity`，颜色自身的 alpha 用 `fill-opacity` / `stroke-opacity`。Y 轴向下。

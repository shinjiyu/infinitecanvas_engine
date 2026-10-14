# 场景持久化

场景 JSON 与 PNG 都从 `infinitecanvas-engine` 产出，WASM 只做转发，Demo 负责保存到本地。

```text
www/main.js
    │ export_json / import_json / export_png
    ▼
infinitecanvas-wasm::InfiniteCanvas
    │ Scene::to_json / from_json
    │ export_viewport_png / export_selection_png
    ▼
Scene ── elements: rectangle | circle | image | line | path | text | group
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

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
Scene ── elements: rectangle | circle | image | line | path
```

## JSON（version 1）

| 字段 | 含义 |
| --- | --- |
| `version` | 必须是 `1`，缺失或其它值都拒绝 |
| `next_id` | 可省略。小于等于已有最大 ID 时抬到 `max_id + 1` |
| `elements[]` | `{ "type", "data" }`。`type` 为 `rectangle`、`circle`、`image`、`line`、`path` |

`data.props.transform` 含 `translation`、`rotation`（弧度，即 `ElementProps::rotation`）、`scale`。直线的 `arrow` 缺省为 `false`。路径的 `points` 缺省为空。PNG 采样先把世界点逆变换回局部空间，所以旋转和非均匀缩放与命中测试一致。

省略的可选字段使用 serde 默认值（旋转 0、缩放 1、可见、不透明度 1、颜色 alpha 1）。文档或元素上的未知字段会被忽略，因此以后增加可选字段不必升版本。未知元素种类（例如尚未合并的 `text`）会报错，避免静默丢掉数据。

## PNG

视口导出使用视口的世界包围盒和当前缩放。选区导出使用选中元素包围盒并外扩 1 个世界单位。直线、箭头和路径按半线宽上色，不含点选余量。图片纹理不在场景文件里，PNG 中为纯色占位。

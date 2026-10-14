# 文本元素

文本走现有纹理管线。浏览器负责排版和中英文字形，Rust 只保存内容和 LOD。

```text
T 单击 / add_text
        │
        ▼
   Text 元素  ──take_text_raster_jobs(zoom)──► Canvas2D / OffscreenCanvas
        │                                              │
        │                                         RGBA 纹理
        │                                              │
        └──────── commit_text_raster ◄── upload_texture
                         │
                         ▼
                  textured quad
```

## 元素

| 字段 | 含义 |
| --- | --- |
| content | 文本，保留换行 |
| font_size | 世界单位字号 |
| color | 填充色 |
| align | 左 / 中 / 右，同时是锚点 |
| texture | 当前 LOD 的光栅结果 |

锚点：左对齐在左上角，居中在上边中点，右对齐在右上角。

## 缩放

`lod_bucket(zoom)` 把缩放收成 1/4 倍频程一档。换档才重新光栅化。世界尺寸用「纹理像素 / rasterScale」回写，所以换档不改变文字块大小，只提高像素密度。

编辑用 DOM textarea 盖在文字上。编辑期间不画纹理。Enter 提交并进入撤销栈，Esc 取消，空白内容会删除元素。

场景 JSON（version 1）保存 `content`、`font_size`、`color`、`align`、`font_family`、`size` 和 `props.transform` 的旋转/缩放。光栅用的纹理 id、LOD、`revision`、`editing` 以及元素的 `dirty` 都不写入文件。T 工具的双击计时用 `web_time::Instant`，在 wasm32 上走 `Performance.now`。

顶点进持久纹理缓冲。创建、改内容/外观、`commit_text_raster`、进入或离开编辑，以及对应的撤销/重做，都调用 `Scene::mark_dirty`。文本不画纯色底。

SVG 导出不使用光栅纹理，写成 `<text>` / `<tspan>`：锚点与 `TextAlign` 相同，`font-size` 是世界单位字号，行距为字号的 1.25 倍。

PNG 不读取 Canvas2D 纹理。导出用 `font8x8` 把已覆盖的字符画进文字块；中文等没有点阵的字符画成方框。

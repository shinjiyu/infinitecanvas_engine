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
| 矩形 / 圆 / 直线 / 路径 | `ElementStyle` 的填充、描边、线宽、不透明度 |
| 文本 | 字号、文字颜色、不透明度、描边色、描边宽度 |
| 组 | 没有自己的样式，读写都落到子孙 |
| 图片 | 不进这张面板 |

多选时字段不一致记为 mixed。一次修改写到所有未锁定的目标。滑块在按下时记下基线，松手收成一条历史；撤销和重做会先提交未结束的预览。

矩形和圆的线宽画成填充缓冲里的三角形带，因为线段管线宽度是 1 像素。不透明度乘进顶点 alpha；文本不透明度乘在纹理 tint 上，不重新光栅化。字号、文字颜色和文本描边会丢掉纹理，下一帧按当前缩放重画。

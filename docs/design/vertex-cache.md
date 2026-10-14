# 顶点缓存与脏标记

世界空间顶点只在元素变化时重生成。视口平移/缩放只更新投影 uniform。

```text
Scene.mark_dirty(id) ──► VertexCache.sync
        │                      │
        │ 无脏元素              ├─ 增量 GpuVertexWrite ──► queue.write_buffer
        │                      └─ geometry_unchanged ──► 只写 64 字节 uniform
        └─ 增删元素 ──► 按 z-index 重排一次，整段上传
```

| 元素 | 缓冲 | 说明 |
|------|------|------|
| 矩形 / 圆形填充 | fill（三角形） | 6 或扇形顶点。颜色 alpha 乘 `ElementStyle::opacity` |
| 矩形 / 圆形粗描边 | fill（三角形带） | 线宽大于 0 时跟在填充后面，画布能看出线宽 |
| 矩形 / 圆形细描边 | stroke（线段） | 线宽小于等于 0 时不生成 |
| 直线 / 箭头 / 自由画笔 | fill（三角形） | 与旧收集器相同，三角形带 |
| 图片 | textured | 按纹理合并相邻绘制，先画纯色底 |
| 文本 | textured | 不画纯色底，避免挡住透明字形 |
| 组 | 无 | 组本身不生成顶点。平移、旋转、缩放或显隐时，子树里的叶子（含文本）被标脏并重写 |

几何修改（平移、旋转、缩放、端点、箭头、采样点、透明度、填充/描边/线宽、文本内容或光栅结果，以及组变换）必须调用 `Scene::mark_dirty`。只改视口不要调用。`SetStyle` 的执行和撤销会标记受影响的元素。文本改字号、颜色或描边时丢掉光栅，下一帧重画；只改不透明度时保留纹理，用 tint。文本的创建、改内容、撤销/重做和 `commit_text_raster` 都会标记。组的 `translate_element` / `set_world_transform` / `set_visible` 会把整棵子树入队，文本纹理四边形跟着重写。隐藏的组在重写时不再输出子孙三角形。

`Scene::geometry_epoch` 在增删元素和每次 `mark_dirty` 时递增，小地图用它决定要不要重画缩略图。视口平移/缩放不递增。

变换手柄写完整 `Transform`。实时拖动（含多选和取消）在应用变换列表时标记脏；`TransformElementsCommand` 的执行与撤销同样标记，顶点才会重生成。

`dirty`、结构版本和脏队列不写入场景 JSON。`import_json` 让缓存失效，下一帧整表重建并全量上传。

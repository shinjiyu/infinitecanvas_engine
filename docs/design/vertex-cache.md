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
| 矩形 / 圆形填充 | fill（三角形） | 6 或扇形顶点 |
| 矩形 / 圆形细描边 | stroke（线段） | |
| 直线 / 箭头 / 自由画笔 | fill（三角形） | 与旧收集器相同，三角形带 |
| 图片 | textured | 按纹理合并相邻绘制 |

几何修改（平移、旋转、缩放、端点、箭头、采样点、透明度）必须调用 `Scene::mark_dirty`。只改视口不要调用。

变换手柄写完整 `Transform`。实时拖动（含多选和取消）在应用变换列表时标记脏；`TransformElementsCommand` 的执行与撤销同样标记，顶点才会重生成。

`dirty`、结构版本和脏队列不写入场景 JSON。`import_json` 让缓存失效，下一帧整表重建并全量上传。

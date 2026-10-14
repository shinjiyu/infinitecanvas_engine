# 小地图

右下角缩略图显示可见叶子的外框和当前视口。点击或拖拽视口框移动相机。不进入 WebGPU 渲染循环。

```text
可见叶子 world_bounds ──► scene_bounds ──► fit_minimap（等比、留白内居中）
        │                                      │
        │  geometry_epoch 变化                 ├─ minimap_element_rects（缓存）
        │                                      └─ minimap_viewport_rect（只读相机）
        └─ 点击 / 拖拽 ──► minimap_to_world ──► set_camera_center
```

| 变化 | 重算 |
|------|------|
| 增删元素、`mark_dirty` | `geometry_epoch` 加一，下一帧重画外框 |
| 平移、缩放 | 外框缓存不动，只重画视口框 |
| 滚轮在小地图上 | 不调用 `zoom_at` |

10 万个亚像素外框按像素去重后再交给页面。页面用 `minimap_epoch()` 判断要不要取外框，拖动过程中最多约每 150ms 取一次。

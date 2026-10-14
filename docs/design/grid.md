# 背景网格

网格画在形状下面。线在世界坐标的整数倍上，平移和缩放只改投影，线的世界位置不动。

```text
set_grid_visible ──► InfiniteCanvas.grid_visible（默认 true）
                              │
                              ▼
                    visible_grid_spacing(zoom)
                              │
              视口世界范围 ──► grid_vertices ──► LineList（视口没变则不重传）
```

| 项 | 值 |
|----|----|
| 基础间距 | 20 世界单位。1× 时和标尺次刻度重合 |
| 缩小 | 相邻线不足 8px 时间距 ×5，仍是 20 的倍数 |
| 默认 | 可见。Demo 用 `localStorage` 键 `infinitecanvas.gridVisible`，只有 `"false"` 才隐藏 |
| 快捷键 | `Ctrl/Cmd+'`。`G` 仍是标尺 |

隐藏不进撤销栈，下一帧生效。

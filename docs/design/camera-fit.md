# 相机：缩放到选中 / 适应全部

一键把相机对准当前选中，或对准整张场景。只改视口中心和缩放，不改元素几何。

```text
选中 id ──可见且未锁定──► world_bounds（组不展开，用组根）
可见叶子 ── scene_bounds ─┤
                          ▼
             camera_for_bounds（每边留视口的 10%）
                          │
                          ▼
                Viewport center + zoom

空选   ──► 相机不动，返回 false
空场景 ──► 默认相机（原点，1×），返回 false
```

| 命令 | 内容 | 空内容 |
|------|------|--------|
| `zoom_to_selection` | 可见、未锁定的选中目标。组用组根包围盒，隐藏子元素仍算在组根里 | 不改相机 |
| `zoom_to_fit` / `fit_scene` | 与小地图相同的可见叶子并集 | 回到原点、缩放 1 |

边距是视口比例，不是像素：内容装进视口的 80%。更紧的那条轴每边留 10%，另一条轴留白更大。缩放仍限制在 0.1×–10×。某一轴长度为 0 时绕中心扩成 1 个世界单位。

相机不进撤销栈。`Command` 只作用于场景，平移和滚轮缩放同样不入栈。`geometry_epoch` 不变，所以小地图外框缓存不动，只跟着相机重画视口框。`set_camera_center`、`pan`、`zoom_at` 和框选不受影响。

Demo 工具栏有「适应全部」和「缩放到选中」。`Shift+1` 与 `Shift+2` 登记在 `www/shortcuts.js` 的 `SHORTCUTS`，`?` 帮助面板从这张表生成：

```text
Shift+1           zoomFit       ──► zoom_to_fit / fit_scene
Shift+2           zoomSelection ──► zoom_to_selection
Shift+0 / Ctrl+0  zoom100       ──► set_zoom(1)
Ctrl+= / Ctrl+-   zoomIn/Out    ──► zoom_by(zoom_step)
```

键盘步进不另写一套上下限。`ZOOM_STEP`（1.1）与滚轮相同；`zoom_by` 把锚点放在画布中心后调用 `zoom_at`，所以钳制仍是视口的 `min_zoom` / `max_zoom`（0.1×–10×），和 `camera_for_bounds` 一样。滚轮锚点是指针，键盘锚点是视口中心。页脚用百分比显示 `get_zoom()`。

`Shift+H` / `Shift+V` / `Shift+R` / `Shift+Alt+R` 仍是翻转和 90° 旋转。页面里没有第二套 Digit1 / Digit2 处理。

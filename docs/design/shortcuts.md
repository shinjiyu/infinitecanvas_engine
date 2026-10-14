# 快捷键

首页 Demo 的键盘操作只有一张表：`www/shortcuts.js` 里的 `SHORTCUTS`。按键分发、`?` 帮助面板，以及右键菜单上的键帽都读它。菜单动作仍进同一套 `dispatchShortcut`。

```text
keydown / keyup
      │
      ▼
matchShortcut(SHORTCUTS) ──► main.js 分发到 WASM
      │
      ▼
helpEntries(SHORTCUTS) ──► #shortcut-help
```

`tool` 是所属工具（`select`、`rectangle`、`circle`、`line`、`pen`、`text`、`pan`）。不绑定某个工具的操作记为 `canvas`。Mac 上修饰键显示为 ⌘ / ⇧，其他平台显示为 Ctrl / Shift。

焦点在 `input`、`textarea`、`select` 或 `contenteditable` 上时不触发。Esc 在帮助面板打开时先关闭面板，否则取消选择。`M` 显示或隐藏小地图。

复制、粘贴、克隆走同一张表：`Ctrl/Cmd+C`、`Ctrl/Cmd+V`、`Ctrl/Cmd+D`（不带 Shift）。方向键微调也在表里，普通步长 1 个世界单位，Shift 为 10 倍；带 Ctrl、Alt 或 ⌘ 的方向键不触发。粘贴和复制仍由 `www/clipboard.js` 读写系统剪贴板，失败时退回内存。`Ctrl/Cmd+V` 不调用 `preventDefault`，好让 `paste` 事件先看到剪贴板。有 PNG/JPEG 时页面用 `add_image_bytes` 粘贴图片；没有图片时才粘贴元素 JSON。

层级也在这张表里：`]` 前移，`[` 后移，`Ctrl/Cmd+]` 置顶，`Ctrl/Cmd+[` 置底。Shift 或 Alt 不触发。图层面板上的四个按钮调用同一个 `reorder_selection`。

翻转和 90° 旋转（#49）也在这张表里：`Shift+H` 水平翻转，`Shift+V` 垂直翻转，`Shift+R` 顺时针 90°，`Shift+Alt+R` 逆时针 90°。按住不重复。不带 Shift 的 H / V / R 仍是平移、选择、矩形。带 Ctrl 或 ⌘ 的 H / V / R 不翻转、不旋转。工具栏和属性面板的四个按钮调用同一个 `flip_selection` / `rotate_selection_quarter`。

锁定和隐藏（#57）也在这张表里：`Ctrl/Cmd+L` 切换锁定，`Ctrl/Cmd+Shift+H` 切换隐藏。按住不重复。图层面板的眼睛和锁调用 `set_element_visible` / `set_element_locked`，和这两条快捷键共用 `SetVisibilityCommand` / `SetLockedCommand`。空选不入栈。隐藏后不参与命中和框选。锁定后不能平移或变换，图层面板仍可选中，快捷键切换时选择保留。Mac 上显示为 ⌘L、⌘⇧H。

视图缩放也在这张表里：`Shift+1`（`zoomFit`）调用 `zoom_to_fit`，`fit_scene` 是同一个命令；`Shift+2`（`zoomSelection`）调用 `zoom_to_selection`；`Shift+0` 和 `Ctrl/Cmd+0`（`zoom100`）调用 `set_zoom(1)`。`Ctrl/Cmd+=` 或 `Ctrl/Cmd++`（`zoomIn`）以及 `Ctrl/Cmd+-`（`zoomOut`）按 `zoom_step`（`ZOOM_STEP`，1.1）绕视口中心步进，钳制走 `zoom_at`，与滚轮、适应全部同一组 0.1×–10×。滚轮仍把指针交给 `zoom_at`，倍数同样读 `zoom_step`。页脚把 `get_zoom()` 显示成百分比，键盘和滚轮都会刷新。`?` 帮助面板从同一张表列出这些行。工具栏的「适应全部」和「缩放到选中」走同样的两个函数。详见 [camera-fit.md](./camera-fit.md)。

`Ctrl/Cmd` 的放大、缩小和 100% 会挡住浏览器缩放，所以只在焦点位于画布、舞台，或还没有落到具体控件（`body`）时 `preventDefault`。输入框、下拉框和按钮不接管。`Shift+0` 不跟浏览器抢键，仍按原来的范围生效。

多边形和星形（#65）也在这张表里：不带修饰键的 `N` 切换多边形工具，`K` 切换星形工具。`?` 帮助面板列出这两行。工具栏按钮调用同一个 `set_tool`。

画板（#74）也在这张表里：不带修饰键的 `F` 切换画板工具。`Shift+F`、`Ctrl/Cmd+F`、`Alt+F` 不触发。它不占用 V / L / P / R / O / T / H / I / M / G / N / K，也不占用 `Ctrl/Cmd+'` 或 `Ctrl/Cmd+L`。`?` 帮助面板列出这一行。工具栏「画板」按钮调用同一个 `set_tool`。按住 Alt 单击选中画板而不是它上面的子元素。详见 [frame.md](./frame.md)。

标尺和手动参考线（#56）也在这张表里：不带修饰键的 `G` 显示或隐藏标尺和参考线，按住不重复。`Ctrl/Cmd+G` 仍是成组，`Ctrl/Cmd+Shift+G` 仍是取消分组。吸附一组里的按钮调用同一个 `set_rulers_visible`。选中参考线时，键盘 Delete / Backspace 先删参考线，否则仍删除选中元素。右键菜单里的「删除」只删元素。

吸管（#60）是不带修饰键的 `I`。`Shift+I`、`Ctrl/Cmd+I`、`Alt+I` 都不触发，因此不会占用 V / L / P / R / O / T / H / G，也不会和 Shift+H / Shift+V / Shift+R 撞车。工具栏有同样的按钮。按住不重复。

背景网格（#64）也在这张表里：`Ctrl/Cmd+'`（`toggleGrid`）显示或隐藏网格，按住不重复。不占用 `G`，也不占用 `I` 或 `Ctrl/Cmd+=` / `-` / `0`。工具栏「网格」按钮调用同一个 `set_grid_visible`。Mac 上显示为 ⌘'。详见 [grid.md](./grid.md)。

## 引擎补充

删除、全选和缩放在页面里没有现成的一次调用，所以 WASM 增加了这些入口，场景逻辑仍在 engine 里：

| 方法 | 作用 |
| --- | --- |
| `delete_selection` | 选中元素合成一条 `RemoveElementsCommand`。组会连同子孙删除，撤销时放回整棵子树 |
| `select_all` | 选中顶层可见元素。组只选中组本身。锁定元素（含被锁定的组）不选 |
| `zoom_to_selection` | 可见且未锁定的选中目标。组用组根包围盒。每边留视口的 10%。空选不改相机 |
| `zoom_to_fit` / `fit_scene` | 与小地图相同的可见叶子。每边留视口的 10%。没有可见叶子时回到原点、缩放 1 |
| `zoom_by` | 绕视口中心乘上倍数，中心不动。钳制交给 `zoom_at` |
| `zoom_step` | 返回 `ZOOM_STEP`（1.1）。滚轮和键盘共用 |
| `set_zoom` | 设置缩放倍数，视口中心不变。`1` 即 100% |

文本（#4）已经进 master：`T` 切换文本工具；编辑框聚焦时 `Enter` 确认、`Esc` 取消，这两条的 `scope` 是 `text-editor`，不会在画布上触发。分组（#13）已进 master：`Ctrl+G` 调用 `group_elements`，`Ctrl+Shift+G` 调用 `ungroup`。成组后改选新组；取消分组后改选原来的直接子元素。

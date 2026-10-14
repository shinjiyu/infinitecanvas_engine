# 快捷键

首页 Demo 的键盘操作只有一张表：`www/shortcuts.js` 里的 `SHORTCUTS`。按键分发和 `?` 帮助面板都读它。

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

## 引擎补充

删除、全选和缩放在页面里没有现成的一次调用，所以 WASM 增加了四个入口，场景逻辑仍在 engine 里：

| 方法 | 作用 |
| --- | --- |
| `delete_selection` | 选中元素合成一条 `RemoveElementsCommand`。组会连同子孙删除，撤销时放回整棵子树 |
| `select_all` | 选中顶层可见元素。组只选中组本身。锁定元素（含被锁定的组）不选 |
| `zoom_to_fit` | 用 `Scene::content_bounds`（跳过组节点和隐藏元素）和 `Viewport::fit_rect`，四周留 48px |
| `set_zoom` | 设置缩放倍数，视口中心不变。`1` 即 100% |

文本（#4）已经进 master：`T` 切换文本工具；编辑框聚焦时 `Enter` 确认、`Esc` 取消，这两条的 `scope` 是 `text-editor`，不会在画布上触发。分组（#13）已进 master：`Ctrl+G` 调用 `group_elements`，`Ctrl+Shift+G` 调用 `ungroup`。成组后改选新组；取消分组后改选原来的直接子元素。

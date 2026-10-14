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

焦点在 `input`、`textarea` 或 `contenteditable` 上时不触发。Esc 在帮助面板打开时先关闭面板，否则取消选择。`M` 显示或隐藏小地图。

## 引擎补充

删除、全选和缩放在页面里没有现成的一次调用，所以 WASM 增加了四个入口，场景逻辑仍在 engine 里：

| 方法 | 作用 |
| --- | --- |
| `delete_selection` | 选中元素合成一条 `RemoveElementsCommand` |
| `select_all` | 选中全部元素 |
| `zoom_to_fit` | 用 `Scene::content_bounds` 和 `Viewport::fit_rect`，四周留 48px |
| `set_zoom` | 设置缩放倍数，视口中心不变。`1` 即 100% |

文本（#4）已经进 master：`T` 切换文本工具；编辑框聚焦时 `Enter` 确认、`Esc` 取消，这两条的 `scope` 是 `text-editor`，不会在画布上触发。分组（#13）还没进 master，表里没有 Ctrl+G。

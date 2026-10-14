# 右键上下文菜单

画布上右键弹出编辑菜单。空白处和元素上的项目不同。菜单不自己改场景，每一项都调用 `dispatchShortcut` 里已经存在的入口。

```text
contextmenu
    │
    ├─ hit_test_screen_point
    │     ├─ 空白 ──────────────────────────► 粘贴 / 全选 / 缩放到适合
    │     ├─ 命中已选中，或祖先已选中 ────────► 保持选择
    │     └─ 命中未选中 ──► select_element ─► 选中菜单
    │
    └─ 选中菜单
          ├─ 复制 / 粘贴 / 克隆 ── dispatchShortcut copy / paste / duplicate
          ├─ 删除 ── dispatchShortcut delete ── delete_selection
          ├─ 成组 / 解组 ── dispatchShortcut group / ungroup
          └─ 置顶 / 置底 / 前移 / 后移 ── dispatchShortcut reorder
```

| 菜单 | 项 | 入口 |
| --- | --- | --- |
| 选中 | 复制、粘贴（剪贴板有文本）、克隆 | `copy` / `paste` / `duplicate` |
| 选中 | 删除 | `delete` |
| 选中 | 成组（至少两个）、解组（唯一选中是组） | `group` / `ungroup` |
| 选中 | 置顶、置底、前移、后移 | `reorder` |
| 空白 | 粘贴、全选、缩放到适合 | `paste` / `selectAll` / `zoomFit` |

粘贴在选中菜单里只在剪贴板有非空文本时出现。空白菜单始终列出粘贴；剪贴板是空的时候点击没有效果。菜单上的粘贴先把右键落点写回指针位置，再走和 Ctrl+V 相同的粘贴（剪贴板里有图片时仍先贴图片）。

删除、全选、成组、层级和缩放到适合都进 `dispatchShortcut`，撤销行为和对应快捷键相同。缩放到适合只动相机，不进撤销栈。

Esc，或点击菜单以外的地方，关掉菜单。关掉菜单的那一下指针不到引擎，因此不会开始平移或框选。菜单开着时按 Esc 只关菜单，不取消选择，也不关快捷键帮助面板。

菜单项旁边的键帽从 `www/shortcuts.js` 的 `SHORTCUTS` 用 `formatChord` 取出，和 `?` 面板是同一组字符串。表里没有对应条目的项不显示键帽。

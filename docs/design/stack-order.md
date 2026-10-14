# 层级快捷键

选中元素后，不打开图层面板也能改同一层的绘制顺序。下标 `0` 是最底层（先画）。面板把这一层反过来画，最上面的一行是最前面的元素。

```text
] / [ / Ctrl+] / Ctrl+[（或 Cmd）
        │
        ▼
reorder_selection("front" | "back" | "forward" | "backward")
        │
        ▼
ReorderSelectionCommand ── Scene::arrange_siblings
        │
        ├─ 一条 CommandHistory（OutlineSnapshot 撤销）
        └─ structure_epoch ──► 顶点缓存按 z_index 整表重排
```

| 动作 | 按键 | 效果 |
| --- | --- | --- |
| `forward` | `]` | 前移一层 |
| `backward` | `[` | 后移一层 |
| `front` | Ctrl+] 或 Cmd+] | 置顶 |
| `back` | Ctrl+[ 或 Cmd+[ | 置底 |

## 谁会被移动

- 只比较**同一个父节点**下的兄弟。选中项的父节点不一致（包括选中了组、又选中了组里的子元素）时整次操作不入栈。
- 选中组时移动组根。组内 `children` 顺序不变，子元素不会被拆到组外。
- 多选时保持这些兄弟原来的相对顺序。置顶 / 置底把它们整块放到这一层的最前或最后。前移 / 后移只跨过一个未选中的兄弟；挨在一起的选中项会作为一块移动。
- 已经在目标端时不入栈。

## 锁定和隐藏

和图层面板拖拽一致：

- 元素自己被锁定，或任一祖先被锁定：这个 id 跳过。同一层里其余未锁定的选中项照常重排。一个都挪不动时不入栈。
- 隐藏不阻止重排。元素自己不可见，或祖先被隐藏，仍然可以改顺序。图层面板里隐藏行可以拖，眼睛只影响命中和绘制。

锁定的未选中兄弟可以留在原相对位置上。把旁边未锁定的元素移过去时，它的下标可能跟着变，和拖拽越过一行锁定图层相同。

## 绘制

`arrange_siblings` 按深度优先重写 `z_index` 并推进 `structure_epoch`。顶点缓存看到结构版本变化就整表重排，所以画布顺序和图层面板的 `get_scene_tree` 一起变。几何脏队列不增加：三角形没变，小地图的 `geometry_epoch` 也不变。拖拽排序（`move_in_outline` / `ReorderOutlineCommand`）走的是同一条结构版本路径。

世界坐标不改。撤销写回父指针、子列表和 `z_index`。

`SHORTCUTS` 登记了 `]`、`[`、`Ctrl/Cmd+]`、`Ctrl/Cmd+[`，`?` 帮助面板列出这四条。图层面板顶上的四个按钮调用同一个 `reorder_selection`。Shift 或 Alt 不触发。输入框、下拉框和 contenteditable 里这些键留给文本。

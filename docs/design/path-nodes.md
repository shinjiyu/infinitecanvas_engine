# Path 节点编辑

选中单一 Path 后进入节点模式，拖拽锚点（及已有贝塞尔控制柄）修形。一次拖动对应一条 `EditPath` 撤销记录。

```text
双击 Path / 面板「编辑节点」
        │
        ▼
 path_node_mode ──► 覆盖层锚点方块 + 控制柄
        │
        ▼
 拖拽 ── move_path_node ── mark_dirty
        │
        ▼
 松手 ── EditPathCommand（空选/锁定/无变化不入栈）
```

## 几何

- 自由画笔：`points` 是锚点；可选 `point_handles`（相对锚点的 `hin` / `hout`）。缺省或空 = 尖角折线，与旧 JSON 兼容。
- 有控制柄时，`stroke_polyline()` 把相邻锚点间的三次贝塞尔展平；命中、WebGPU、PNG、SVG 都走展平结果。
- 布尔填充 Path：编辑 `shapes` 环上的顶点（暂无控制柄）。

## 与选择态

| 操作 | 行为 |
|------|------|
| Esc | 先退出节点模式，保留选择 |
| 点空白 | 退出节点模式并清空选择（开始框选） |
| 节点模式开 | 隐藏包围盒缩放/旋转手柄，只画节点 |
| 快捷键 | 无新增；避开 V/L/P/R/O/T/H/I/M/G/F/N/K |

## API

- `set_path_node_mode` / `path_node_mode` / `path_node_edit_supported`
- `begin_path_node_edit_at(x, y)`：双击入口
- `get_path_nodes()`：世界坐标节点列表（调试/自定义 UI）

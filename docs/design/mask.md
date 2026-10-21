# 形状遮罩（Mask）

用一个闭合形状的**填充轮廓**裁剪绑定目标的可见区域。关系可撤销，旧 JSON 无字段仍可读。

```text
选中 ≥2 叶子 ──► SetMask（最上层 = mask 源）
解除 ──► ReleaseMask
        │
        ▼
目标 props.masked_by = mask_id
        │
        ├─ 画布：顶点三角形 / 线段中心点在 mask 内才保留
        ├─ PNG：采样前 mask_covers
        └─ SVG：clipPath（userSpaceOnUse）
```

## 数据

| 字段 | 位置 | 缺省 | 作用 |
| --- | --- | --- | --- |
| `masked_by` | `ElementProps` | `None` | 目标指向遮罩源。`serde(default)` + 缺省不写出 |

删除遮罩源或解组删掉组节点后，场景清掉悬挂的 `masked_by`。

## 命令

| 名 | 行为 |
| --- | --- |
| `SetMask` | 最上层可作 mask 的叶子裁剪其余选中目标；空选 / 锁定 / 不足两个 = no-op |
| `ReleaseMask` | 清选中目标上的关系，或清所有指向所选 mask 源的目标 |

成功后对目标与 mask `mark_dirty`。mask 几何变化时，依赖它的目标一并入脏队列。

## 与 opacity / blend

- **遮罩源**照常绘制（含自身 opacity / blend）。
- **目标**先按 mask 填充轮廓硬裁剪，再按目标自己的 opacity / blend 合成。
- 不把 mask 的 alpha 乘到目标上（v1 是轮廓 clip，不是 alpha matte）。

## UI

属性面板 / 工具栏「设为遮罩」「解除遮罩」；右键菜单同入口。**无新快捷键**（避开 V/L/P/R/O/T/H/I/M/G/N/K/F 等）。

## 导出

- PNG：与画布同一套 `mask_covers` 采样，反映裁剪。
- SVG：`clipPath` + 目标外包 `<g clip-path>`。mask 缺失时 clip 为空，目标不可见。

# 多边形与星形

正多边形和星形是同一个元素。拖拽决定外接圆，边数和内径在属性面板里改，并且可以撤销。

```text
工具 N / K
    │ 拖拽外接圆
    ▼
Polygon ── kind + sides + inner_radius
    │
    ├─ 顶点缓存：中心扇形 + 描边
    ├─ 命中 / 包围盒：局部顶点
    └─ SetStyle.sides / inner_radius ── 一条撤销，mark_dirty
```

## 元素

| 字段 | 含义 |
| --- | --- |
| kind | `polygon` 或 `star`。JSON 缺省为正多边形 |
| radius | 外接圆半径。中心在 `props.transform.translation` |
| sides | 边数或角数，3–64。正多边形缺省 6，星形构造缺省 5。JSON 别名 `points` |
| inner_radius | 星形内点半径 / 外接圆，0.05–0.95，缺省 0.5。正多边形保留该值但不改变外形 |
| style | 与矩形、圆相同的填充、描边、线宽、不透明度、虚线、线帽、线结 |

第一个顶点朝向局部负 Y，所以六边形是尖顶，星形有一角朝上。旋转和非均匀缩放走已有的 `transform`。

场景 JSON 的类型名是 `polygon`。导入时 `star` 也接受，并强制 `kind` 为星形；如果这份数据没写边数，角数用 5。没有该类型的旧文档仍然有效。

## 编辑

画布上松开鼠标后，预览元素经 `AddElementCommand` 入栈。`Ctrl+Z` 删掉刚创建的形状。

边数和内径是 `SetStyle` 的扩展字段，和填充、描边同一次预览 / 提交。内径只写到星形。圆、矩形、直线忽略这两个字段。

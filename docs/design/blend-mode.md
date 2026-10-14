# 混合模式

选中的形状、文本和图片可以各自正片叠底、滤色、叠加、变暗或变亮，不必先把重叠区域烘焙成一张位图。

```text
属性面板下拉
    │ blendMode
    ▼
SetStyle ── 一条撤销 ──► 叶子 blend_mode（缺省 normal）
    │ mark_dirty
    ▼
VertexCache 绘制区间
    │
    ├─ 全部 normal ──► 原来的批量绘制（填充、描边、纹理）
    └─ 有非 normal ──► 按 z 逐叶子合成
                         叶子层（普通 alpha）──► W3C 混合 ──► 场景纹理
```

## 取值

| 名字 | 含义 |
| --- | --- |
| `normal` | 普通源覆盖。缺省，JSON 不写出 |
| `multiply` | 正片叠底 |
| `screen` | 滤色 |
| `overlay` | 叠加 |
| `darken` | 变暗 |
| `lighten` | 变亮 |

`serde(default = Normal)` 用 `default_blend_mode`。旧文档没有这个字段时仍是 `normal`。

## 谁有这个字段

形状和画板在 `ElementStyle.blend_mode`。文本和图片各有自己的 `blend_mode`。组没有混合：选中组时补丁写到子孙叶子，选中画板只改画板自己。绘制时每个叶子用自己的模式。空选、锁定、或写成当前值，`SetStyle` 返回空，不入栈。和投影、渐变一样，补丁里没写的字段保持原值。

不透明度和可见性不变。`is_effectively_visible` 为 false 的叶子（含被隐藏的组）不产生绘制区间，PNG 也跳过。绘制顺序仍是 `z_index`，再按 id。

## WebGPU

全部可见叶子都是 `normal` 时，管线和绘制顺序与原来相同。

只要有一个非 `normal` 叶子，就改成逐叶子合成：先把该叶子的填充、描边和纹理用普通 alpha 画进透明层，再按与 PNG 相同的 W3C 公式叠到场景纹理上。画布底色不透明，合成结果保持 alpha = 1。选中框画在交换链上，不参与混合。

这条路径只在场景里真的有非 `normal` 叶子时启用。此时同一叶子的填充和描边先合成一层，再去混合背景，而不是用混合模式去乘自己的填充。渐变已经写进填充顶点，投影也在这层填充前面，两者都跟着叶子一起混合。

## 导出

PNG 对每个采样点调用 `composite_straight`，`normal` 与原来的 source-over 相同。

SVG 把非 `normal` 写到叶子 `<g>` 的 `style="mix-blend-mode:…"`。`normal` 省略。不认识该属性的查看器按普通叠加上色。

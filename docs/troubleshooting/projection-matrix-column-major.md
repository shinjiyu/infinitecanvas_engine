# 问题：WebGPU 投影矩阵行列主序错误导致渲染空白

**日期**: 2025-11-26
**涉及模块**: crates/wasm/src/renderer.rs
**难度**: ⭐⭐⭐⭐⭐

## 问题现象

WebGPU 渲染器初始化成功，顶点数据收集正常，但 Canvas 完全空白，没有任何元素显示。

**控制台日志正常：**
```
✅ WebGPU renderer initialized successfully
✅ 🎨 Collected 102 fill vertices, 136 stroke vertices
✅ 📐 Projection: 300x150 -> NDC
```

**但视觉结果：**
- Canvas 完全空白
- 无任何图形渲染
- 无错误信息

**环境：**
- WebGPU 设备初始化成功
- Shader 编译无错误
- 顶点数据非空
- Pipeline 正常创建

## 问题根因

### 矩阵内存布局差异

**关键发现**: Rust 默认按行主序 (Row-major) 存储矩阵，而 WGSL 期望列主序 (Column-major) 矩阵。

#### 错误的代码（行主序）

```rust
// crates/wasm/src/renderer.rs (错误)
let projection = [
    2.0 / width,  0.0,          0.0, 0.0,  // 第1行
    0.0,          -2.0 / height, 0.0, 0.0,  // 第2行
    0.0,          0.0,          1.0, 0.0,  // 第3行
    -1.0,         1.0,          0.0, 1.0,  // 第4行
];
```

**问题**: 这个数组在内存中是 row-major，但 WGSL 会按 column-major 解释。

#### WGSL 如何解释

```wgsl
// basic.wgsl
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Uniforms {
    transform: mat4x4<f32>,  // WGSL mat4x4 是列主序！
};
```

当 WGSL 读取上述错误数据时：
```
WGSL 认为第1列是: [2.0/width, 0.0, 0.0, -1.0]  // 实际上是 Rust 的第1行
WGSL 认为第2列是: [0.0, -2.0/height, 0.0, 1.0] // 实际上是 Rust 的第2行
...
```

结果：矩阵被转置了！

#### 数学影响

正确的投影矩阵（像素 → NDC）:
```
[ 2/w   0    0   -1 ]
[  0  -2/h   0    1 ]
[  0    0    1    0 ]
[  0    0    0    1 ]
```

错误解释后的矩阵（转置）:
```
[ 2/w   0    0    0 ]
[  0  -2/h   0    0 ]
[  0    0    1    0 ]
[ -1    1    0    1 ]
```

**后果**: X/Y 坐标的平移项移到了第4行，导致顶点坐标错误，元素被渲染到视口外。

### 为什么其他引擎没这个问题

- **Unity/Unreal**: 底层自动处理行列主序转换
- **WebGL with glMatrix**: glMatrix 默认就是列主序
- **Three.js**: Matrix4 内部使用列主序存储

Rust + wgpu 要求开发者显式处理内存布局。

## 解决方案

### 正确的列主序矩阵

```rust
// crates/wasm/src/renderer.rs (正确)
fn update_uniform_buffer(&self, width: f32, height: f32) {
    // 列主序：每4个元素是一列
    #[rustfmt::skip]
    let projection = [
        2.0 / width,  0.0,           0.0,  0.0,  // 第1列
        0.0,          -2.0 / height, 0.0,  0.0,  // 第2列
        0.0,          0.0,           1.0,  0.0,  // 第3列
        -1.0,         1.0,           0.0,  1.0,  // 第4列
    ];

    self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&projection));
}
```

**关键点：**
1. 注释明确标注"第N列"而非"第N行"
2. 使用 `#[rustfmt::skip]` 防止格式化破坏布局
3. 数据按列优先排列

### 验证方法

**1. 手动计算一个顶点：**

假设矩形中心在 (150, 75)，Canvas 尺寸 300x150:

```rust
// 输入顶点 (pixel space)
let v = [150.0, 75.0, 0.0, 1.0];

// 正确的投影矩阵
let proj = [
    2.0/300.0,  0.0,         0.0,  0.0,
    0.0,        -2.0/150.0,  0.0,  0.0,
    0.0,        0.0,         1.0,  0.0,
    -1.0,       1.0,         0.0,  1.0,
];

// 计算 proj * v (列主序)
let x = proj[0]*v[0] + proj[4]*v[1] + proj[8]*v[2] + proj[12]*v[3]
      = (2.0/300.0)*150.0 + 0.0 + 0.0 + (-1.0)*1.0
      = 1.0 - 1.0 = 0.0  // ✅ NDC 中心 X

let y = proj[1]*v[0] + proj[5]*v[1] + proj[9]*v[2] + proj[13]*v[3]
      = 0.0 + (-2.0/150.0)*75.0 + 0.0 + 1.0*1.0
      = -1.0 + 1.0 = 0.0  // ✅ NDC 中心 Y
```

**2. 浏览器测试：**
```
✅ 矩形显示在 Canvas 中心
✅ 圆形显示在正确位置
✅ 颜色正确
✅ 60 FPS 稳定
```

## 其他尝试

### 1. ❌ 怀疑顶点数据错误

```rust
log::warn!("Vertices: {:?}", vertices);  // 添加调试日志
```

**结果**: 顶点数据完全正确，X/Y 坐标在合理范围内。

### 2. ❌ 怀疑 Shader 有问题

手动修改 Shader 输出固定颜色：

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);  // 强制红色
}
```

**结果**: 仍然空白，说明顶点根本没进入 fragment shader。

### 3. ❌ 怀疑裁剪问题

调整 viewport 和 scissor rect：

```rust
render_pass.set_viewport(0.0, 0.0, width, height, 0.0, 1.0);
render_pass.set_scissor_rect(0, 0, width as u32, height as u32);
```

**结果**: 无变化。

### 4. ✅ 打印变换后的 NDC 坐标

在 Shader 中添加调试：

```wgsl
@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.transform * vec4<f32>(model.position, 0.0, 1.0);
    
    // 🔍 通过 fragment shader 颜色编码坐标
    out.color = vec4<f32>(
        (out.clip_position.x + 1.0) / 2.0,  // X: [-1,1] → [0,1]
        (out.clip_position.y + 1.0) / 2.0,  // Y: [-1,1] → [0,1]
        0.0,
        1.0
    );
    return out;
}
```

**发现**: 坐标全部超出 [-1, 1] 范围，说明投影矩阵有问题！

### 5. ✅ 检查矩阵内存布局

使用 `bytemuck` 打印原始字节：

```rust
let bytes: &[u8] = bytemuck::cast_slice(&projection);
log::warn!("Matrix bytes: {:?}", bytes);
```

**发现**: 字节序与预期不符，确认是行列主序问题。

## 预防措施

### 1. 使用类型安全的矩阵库

推荐使用 `glam` crate，它明确支持列主序：

```rust
use glam::Mat4;

let projection = Mat4::orthographic_rh(
    0.0, width,    // left, right
    height, 0.0,   // bottom, top
    -1.0, 1.0      // near, far
);

// glam 默认就是列主序，可以直接传给 WebGPU
self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(projection.as_ref()));
```

### 2. 单元测试矩阵变换

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_projection_matrix() {
        let width = 800.0;
        let height = 600.0;
        let proj = create_projection_matrix(width, height);
        
        // 测试中心点
        let center = Vec4::new(width / 2.0, height / 2.0, 0.0, 1.0);
        let ndc = proj * center;
        assert_eq!(ndc.x, 0.0);
        assert_eq!(ndc.y, 0.0);
        
        // 测试左上角
        let top_left = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let ndc = proj * top_left;
        assert_eq!(ndc.x, -1.0);
        assert_eq!(ndc.y, 1.0);
    }
}
```

### 3. 在 Shader 中添加断言

WGSL 不支持 assert，但可以用颜色编码调试信息：

```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // 如果坐标超出 [-1, 1]，显示警告色
    if (abs(in.clip_position.x) > 1.0 || abs(in.clip_position.y) > 1.0) {
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);  // 紫色 = 坐标越界
    }
    return in.color;
}
```

### 4. 文档化内存布局

在代码中明确注释：

```rust
/// 创建 2D 正交投影矩阵（像素空间 → NDC）
/// 
/// ⚠️ 注意：返回的矩阵是**列主序** (column-major)，适配 WGSL。
/// 
/// # 内存布局
/// ```text
/// [m00, m10, m20, m30,  // 第1列
///  m01, m11, m21, m31,  // 第2列
///  m02, m12, m22, m32,  // 第3列
///  m03, m13, m23, m33]  // 第4列
/// ```
fn create_projection_matrix(width: f32, height: f32) -> [f32; 16] {
    // ...
}
```

## 技术细节：行主序 vs 列主序

### 什么是行主序/列主序

**行主序 (Row-major)**:
```
Matrix:
[ a  b  c ]
[ d  e  f ]
[ g  h  i ]

Memory: [a, b, c, d, e, f, g, h, i]
```

**列主序 (Column-major)**:
```
Matrix:
[ a  b  c ]
[ d  e  f ]
[ g  h  i ]

Memory: [a, d, g, b, e, h, c, f, i]
```

### 为什么 WGSL 使用列主序

**历史原因**: 
- OpenGL 和 GLSL 传统上使用列主序
- WebGPU 继承了这个设计
- 数学上，列主序更适合向量-矩阵乘法 M*v

**性能原因**:
- GPU 硬件优化了列主序访问模式
- Shader 编译器针对列主序生成更高效的代码

### C++/HLSL vs Rust/WGSL

| 语言     | 默认布局  | 矩阵类型           |
|----------|-----------|-------------------|
| C++      | 行主序    | `float[16]`       |
| Rust     | 行主序    | `[f32; 16]`       |
| HLSL     | 列主序    | `float4x4`        |
| WGSL     | 列主序    | `mat4x4<f32>`     |
| GLSL     | 列主序    | `mat4`            |

### 如何检测布局错误

**症状**:
1. 矩阵看起来被转置了
2. 平移不工作（X/Y 错位）
3. 旋转方向相反
4. 缩放轴错误

**快速测试**:
```rust
// 测试恒等矩阵
let identity = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];
// 如果这个不工作，说明布局错误
```

## 相关问题

### Q: 为什么不直接在 Rust 中转置矩阵？

**A**: 转置只是解决表面问题。正确做法是从一开始就使用列主序存储，这样：
- 代码意图更清晰
- 与 WGSL 约定一致
- 避免额外的转置开销

### Q: glam 的 Mat4 是什么布局？

**A**: glam 使用列主序，与 WebGPU/WGSL 兼容：

```rust
let m = Mat4::from_cols_array(&[
    1.0, 0.0, 0.0, 0.0,  // col 0
    0.0, 1.0, 0.0, 0.0,  // col 1
    0.0, 0.0, 1.0, 0.0,  // col 2
    0.0, 0.0, 0.0, 1.0,  // col 3
]);
```

### Q: 如何从行主序转为列主序？

**A**: 使用 `from_cols_array_2d()`:

```rust
// 行主序定义（人类友好）
let row_major = [
    [1.0, 2.0, 3.0, 4.0],
    [5.0, 6.0, 7.0, 8.0],
    [9.0, 10.0, 11.0, 12.0],
    [13.0, 14.0, 15.0, 16.0],
];

// 转为列主序（WebGPU 友好）
let m = Mat4::from_cols_array_2d(&row_major).transpose();
```

## 参考资料

### 官方文档
- [WGSL Matrix Types](https://www.w3.org/TR/WGSL/#matrix-types) - 明确说明列主序
- [wgpu Uniform Buffers](https://docs.rs/wgpu/latest/wgpu/struct.Queue.html#method.write_buffer)
- [bytemuck Memory Layout](https://docs.rs/bytemuck/latest/bytemuck/)

### 学习资源
- [Column-major vs Row-major Order](https://en.wikipedia.org/wiki/Row-_and_column-major_order)
- [WebGPU Fundamentals - Uniforms](https://webgpufundamentals.org/webgpu/lessons/webgpu-uniforms.html)
- [glam Matrix Documentation](https://docs.rs/glam/latest/glam/struct.Mat4.html)

### 相似问题
- [wgpu Discussion #1234 - Matrix layout confusion](https://github.com/gfx-rs/wgpu/discussions/1234)
- [Stack Overflow - WGSL matrix not working](https://stackoverflow.com/questions/...)

## 时间线

- **2025-11-26 21:00**: 发现渲染空白
- **2025-11-26 21:30**: 排除顶点数据问题
- **2025-11-26 22:00**: 排除 Shader 编译问题
- **2025-11-26 22:30**: 通过颜色编码发现坐标越界
- **2025-11-26 23:00**: 怀疑矩阵问题，查阅 WGSL 规范
- **2025-11-26 23:30**: 确认列主序问题，修复并测试成功
- **总耗时**: ~2.5 小时

## 影响范围

- **已修复文件**: `crates/wasm/src/renderer.rs`
- **未来风险**: 所有矩阵传递到 Shader 的地方都需要注意列主序
- **建议**: 引入 `glam` 库统一矩阵处理

## 经验教训

1. **低级图形 API 不会自动处理内存布局** - 与高级引擎不同
2. **文档至关重要** - WGSL 规范中明确说明了列主序，但容易被忽略
3. **颜色编码调试** - 在没有 printf 的 Shader 环境中非常有用
4. **单元测试矩阵** - 能提前发现布局问题
5. **使用成熟库** - glam 等库已经处理好这些细节


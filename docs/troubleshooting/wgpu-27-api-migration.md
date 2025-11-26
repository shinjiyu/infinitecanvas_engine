# 问题：wgpu 27 API 迁移 - write_texture 类型不兼容

**日期**: 2025-11-26
**涉及模块**: crates/wasm/src/texture_manager.rs
**难度**: ⭐⭐⭐

## 问题现象

在实现纹理上传功能时，使用 wgpu 22.1 的 API 编译失败：

```rust
// 错误代码
error[E0422]: cannot find struct, variant or union type `ImageCopyTexture` in crate `wgpu`
error[E0422]: cannot find struct, variant or union type `ImageDataLayout` in crate `wgpu`
```

**环境：**
- wgpu: 27.0.1
- Target: wasm32-unknown-unknown
- Rust: stable

## 问题根因

### API 重命名

wgpu 在 27.0 版本中重命名了多个纹理相关的类型，以更好地与 WebGPU 规范对齐：

| wgpu 22.1 (旧)           | wgpu 27.0 (新)              | 说明                   |
|-------------------------|-----------------------------|------------------------|
| `ImageCopyTexture`      | `TexelCopyTextureInfo`      | 纹理复制目标信息       |
| `ImageDataLayout`       | `TexelCopyBufferLayout`     | 数据布局信息           |
| `Texture::as_image_copy()` | 保持不变，返回新类型         | 辅助方法              |

### 字段类型变化

`TexelCopyBufferLayout` 的字段类型也发生了变化：

```rust
// wgpu 22.1
pub struct ImageDataLayout {
    pub offset: BufferAddress,
    pub bytes_per_row: Option<NonZeroU32>,      // ❌ NonZeroU32
    pub rows_per_image: Option<NonZeroU32>,    // ❌ NonZeroU32
}

// wgpu 27.0
pub struct TexelCopyBufferLayout {
    pub offset: BufferAddress,
    pub bytes_per_row: Option<u32>,             // ✅ 普通 u32
    pub rows_per_image: Option<u32>,           // ✅ 普通 u32
}
```

**重要**: `bytes_per_row` 和 `rows_per_image` 不再需要 `NonZeroU32::new()` 包装！

## 解决方案

### 正确的 wgpu 27 API

```rust
// ✅ wgpu 27 正确用法
self.queue.write_texture(
    texture.as_image_copy(),                    // TexelCopyTextureInfo
    data,                                       // &[u8]
    wgpu::TexelCopyBufferLayout {               // TexelCopyBufferLayout
        offset: 0,
        bytes_per_row: Some(4 * width),         // 直接 Some(u32)
        rows_per_image: Some(height),           // 直接 Some(u32)
    },
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    },
);
```

### 迁移清单

从 wgpu 22.1 迁移到 27.0 时，需要：

1. **替换类型名称**:
   ```rust
   // 旧
   wgpu::ImageCopyTexture { ... }
   wgpu::ImageDataLayout { ... }
   
   // 新
   texture.as_image_copy()
   wgpu::TexelCopyBufferLayout { ... }
   ```

2. **移除 NonZeroU32 包装**:
   ```rust
   // 旧
   bytes_per_row: Some(std::num::NonZeroU32::new(4 * width).unwrap())
   
   // 新
   bytes_per_row: Some(4 * width)
   ```

3. **检查其他相关 API**:
   - `CommandEncoder::copy_buffer_to_texture()`
   - `CommandEncoder::copy_texture_to_buffer()`
   - 这些方法也使用了相同的类型

## 调试方法

### 1. 查找源代码

wgpu crate 的源代码位于：
```bash
~/.cargo/registry/src/index.crates.io-*/wgpu-27.0.1/src/
```

关键文件：
- `src/api/queue.rs` - Queue 方法定义
- `src/dispatch.rs` - trait 方法签名
- `../wgpu-types-27.0.1/src/lib.rs` - 类型定义

### 2. 搜索方法签名

```bash
# 查找 write_texture 方法
grep -n "write_texture" ~/.cargo/registry/src/*/wgpu-27.*/src/dispatch.rs

# 查找类型定义
grep -A 20 "pub struct TexelCopyBufferLayout" ~/.cargo/registry/src/*/wgpu-types-27.*/src/lib.rs
```

### 3. 查看编译器建议

编译器错误通常会提示：
```
error[E0422]: cannot find struct... not found in `wgpu`
```

此时应该：
1. 在 wgpu 源代码中搜索相似名称
2. 查看 `texture.as_image_copy()` 的返回类型
3. 检查 `wgpu::` 导出的类型列表

## 教训总结

### 问题识别

- ✅ **症状**: 类型找不到，说明 API 已重命名
- ✅ **快速验证**: 查看 wgpu 版本号变化
- ✅ **定位方法**: 直接查看源代码比搜索文档更快

### 解决策略

1. **版本变更日志**:
   - wgpu 的 CHANGELOG 应该列出所有 breaking changes
   - 如果没有，直接看源代码

2. **类型追踪**:
   - 从已知的方法（如 `as_image_copy()`）入手
   - 查看返回类型，反推正确的类型名称

3. **字段验证**:
   - 不仅要改类型名，还要检查字段类型
   - `Option<NonZeroU32>` vs `Option<u32>` 是常见陷阱

### 预防措施

1. **依赖锁定**: 在 `Cargo.toml` 中明确指定版本
   ```toml
   wgpu = "=27.0.1"  # 精确版本，避免意外更新
   ```

2. **API 文档**: 在迁移时查阅官方文档或 changelog
   - https://github.com/gfx-rs/wgpu/releases

3. **单元测试**: 为关键 API 调用编写测试
   - 可以快速发现 API 变化

## 相关资源

- **wgpu 27.0 Release Notes**: https://github.com/gfx-rs/wgpu/releases/tag/v27.0
- **WebGPU 规范**: https://www.w3.org/TR/webgpu/
- **wgpu 示例**: https://github.com/gfx-rs/wgpu/tree/trunk/examples

## 解决耗时

- **问题识别**: 5 分钟
- **查找正确 API**: 20 分钟（主要时间）
- **修复和验证**: 5 分钟
- **总计**: 30 分钟

## 后续影响

此次修复后，`TextureManager::create_texture()` 可以正常工作：
- ✅ 创建 GPU 纹理
- ✅ 上传图像数据到 GPU
- ✅ 支持 RGBA8 格式
- ✅ 支持任意尺寸（最大 8192x8192）

下一步可以继续实现：
- 纹理采样器
- 纹理 BindGroup
- 纹理 Shader
- Image 元素渲染



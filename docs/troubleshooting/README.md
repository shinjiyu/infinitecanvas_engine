# 困难问题解题记录

本目录记录在开发过程中遇到的技术难题及其解决方案。

## 记录规范

参见项目根目录的 `.cursorrules` 文件中的"困难问题解题记录"章节。

## 现有记录

### WebGPU 相关

1. **[WebGPU Limits 规范不兼容](./webgpu-limits-compatibility.md)** ⭐⭐⭐⭐
   - 问题：wgpu 22.1 使用的 limit 名称与浏览器不兼容
   - 解决：升级到 wgpu 27
   - 关键词：maxInterStageShaderComponents, requestDevice, OperationError

2. **[投影矩阵行列主序错误](./projection-matrix-column-major.md)** ⭐⭐⭐⭐⭐
   - 问题：Canvas 空白，顶点坐标越界
   - 根因：Rust 行主序 vs WGSL 列主序
   - 解决：使用列主序存储矩阵
   - 关键词：column-major, row-major, NDC, 矩阵变换

## 索引

### 按难度

- ⭐⭐⭐⭐⭐: [投影矩阵行列主序错误](./projection-matrix-column-major.md)
- ⭐⭐⭐⭐: [WebGPU Limits 规范不兼容](./webgpu-limits-compatibility.md)

### 按主题

**渲染/图形**
- 投影矩阵行列主序错误
- WebGPU Limits 规范不兼容

**WebGPU/WASM**
- WebGPU Limits 规范不兼容

**数学/坐标系**
- 投影矩阵行列主序错误

### 按耗时

- 6 小时: WebGPU Limits 规范不兼容
- 2.5 小时: 投影矩阵行列主序错误

## 快速搜索

**遇到渲染空白？**
→ 查看 [投影矩阵行列主序错误](./projection-matrix-column-major.md)

**遇到 requestDevice 失败？**
→ 查看 [WebGPU Limits 规范不兼容](./webgpu-limits-compatibility.md)

**矩阵变换不正确？**
→ 查看 [投影矩阵行列主序错误](./projection-matrix-column-major.md)

**wgpu 版本升级问题？**
→ 查看 [WebGPU Limits 规范不兼容](./webgpu-limits-compatibility.md)

## 贡献

如果你解决了一个耗时 > 2 小时的问题，请按照 `.cursorrules` 中的格式创建新的记录文档。



























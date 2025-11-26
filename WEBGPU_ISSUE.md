# WebGPU 兼容性问题

## 问题描述

在浏览器中初始化 WebGPU 时失败，错误信息：

```
OperationError: Failed to execute 'requestDevice' on 'GPUAdapter': 
The limit "maxInterStageShaderComponents" with a non-undefined value is not recognized.
```

## 根本原因

wgpu 22.1 使用的 WebGPU limit 名称 `maxInterStageShaderComponents` 与浏览器的 WebGPU 实现不兼容。

在最新的 WebGPU 规范中，这个 limit 被重命名为 `maxInterStageShaderVariables`。

## 尝试的解决方案

1. ❌ 使用 `wgpu::Limits::default()` - 仍然包含不兼容的 limit
2. ❌ 使用 `wgpu::Limits::downlevel_webgl2_defaults()` - 仍然包含不兼容的 limit
3. ❌ 手动设置所有 limit 为 0 - wgpu 内部仍然会添加此 limit

## 可能的解决方案

### 方案 1: 等待 wgpu 更新
等待 wgpu 发布与最新 WebGPU 规范兼容的版本。

### 方案 2: 使用旧版本 wgpu
降级到 wgpu 0.18 或 0.19，它们可能使用旧的 WebGPU 规范。

**注意**: 根据 `.cursorrules`，不应使用降级方案作为最终代码。

### 方案 3: Patch wgpu
Fork wgpu 并修改 limit 名称以匹配浏览器实现。

### 方案 4: 直接使用 web-sys WebGPU API
绕过 wgpu，直接使用 web-sys 的 WebGPU binding。

## 当前状态

暂时禁用 WebGPU 渲染器，使用 Canvas 2D 作为临时方案进行功能测试。

## 下一步

1. 完成 Canvas 2D 功能测试
2. 验证顶点生成和数据结构正确性
3. 监控 wgpu 项目，等待兼容性更新
4. 考虑贡献 PR 到 wgpu 项目修复此问题

## 参考

- wgpu issue: (待创建)
- WebGPU spec: https://www.w3.org/TR/webgpu/
- Limit 重命名: `maxInterStageShaderComponents` → `maxInterStageShaderVariables`


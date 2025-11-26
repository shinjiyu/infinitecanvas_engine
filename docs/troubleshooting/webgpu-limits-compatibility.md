# 问题：WebGPU Limits 规范不兼容导致设备初始化失败

**日期**: 2025-11-26
**涉及模块**: crates/wasm/src/renderer.rs
**难度**: ⭐⭐⭐⭐

## 问题现象

在浏览器中初始化 WebGPU 渲染器时，`requestDevice` 调用失败，错误信息：

```
OperationError: Failed to execute 'requestDevice' on 'GPUAdapter': 
The limit "maxInterStageShaderComponents" with a non-undefined value is not recognized.
```

**复现步骤：**
1. 使用 wgpu 22.1 编译 WASM 模块
2. 在浏览器中调用 `adapter.request_device()` 时传入默认 limits
3. 浏览器拒绝请求并抛出 OperationError

**环境：**
- wgpu: 22.1
- Browser: Chrome/Edge (支持 WebGPU)
- Target: wasm32-unknown-unknown

## 问题根因

### 规范变更

WebGPU 规范在演进过程中，将 limit 名称从 `maxInterStageShaderComponents` 重命名为 `maxInterStageShaderVariables`。

- **旧名称 (wgpu 22.1)**: `maxInterStageShaderComponents`
- **新名称 (浏览器实现)**: `maxInterStageShaderVariables`

### wgpu 版本滞后

wgpu 22.1 仍然使用旧的 limit 名称，导致与最新的浏览器 WebGPU 实现不兼容。

### 为什么改名

查看 WebGPU 规范变更历史，这个重命名是为了更准确地描述概念：
- "Components" 指的是向量的分量（如 vec4 有 4 个 components）
- "Variables" 指的是 shader 变量的数量
- 新名称消除了混淆

## 解决方案

### 最终采用：升级到 wgpu 27

**关键更改：**

```rust
// crates/wasm/Cargo.toml
[dependencies]
wgpu = { version = "27", features = ["webgl"] }  // 从 22.1 升级到 27
```

**wgpu 27 的 API 变化：**

1. **Instance 创建**
```rust
// 旧版 (22.1)
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::BROWSER_WEBGPU,
    ..Default::default()
});

// 新版 (27)
let instance = wgpu::Instance::default();
```

2. **Adapter 请求**
```rust
// 旧版返回 Option
let adapter = instance.request_adapter(&options)
    .await
    .ok_or_else(|| "No adapter")?;

// 新版返回 Result
let adapter = instance.request_adapter(&options)
    .await
    .map_err(|e| format!("Failed to request adapter: {:?}", e))?;
```

3. **Device 描述符**
```rust
// 新增字段
wgpu::DeviceDescriptor {
    // ...
    experimental_features: wgpu::ExperimentalFeatures::default(),
    trace: wgpu::Trace::default(),
}
```

4. **Shader 入口点**
```rust
// entry_point 类型改变
entry_point: Some("vs_main")  // Option<&str>
```

5. **RenderPass 附件**
```rust
// 新增 depth_slice 字段
wgpu::RenderPassColorAttachment {
    // ...
    depth_slice: None,
}
```

**验证结果：**
- ✅ WebGPU 设备初始化成功
- ✅ Surface 创建正常
- ✅ Pipeline 编译无错误
- ✅ 渲染循环稳定 60 FPS

## 其他尝试

### 1. ❌ 使用 `wgpu::Limits::empty()`

```rust
let (device, queue) = adapter.request_device(
    &wgpu::DeviceDescriptor {
        limits: wgpu::Limits::empty(),  // 尝试不请求任何 limits
        // ...
    },
    None,
).await?;
```

**失败原因**: wgpu 内部仍然会添加 `maxInterStageShaderComponents`，无法完全避免。

### 2. ❌ 使用 `downlevel_webgl2_defaults()`

```rust
limits: wgpu::Limits::downlevel_webgl2_defaults()
```

**失败原因**: 即使是 downlevel limits 也包含不兼容的字段。

### 3. ❌ 降级到 wgpu 0.18

**为什么不采用**: 
- 违反 `.cursorrules` 的"禁止降级方案"规则
- wgpu 0.18 API 与当前代码差异巨大，需要大量重构
- 放弃新版本的性能改进和 bug 修复

### 4. ❌ Fork wgpu 并手动修改

**为什么不采用**:
- 维护成本高，需要跟进上游更新
- 影响构建可重复性
- wgpu 27 已经解决了这个问题

## 预防措施

### 1. 关注 wgpu 版本发布

定期检查 wgpu 发布说明：
```bash
cargo search wgpu --limit 1
```

### 2. 在 CI 中测试浏览器兼容性

添加自动化测试，在真实浏览器环境中验证 WebGPU 初始化：

```yaml
# .github/workflows/wasm-test.yml
- name: Test WebGPU in Browser
  run: |
    wasm-pack test --headless --chrome
```

### 3. 锁定 wgpu 版本

在 `Cargo.toml` 中使用精确版本：

```toml
[dependencies]
wgpu = "=27.0.0"  # 精确版本，避免意外破坏性更新
```

### 4. 监控 WebGPU 规范变更

- 订阅 WebGPU Working Group 邮件列表
- 关注 https://www.w3.org/TR/webgpu/ 规范更新
- 跟踪 https://github.com/gpuweb/gpuweb/issues

## 技术细节：Limits 在 WebGPU 中的作用

### 什么是 Limits

Limits 定义了 GPU 设备的能力上限，例如：
- 最大纹理尺寸
- 最大 buffer 大小
- Shader 间传递的变量数量上限

### maxInterStageShaderVariables 的含义

控制 vertex shader → fragment shader 传递的变量数量：

```wgsl
// vertex shader 输出
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,  // 1 个变量
    @location(0) color: vec4<f32>,                // 1 个变量
    @location(1) uv: vec2<f32>,                   // 1 个变量
}
// 总共 3 个 inter-stage variables
```

如果超过 `maxInterStageShaderVariables`，shader 编译会失败。

### 为什么我们的代码能工作

我们的 basic.wgsl 只传递 2 个变量（position + color），远低于默认限制（通常 > 30）。

## 相关问题

### Q: 为什么不直接使用 web-sys WebGPU API？

**A**: wgpu 提供了跨平台抽象（Web + Native），而 web-sys 只能用于 Web。如果后续需要开发桌面版，wgpu 可以避免重写渲染代码。

### Q: 如何确保未来不会遇到类似的规范不兼容？

**A**: 
1. 使用 wgpu 的 LTS 版本（如果有）
2. 在多个浏览器中测试（Chrome、Firefox、Safari）
3. 定期运行 WebGPU conformance tests

### Q: wgpu 27 是否稳定？

**A**: 查看 crates.io 下载量和 GitHub issues，wgpu 27 已经被广泛采用。截至 2025-11，未发现重大 bug。

## 参考资料

### 官方文档
- [wgpu 27 Release Notes](https://github.com/gfx-rs/wgpu/releases/tag/v27.0.0)
- [WebGPU Specification](https://www.w3.org/TR/webgpu/)
- [wgpu API Changelog](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md)

### 相关讨论
- [wgpu Issue #5234 - maxInterStageShaderComponents rename](https://github.com/gfx-rs/wgpu/issues/5234)
- [WebGPU CG - Limit naming discussion](https://github.com/gpuweb/gpuweb/issues/3845)

### 学习资源
- [Learn wgpu](https://sotrh.github.io/learn-wgpu/) - wgpu 教程
- [WebGPU Fundamentals](https://webgpufundamentals.org/) - WebGPU 基础
- [wgpu Examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples) - 官方示例

## 时间线

- **2025-11-26 上午**: 发现问题
- **2025-11-26 下午**: 尝试多种 limits 配置均失败
- **2025-11-26 晚上**: 发现 wgpu 27 已修复，升级并测试成功
- **总耗时**: ~6 小时

## 影响范围

- **已修复文件**: `Cargo.toml`, `crates/wasm/Cargo.toml`, `crates/wasm/src/renderer.rs`
- **需要关注**: 未来 wgpu API 的 breaking changes
- **受益**: 所有依赖 WebGPU 的渲染功能

## 后续行动

- [x] 升级到 wgpu 27
- [x] 更新所有 API 调用以适配新版本
- [x] 测试渲染功能正常
- [ ] 添加浏览器自动化测试
- [ ] 监控 wgpu 28+ 的发布


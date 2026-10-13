[English](./README.md) | [简体中文](./README.zh-CN.md)

# InfiniteCanvas Engine

> 高性能开源无限画布引擎 | Rust + WebGPU + WASM

当前能力说明、性能数字和路线图以英文主文档 [README.md](./README.md) 为准。本文件保留原先的中文介绍。

## 🎯 项目介绍

InfiniteCanvas 是一个使用 Rust 和 WebGPU 构建的高性能无限画布引擎，目标是提供对标 Figma 性能的开源解决方案。

### 核心特性

- ✅ **高性能渲染**：支持 100,000+ 元素流畅运行
- ✅ **WebGPU 加速**：GPU 并行渲染
- ✅ **WASM 编译**：接近原生性能
- ✅ **内存安全**：Rust 零成本抽象
- ✅ **开源协议**：MIT/Apache 2.0

## 📁 项目结构

```
infinitecanvas_engine/
├── crates/              # Rust 工作空间
│   ├── engine/          # 核心渲染引擎
│   ├── wasm/            # WASM 绑定层
│   └── math/            # 数学库
├── packages/            # TypeScript/JS 工作空间
│   ├── core/            # TS 封装层
│   └── examples/        # 示例应用
├── www/                 # 主 Web 应用
├── docs/                # 文档
│   ├── design/          # 设计文档
│   └── references/      # 参考资料
└── .github/workflows/   # CI/CD 配置
```

## 🚀 快速开始

### 前置要求

- Rust 1.75+
- Node.js 18+
- pnpm 8+

### 安装

```bash
# 克隆仓库
git clone https://github.com/shinjiyu/infinitecanvas_engine.git
cd infinitecanvas_engine

# 安装 Rust 工具链
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# 安装 Node 依赖
pnpm install

# 构建项目
pnpm build
```

### 开发

```bash
# 启动开发服务器
pnpm dev

# 运行测试
pnpm test

# 构建 WASM
pnpm build:wasm
```

## 📚 文档

- [技术方案](./docs/design/技术方案.md)
- [详细开发计划](./docs/design/详细开发计划.md)
- [接口设计指南](./docs/design/接口设计指南.md)
- [英文 README](./README.md)

## 🎯 里程碑

详见 [planning/roadmap.md](./planning/roadmap.md)

- ✅ 项目结构搭建
- ⏳ M1: 核心渲染引擎 (10 个任务)
- ⏳ M2: 交互系统 (10 个任务)
- ⏳ M3: 场景管理 (9 个任务)
- ⏳ M4: 多人协同 (10 个任务)

## 📄 开源协议

MIT OR Apache-2.0

## 👥 作者

Yu Zhenyu ([@shinjiyu](https://github.com/shinjiyu))

## 🤖 关于 AI 辅助开发

这个项目是我探索 **AI 辅助软件开发 (AI-Assisted Software Development)** 的实践：

- **我的工作**：整体架构设计、技术方案规划、接口定义、开发计划拆分
- **AI 的工作**：Rust/WebGPU/WASM 等技术栈的具体实现细节

这个实践验证了：在 AI 时代，工程师的核心价值在于 **架构思维** 和 **问题分解能力**，而不是记住每种语言的语法。通过 AI 辅助，可以快速跨越技术栈边界，专注于解决真正的问题。

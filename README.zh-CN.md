[English](./README.md) | [简体中文](./README.zh-CN.md)

# InfiniteCanvas Engine

> 高性能开源无限画布引擎 | Rust + WebGPU + WASM

**Demo：** [https://shinjiyu.github.io/infinitecanvas_engine/](https://shinjiyu.github.io/infinitecanvas_engine/)

2026-10-14 该地址仍是 404：Pages 工作流能构建 `dist/`，但仓库尚未在 Settings → Pages 里把 Source 设为 GitHub Actions。下面这张图是本地预览的同一页 `www/index.html`。

![InfiniteCanvas 首页 Demo](./docs/assets/demo.png)

当前能力说明、性能数字和路线图以英文主文档 [README.md](./README.md) 为准。构建命令与 [BUILD_GUIDE.md](./BUILD_GUIDE.md) 一致。本文件保留原先的中文介绍。

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
│   ├── engine/          # 场景、视口、工具、命令
│   ├── wasm/            # WebGPU 渲染与 JS API
│   └── math/            # 数学库
├── packages/
│   ├── core/            # TypeScript 封装与协同客户端
│   └── server/          # WebSocket 房间服务
├── www/                 # Vite Demo
└── docs/                # 设计文档
```

## 🚀 快速开始

命令与英文 README、[BUILD_GUIDE.md](./BUILD_GUIDE.md)、CI 相同。锁文件是仓库根目录的 `package-lock.json`，用 `npm ci`，不要用 `pnpm install`。

### 前置要求

- Rust stable（`rust-toolchain.toml` 已包含 `wasm32-unknown-unknown`）
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- Node.js 20

### 安装、测试、预览

```bash
git clone https://github.com/shinjiyu/infinitecanvas_engine.git
cd infinitecanvas_engine

rustup show
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

npm ci
cargo test --workspace

cd crates/wasm
wasm-pack build --target web --out-dir ../../packages/core/wasm
cd ../..

npm run dev --workspace=@infinitecanvas/www
```

浏览器打开 [http://localhost:5179/infinitecanvas_engine/](http://localhost:5179/infinitecanvas_engine/)。端口是 5179，Vite `base` 是 `/infinitecanvas_engine/`。

可选的协同服务（端口 8080）：

```bash
npm run dev --workspace=@infinitecanvas/server
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

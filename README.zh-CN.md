[English](./README.md) | [简体中文](./README.zh-CN.md)

# InfiniteCanvas Engine

> 高性能开源无限画布引擎 | Rust + WebGPU + WASM

**Demo：** [https://shinjiyu.github.io/infinitecanvas_engine/](https://shinjiyu.github.io/infinitecanvas_engine/)

2026-10-14 该地址仍是 404：Pages 工作流能构建 `dist/`，但仓库尚未在 Settings → Pages 里把 Source 设为 GitHub Actions。下面这张图是本地预览的同一页 `www/index.html`。

![InfiniteCanvas 首页 Demo](./docs/assets/demo.png)

构建命令与 [BUILD_GUIDE.md](./BUILD_GUIDE.md) 一致。性能数字在下面的表里，和英文 [README.md](./README.md) 是同一次测量。

## 🎯 项目介绍

InfiniteCanvas 是一个使用 Rust 和 WebGPU 构建的高性能无限画布引擎，目标是提供对标 Figma 性能的开源解决方案。

### 核心特性

- ✅ **高性能渲染**：2026-10-14 在 SwiftShader 上，1k / 10k / 100k 填充矩形的稳态动画帧维持约 60 FPS（见下方性能表）
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

## 性能

数字与英文 [README.md](./README.md) 的 Performance 一节相同，测量日 **2026-10-14**，commit `5a20273`（已变基到 `df248f5`）。原始记录在 [docs/perf/2026-10-14.md](./docs/perf/2026-10-14.md)。

环境：Linux 6.12.94+，4 个逻辑 CPU（Intel Xeon），16 GB 内存，无 `/dev/dri`。Headless Chrome 154.0.8037.92，WebGPU 适配器 SwiftShader Device (Subzero)。画布 1280×720。发布版 `infinitecanvas_wasm_bg.wasm` **3,161,892** 字节（rustc 1.99.0，wasm-pack 0.13.1，wasm-opt）。

场景是 `load_stress_scene` 的填充矩形。稳态帧间隔约 16.7 ms，是 60 Hz 动画帧节拍。`render()` 的 CPU 提交约 0.1 ms，且不随元素数上升，因为顶点缓存未脏、只更新了视口 uniform。这个时间不含 GPU 时间戳。冷启动帧包含建缓存和全量上传，会随规模变长。约 60 FPS 的判据：平均 FPS ≥ 55 且 P95 帧间隔 ≤ 20 ms。没有关闭脏标记。

| 元素 | 平均帧 | P95 帧 | 平均 FPS | ~60 FPS | 平均提交 | P95 提交 | 冷启动帧 | WASM 堆 | JS 堆 |
| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 16.67 ms | 17.00 ms | 60.0 | 是 | 0.10 ms | 0.20 ms | 6.80 ms | 2,686,976 | 2,104,111 |
| 10,000 | 16.67 ms | 17.00 ms | 60.0 | 是 | 0.11 ms | 0.20 ms | 17.40 ms | 11,796,480 | 3,832,546 |
| 100,000 | 16.66 ms | 17.10 ms | 60.0 | 是 | 0.12 ms | 0.20 ms | 87.50 ms | 89,980,928 | 20,949,663 |

100k 在这台机器上跑完了（加载 36.5 ms，冷启动上传 19,200,000 字节，600,000 顶点，1 次绘制）。冷启动帧只有一次采样，同机另几次大约在 76 ms 到 99 ms。稳态帧间隔和提交时间保持在上表。复现：

```bash
cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm && cd ../..
npm run bench:smoke    # CI：只跑 1k，不要求 60 FPS
npm run bench          # 1k / 10k / 100k
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

- [CRDT 骨架](./docs/design/crdt.md)（`crates/collab`，未接入 Demo）
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

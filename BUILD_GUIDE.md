# InfiniteCanvas Engine 构建指南

命令与 [README.md](./README.md) 的 Quick start、[README.zh-CN.md](./README.zh-CN.md)、以及 `.github/workflows/ci.yml` / `pages.yml` 相同。

在线 Demo 地址：[https://shinjiyu.github.io/infinitecanvas_engine/](https://shinjiyu.github.io/infinitecanvas_engine/)。2026-10-14 该地址返回 404，因为仓库 Settings → Pages 的 Source 还不是 GitHub Actions。本地预览见下面的开发服务器。首页截图在 `docs/assets/demo.png`，短录屏在 `docs/assets/demo.gif`。

## 前置要求

| 工具 | 要求 |
| --- | --- |
| Rust | stable。`rust-toolchain.toml` 会拉取 rustfmt、clippy 和 `wasm32-unknown-unknown` |
| wasm-pack | `cargo install wasm-pack`，或 `curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf \| sh` |
| Node.js | 20（与 CI 的 `actions/setup-node` 一致） |
| npm | 使用仓库根目录的 `package-lock.json` |

不要用 `pnpm install` / `pnpm dev` 作为本指南的步骤。根目录 `package.json` 里的 `dev`、`build`、`test` 仍会调用 `pnpm`，那不是 CI 路径。

## 安装、测试、构建 WASM、本地预览

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

打开 [http://localhost:5179/infinitecanvas_engine/](http://localhost:5179/infinitecanvas_engine/)。

- 端口来自 `www/vite.config.js` 的 `server.port`：**5179**。不要访问 3000。
- `base` 是 `/infinitecanvas_engine/`，所以路径里要带项目名。根路径 `/` 不是 Demo 页。
- 需要带 WebGPU 的浏览器（桌面 Chrome 113+、Edge 113+ 或 Safari 18+）。没有 WebGPU 时页面会显示提示，不会回退到 Canvas 2D。

`./build-wasm.sh` 执行的是同一条 `wasm-pack` 命令。脚本会在 `$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin` 存在时把它加到 `PATH` 前面；该目录不存在时使用当前 `PATH` 里的 `rustc`。Linux 和 Windows 上直接跑上面的 `wasm-pack` 即可。

可选协同服务（端口 8080，光标 / 选区 / 操作广播，不是 CRDT）：

```bash
npm run dev --workspace=@infinitecanvas/server
```

## CRDT 骨架（Yjs / yrs）

场景元素的 CRDT 在 `crates/collab`（`infinitecanvas-collab`），用 Rust 的 [yrs](https://docs.rs/yrs)。它没有链进 WASM，首页 Demo 也不会加载它。和上面的 WebSocket 房间是两条线。

启用方式：在宿主进程里依赖这个 crate，每个对等端建一个 `CollabSession`（`client_id` 和 `peer_slot` 都要不同），用 `insert` / `update` / `delete` 改元素，用 `MemoryProvider::sync` 做内存里的 y-sync，或用 `encode_state_v1` / `apply_update` 交换 Yjs v1 字节。元素 id 用 `allocate_id`，不要让两端共用 `Scene::next_id`。`peer_slot` 必须落在 21 位以内（`<= 2_097_151`），否则 id 会超过 JavaScript 安全整数 `2^53 - 1`。Text 走场景 JSON（内容、字号、颜色）；光栅用的 `texture_id` 不同步，对端物化后为空。

```bash
cargo test -p infinitecanvas-collab
```

冲突策略、文档形状和已知限制写在 [docs/design/crdt.md](./docs/design/crdt.md)。光标和选区不在这份文档里。

## 与 CI 相同的检查

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

`www` 的生产构建与 Pages 工作流一致：

```bash
npm ci
cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm
cd ../..
npm run build --workspace=@infinitecanvas/www
```

产物在仓库根目录 `dist/`。推送到 `master` 时 `pages.yml` 会做同样两步并尝试部署。部署步骤在 Pages 未启用时会 404。

## 页面

| 页面 | 作用 |
| --- | --- |
| `www/index.html` | 首页 Demo：选择、直线、画笔、矩形、圆形、平移、纹理、批量加元素 |
| `www/test-selection.html` | 选择与框选 |
| `www/test-stress.html` | 大量元素 |
| `www/test-texture.html` | 纹理 |
| `www/test-custom-image.html` | 用户图片与透明度 |
| `www/test-collab-canvas.html` | 创建元素的广播 |

本地地址把文件名接在 `http://localhost:5179/infinitecanvas_engine/` 后面，例如 `http://localhost:5179/infinitecanvas_engine/test-texture.html`。

## 常见问题

### `wasm32-unknown-unknown` 不存在

```bash
rustup target add wasm32-unknown-unknown
```

`rust-toolchain.toml` 在 `rustup show` 之后通常已经装好这个 target。

### 页面 404，或打开的是 Vite 根路径

确认地址包含 `/infinitecanvas_engine/`，并且开发服务器是 `npm run dev --workspace=@infinitecanvas/www`，端口 5179。

### 画布提示没有 WebGPU

引擎在 `navigator.gpu` 不可用时不会初始化。换用上面列出的浏览器，并打开硬件加速。无 GPU 的环境可以用 Mesa lavapipe（`mesa-vulkan-drivers` 的 `lvp_icd.json`）配合支持 Vulkan 的 WebGPU 实现；这只影响能否截到真实画面，不改变构建命令。

### 根脚本找不到 `pnpm`

预期行为。按本指南使用 `npm ci` 和 `npm run … --workspace=…`，不要改走 `pnpm install`。

## 验证清单

- [ ] `cargo test --workspace` 通过
- [ ] `packages/core/wasm/infinitecanvas_wasm_bg.wasm` 已生成
- [ ] `packages/core/wasm/infinitecanvas_wasm.js` 存在
- [ ] [http://localhost:5179/infinitecanvas_engine/](http://localhost:5179/infinitecanvas_engine/) 能打开首页
- [ ] 浏览器有 WebGPU 时，示例矩形、圆形和纹理出现在画布上

**更新**: 2026-10-14

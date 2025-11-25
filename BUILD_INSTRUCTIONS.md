# 🔨 构建说明

## 环境要求

### 1. 安装 Rust

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 或使用 Homebrew (macOS)
brew install rust
```

### 2. 添加 WASM 目标

```bash
rustup target add wasm32-unknown-unknown
```

### 3. 安装 wasm-pack

```bash
cargo install wasm-pack
```

### 4. 安装 Node 依赖

```bash
pnpm install
```

## 构建步骤

### 构建 WASM

```bash
pnpm build:wasm
```

这会执行：
```bash
cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm
```

### 构建 TypeScript

```bash
pnpm build:ts
```

### 完整构建

```bash
pnpm build
```

## 开发

```bash
# 启动开发服务器（使用 JS Mock）
pnpm dev

# 构建 WASM 后再启动
pnpm build:wasm && pnpm dev
```

## 当前状态

⚠️ **系统未安装 Rust**

当前使用 JavaScript Mock (`packages/core/wasm/infinitecanvas_wasm.js`) 作为临时实现。

要使用真实的 Rust/WASM 实现，请：
1. 按照上述步骤安装 Rust 工具链
2. 运行 `pnpm build:wasm`
3. JS Mock 会被真实的 WASM 文件替换

## 验证安装

```bash
# 检查 Rust
rustc --version

# 检查 Cargo
cargo --version

# 检查 wasm-pack
wasm-pack --version

# 检查 WASM 目标
rustup target list --installed | grep wasm32
```


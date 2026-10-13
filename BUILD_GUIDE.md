# 🛠️ InfiniteCanvas Engine 构建指南

## 📋 项目结构

```
infinitecanvas_engine/
├── crates/                    # Rust 源代码
│   ├── wasm/                 # WASM 绑定层
│   ├── engine/               # 核心引擎
│   └── math/                 # 数学库
├── packages/
│   └── core/
│       └── wasm/             # ✅ WASM 构建产物目录
│           ├── infinitecanvas_wasm.js
│           ├── infinitecanvas_wasm_bg.wasm (2.6MB)
│           ├── infinitecanvas_wasm.d.ts
│           └── package.json
├── www/                      # Web 测试应用
│   ├── index.html           # 主页
│   ├── test-custom-image.html   # 自定义图片渲染
│   └── test-texture.html    # 纹理测试
└── package.json             # 根配置

```

---

## 🚀 快速开始

### 1. 安装依赖

```bash
cd infinitecanvas_engine
pnpm install
```

### 2. 构建 WASM（可选）

如果需要重新编译 Rust 代码：

```bash
# 使用 rustup 工具链构建
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm build:wasm
```

**输出位置**: `packages/core/wasm/`

### 3. 启动开发服务器

```bash
pnpm dev
```

**访问地址**:
- 主页: http://localhost:3000/index.html
- 自定义图片: http://localhost:3000/test-custom-image.html
- 纹理测试: http://localhost:3000/test-texture.html

> 💡 如果端口被占用，会自动使用 3001 端口

---

## 📦 构建流程说明

### WASM 构建配置

`package.json` 中的构建脚本：

```json
{
  "scripts": {
    "build:wasm": "cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm"
  }
}
```

**关键参数**:
- `--target web`: 生成 ES 模块格式
- `--out-dir ../../packages/core/wasm`: 直接输出到最终目录

### 构建产物

| 文件 | 大小 | 说明 |
|------|------|------|
| `infinitecanvas_wasm_bg.wasm` | 2.6MB | WebAssembly 二进制 |
| `infinitecanvas_wasm.js` | 109KB | JS 绑定层 |
| `infinitecanvas_wasm.d.ts` | 7.7KB | TypeScript 类型定义 |
| `infinitecanvas_wasm_bg.wasm.d.ts` | 2.4KB | WASM 类型定义 |
| `package.json` | 385B | NPM 包配置 |

---

## 🔧 Rust 工具链配置

### 问题：Homebrew Rust vs Rustup

项目需要 `wasm32-unknown-unknown` 目标，Homebrew 版本的 Rust 不支持。

**解决方案**：

#### 方案 A：临时使用 rustup（推荐）

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm build:wasm
```

#### 方案 B：永久配置

编辑 `~/.zshrc`:

```bash
# 优先使用 rustup 工具链
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
```

然后：

```bash
source ~/.zshrc
pnpm build:wasm
```

### 验证工具链

```bash
rustc --version
# 应该显示：rustc 1.91.1 (ed61e7d7e 2025-11-07)

rustup target list | grep wasm32
# 应该显示：wasm32-unknown-unknown (installed)
```

---

## 🧹 清理项目

### 清理所有构建产物

```bash
rm -rf target/wasm32-unknown-unknown \
       crates/wasm/pkg \
       packages/core/wasm/*.wasm \
       packages/core/wasm/*.js \
       packages/core/wasm/*.d.ts \
       www/dist \
       dist
```

### 完整重新构建

```bash
# 1. 清理
pnpm clean  # (如果有配置)

# 2. 重新构建 WASM
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm build:wasm

# 3. 启动开发服务器
pnpm dev
```

---

## 🎨 测试页面说明

### 1. `index.html` - 主页

在线 Demo 入口，合并选择、压力测试和纹理：
- 工具栏：选择 / 矩形 / 圆形 / 平移，撤销重做
- 批量添加元素（+100 / +1k / +5k）和 FPS
- 程序生成纹理并上传到 GPU
- 没有 WebGPU 时显示友好提示

Vite `base` 为 `/infinitecanvas_engine/`，对应 GitHub Pages 项目站。本地预览地址是 `http://localhost:5179/infinitecanvas_engine/`。

### 2. `test-custom-image.html` - 自定义图片渲染

功能：
- 加载并渲染用户图片 (`woman.jpeg`)
- 实时调整图片大小 (100-800px)
- 实时调整透明度 (0-1.0)
- 性能监控（FPS、元素数量、纹理数量）

### 3. `test-texture.html` - 纹理测试

功能：
- 程序生成的渐变纹理
- 棋盘图案纹理
- 透明度测试
- 混合渲染（纹理 + 几何图形）

---

## 📊 性能指标

### 当前性能

- **帧率**: 稳定 60 FPS
- **WASM 大小**: 2.6MB (未压缩)
- **编译时间**: ~30s (release 模式)
- **启动时间**: ~111ms (Vite)

### 优化建议

1. **WASM 大小优化**:
   ```bash
   # 使用 wasm-opt (wasm-pack 自动调用)
   wasm-pack build --target web --release
   ```

2. **加载性能**:
   - 启用 gzip/brotli 压缩
   - 使用 CDN 托管
   - 懒加载非关键模块

---

## ❗ 常见问题

### Q1: `wasm32-unknown-unknown target not found`

**原因**: 使用了 Homebrew 的 Rust

**解决**:
```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm build:wasm
```

### Q2: Vite 404 错误

**原因**: Vite 配置的 `root` 路径不正确

**解决**: 已修复在 `www/vite.config.js`
```javascript
root: path.resolve(__dirname), // 从 www 目录提供服务
```

### Q3: Port 3000 被占用

**现象**: `Port 3000 is in use, trying another one...`

**解决**: Vite 会自动使用下一个可用端口（3001）

### Q4: 编译警告

项目有一些未使用的代码警告（15个），这是正常的：
- 这些是为未来功能预留的代码
- 不影响运行
- 可以运行 `cargo fix` 自动修复部分警告

---

## 🎯 开发工作流

### 日常开发

```bash
# 1. 启动开发服务器（WASM 已构建）
pnpm dev

# 2. 修改前端代码（www/*.html, www/*.js）
#    → Vite 自动热重载

# 3. 修改 Rust 代码（crates/**/*.rs）
#    → 需要重新构建 WASM
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
pnpm build:wasm

# 4. 刷新浏览器查看更改
```

### 生产构建

```bash
# 1. 构建 WASM (release 模式，已优化)
cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm

# 2. 构建 www（产物在仓库根目录 dist/）
npm run build --workspace=@infinitecanvas/www
```

推送到 `master` 时，`.github/workflows/pages.yml` 会执行同样的两步并部署到 GitHub Pages。仓库 Settings → Pages 的 Source 需要选 GitHub Actions。

---

## 📝 脚本速查

| 命令 | 说明 | 时间 |
|------|------|------|
| `pnpm install` | 安装依赖 | ~10s |
| `pnpm build:wasm` | 构建 WASM | ~30s |
| `pnpm dev` | 启动开发服务器 | ~0.1s |
| `pnpm test:rust` | 运行 Rust 测试 | ~5s |

---

## ✅ 验证清单

构建完成后，验证以下内容：

- [ ] WASM 文件存在：`packages/core/wasm/infinitecanvas_wasm_bg.wasm`
- [ ] WASM 大小合理：~2.6MB
- [ ] JS 绑定存在：`packages/core/wasm/infinitecanvas_wasm.js`
- [ ] TypeScript 类型存在：`packages/core/wasm/infinitecanvas_wasm.d.ts`
- [ ] 开发服务器启动成功：http://localhost:3000 或 3001
- [ ] 主页可访问且无控制台错误
- [ ] 自定义图片页面可以渲染图片
- [ ] FPS 稳定在 60

---

## 🎉 完成！

项目已完成清理和验证，所有模块编译正确，输入输出目录配置无误。

**现在可以开始开发了！** 🚀

---

**更新时间**: 2025-11-28  
**维护者**: InfiniteCanvas Team







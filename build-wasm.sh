#!/bin/bash
# WASM 构建脚本 - 自动使用正确的 Rust 工具链

# 设置 rustup 工具链路径（优先级高于 Homebrew）
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"

echo "🔧 使用 Rust 工具链："
which rustc
rustc --version

echo ""
echo "🚀 开始构建 WASM..."
echo ""

export BUILD_SIGNATURE=$(date +%s)
echo "📛 Build signature: ${BUILD_SIGNATURE}"

cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ WASM 构建成功！"
    echo "📦 输出目录: packages/core/wasm/"
    ls -lh ../../packages/core/wasm/*.wasm
else
    echo ""
    echo "❌ WASM 构建失败"
    exit 1
fi







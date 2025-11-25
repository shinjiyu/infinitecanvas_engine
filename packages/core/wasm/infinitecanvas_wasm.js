/**
 * WASM Mock (JavaScript fallback)
 * 
 * 注意：这是临时的 JavaScript 实现，用于在 Rust/WASM 未编译时测试前端
 * 真正的实现需要通过 wasm-pack 构建 Rust 代码生成
 */

/**
 * InfiniteCanvas WASM 类（Mock）
 */
export class InfiniteCanvas {
  constructor() {
    console.log('[WASM Mock] InfiniteCanvas initialized');
    this.frameCount = 0;
  }

  /**
   * 渲染一帧
   */
  render() {
    this.frameCount++;
    if (this.frameCount % 60 === 0) {
      console.log(`[WASM Mock] Rendered ${this.frameCount} frames`);
    }
  }
}

/**
 * WASM 模块初始化（Mock）
 */
export default async function init() {
  console.log('[WASM Mock] Module initialized (JavaScript fallback)');
  console.warn('⚠️  使用 JavaScript Mock，非真实 WASM');
  console.warn('⚠️  需要安装 Rust 并运行: pnpm build:wasm');
  return Promise.resolve();
}


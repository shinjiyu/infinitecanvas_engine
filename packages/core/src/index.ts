/**
 * InfiniteCanvas Core - TypeScript 封装层
 * 
 * @remarks
 * 负责 WASM 初始化、事件队列管理、提供 Promise 风格 API
 * 
 * @packageDocumentation
 */

import type { InfiniteCanvas as WasmCanvas } from '../wasm/infinitecanvas_wasm';

/**
 * 无限画布主类
 * 
 * @remarks
 * 封装 WASM 引擎，提供友好的 TypeScript API
 * 
 * @example
 * ```ts
 * const canvas = new InfiniteCanvas();
 * await canvas.init('#canvas');
 * canvas.render();
 * ```
 */
export class InfiniteCanvas {
  private wasmInstance: WasmCanvas | null = null;
  private canvasElement: HTMLCanvasElement | null = null;
  private animationId: number | null = null;

  /**
   * 初始化画布
   * 
   * @param canvasOrSelector - Canvas 元素或选择器
   * @returns Promise<void>
   */
  async init(canvasOrSelector: HTMLCanvasElement | string): Promise<void> {
    // 获取 canvas 元素
    if (typeof canvasOrSelector === 'string') {
      const el = document.querySelector(canvasOrSelector);
      if (!(el instanceof HTMLCanvasElement)) {
        throw new Error(`Element ${canvasOrSelector} is not a canvas`);
      }
      this.canvasElement = el;
    } else {
      this.canvasElement = canvasOrSelector;
    }

    // 动态导入 WASM（延迟加载）
    const wasm = await import('../wasm/infinitecanvas_wasm.js');
    await wasm.default(); // 初始化 WASM
    
    // 创建 WASM 实例
    this.wasmInstance = new wasm.InfiniteCanvas();
    
    console.log('InfiniteCanvas initialized');
  }

  /**
   * 渲染一帧
   */
  render(): void {
    if (!this.wasmInstance) {
      throw new Error('Canvas not initialized. Call init() first.');
    }
    this.wasmInstance.render();
  }

  /**
   * 启动渲染循环
   */
  startRenderLoop(): void {
    const loop = () => {
      this.render();
      this.animationId = requestAnimationFrame(loop);
    };
    loop();
  }

  /**
   * 停止渲染循环
   */
  stopRenderLoop(): void {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  /**
   * 销毁实例
   */
  destroy(): void {
    this.stopRenderLoop();
    this.wasmInstance = null;
    this.canvasElement = null;
  }
}


























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
   * 导出当前场景为版本化 JSON。
   *
   * @returns 含 `version` 的场景文档
   */
  exportJson(): string {
    return this.requireWasm().export_json();
  }

  /**
   * 用版本化 JSON 替换当前场景。
   *
   * @param json - `exportJson` 生成的文本
   */
  importJson(json: string): void {
    this.requireWasm().import_json(json);
  }

  /**
   * 导出视口或选中区域的 PNG。
   *
   * @param selectionOnly - 为 true 时只导出选中元素
   * @returns RGBA PNG 字节
   */
  exportPng(selectionOnly = false): Uint8Array {
    return this.requireWasm().export_png(selectionOnly);
  }

  /**
   * 把场景导出为 SVG。
   *
   * 默认包含全部可见元素，viewBox 是它们的包围盒。有像素的图片嵌成 `<image>` data URL。
   * 只有 GPU 纹理、没有字节的图片会被跳过，文档里会有说明。文本导出为 `<text>`。
   *
   * @param selectedOnly - 为 true 时只导出选中元素
   * @returns SVG 文档
   *
   * @example
   * ```ts
   * const svg = canvas.exportSvg(false);
   * ```
   */
  exportSvg(selectedOnly = false): string {
    return this.requireWasm().export_svg(selectedOnly);
  }

  /**
   * 读取当前选择的公共样式。多选不一致时对应字段为 mixed。
   *
   * @returns WASM 返回的样式对象。没有选中可编辑元素时 `hasShape`、`hasText`、`hasImage` 都是 false。图片只参与 `opacity`
   */
  getSelectionStyle(): Record<string, unknown> {
    return this.requireWasm().get_selection_style() as Record<string, unknown>;
  }

  /**
   * 修改当前选择的样式，并记一条撤销记录。
   *
   * 只写需要改的字段。`fill` / `stroke` 传 `null` 表示清掉。
   *
   * @param patch - `fill`、`stroke`、`strokeWidth`、`opacity`、`fontSize`、`textColor`、`strokeDash`、`strokeCap`、`strokeJoin`。线型只作用于形状，文本描边不读这些字段
   * @returns 是否写入了撤销栈
   */
  setSelectionStyle(patch: Record<string, unknown>): boolean {
    return this.requireWasm().set_selection_style(patch);
  }

  /**
   * 预览样式。连续拖动滑块时反复调用，松手后再 {@link commitStylePreview}。
   *
   * @param patch - 同 {@link setSelectionStyle}
   * @returns 这次预览是否改变了元素
   */
  previewSelectionStyle(patch: Record<string, unknown>): boolean {
    return this.requireWasm().preview_selection_style(patch);
  }

  /**
   * 把进行中的样式预览收成一条撤销记录。
   *
   * @returns 是否入栈
   */
  commitStylePreview(): boolean {
    return this.requireWasm().commit_style_preview();
  }

  /**
   * 销毁实例
   */
  destroy(): void {
    this.stopRenderLoop();
    this.wasmInstance = null;
    this.canvasElement = null;
  }

  private requireWasm(): WasmCanvas {
    if (!this.wasmInstance) {
      throw new Error('Canvas not initialized. Call init() first.');
    }
    return this.wasmInstance;
  }
}



























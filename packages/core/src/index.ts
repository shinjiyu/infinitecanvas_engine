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
   * @param scale - 导出倍率，默认 `1`（与旧行为一致）。常用 `1` / `2` / `3`：
   *   像素宽高 ≈ 逻辑区域 × 视口缩放 × scale；2x 面积约 4 倍。按更高分辨率重新采样。
   * @returns RGBA PNG 字节
   */
  exportPng(selectionOnly = false, scale = 1): Uint8Array {
    return this.requireWasm().export_png(selectionOnly, scale);
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
   * 把 SVG 文本解析成场景元素，并记一条撤销记录。
   *
   * `x` / `y` 是 viewBox 原点落到的世界坐标。没有 viewBox 时是用户坐标原点。
   * 支持矩形、圆、椭圆、直线、折线、多边形、路径和文本。嵌套 `g` 的变换会压进子元素。
   *
   * @param svg - SVG 文档
   * @param x - 锚点世界坐标 x
   * @param y - 锚点世界坐标 y
   * @returns 新元素 ID
   *
   * @example
   * ```ts
   * const ids = canvas.importSvg('<svg><rect width="10" height="10"/></svg>', 24, 24);
   * ```
   */
  importSvg(svg: string, x: number, y: number): number[] {
    return Array.from(this.requireWasm().import_svg(svg, x, y));
  }

  /**
   * 读取当前选择的公共样式。多选不一致时对应字段为 mixed。
   *
   * @returns WASM 返回的样式对象。没有选中可编辑元素时 `hasShape`、`hasText`、`hasImage` 都是 false。图片参与 `opacity`、投影、`blendMode` 和图层模糊。矩形另有 `cornerRadius`（多选不一致时 `cornerRadiusMixed`）。多边形和星形另有 `sides`，星形另有 `innerRadius`。形状另有 `fillKind` / `fillStops` / `fillAngle` / `fillCenter` / `fillRadius`。形状、文本、图片还有 `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowColor` / `shadowOpacity`（多选不一致时为 mixed）和 `shadowSupported`。`blendMode` 为 `normal` / `multiply` / `screen` / `overlay` / `darken` / `lighten`。`blurRadius` 是图层模糊标准差（多选不一致时 `blurRadiusMixed`），`blurSupported` 在有形状、文本或图片时为 true。画板算形状。未写出的字段互不清掉
   */
  getSelectionStyle(): Record<string, unknown> {
    return this.requireWasm().get_selection_style() as Record<string, unknown>;
  }

  /**
   * 修改当前选择的样式，并记一条撤销记录。
   *
   * 只写需要改的字段。`fill` / `stroke` 传 `null` 表示清掉。
   *
   * @param patch - `fill`、`stroke`、`strokeWidth`、`opacity`、`fontSize`、`textColor`、`strokeDash`、`strokeCap`、`strokeJoin`、`strokeAlign`、`cornerRadius`（仅矩形）、`sides`、`innerRadius`（仅星形）、`shadowOffsetX`、`shadowOffsetY`、`shadowBlur`、`shadowColor`（只改 RGB）、`shadowOpacity`、`fillKind`（`solid` / `linear` / `radial`）、`fillStops`、`fillAngle`、`fillCenter`、`fillRadius`、`blendMode`、`blurRadius`（图层模糊，也可写成 `blur_radius`）、`startMarker` / `endMarker`（仅直线：`none` / `arrow` / `triangle`）。未写出的字段保持原值。渐变和线型只作用于形状，文本描边不读这些字段。`blendMode` 写到形状、文本、图片和画板，不清模糊
   * @returns 是否写入了撤销栈
   */
  setSelectionStyle(patch: Record<string, unknown>): boolean {
    return this.requireWasm().set_selection_style(patch);
  }

  /**
   * 采样世界坐标上最上层可见叶子的填充和描边。
   *
   * 没有命中时 `id`、`fill`、`stroke` 都是 null。图片命中时 `id` 有值，两个颜色为 null。
   * 文本的填充是文字颜色。锁定元素仍可采样；隐藏元素会跳过。
   *
   * @param x - 世界坐标 X
   * @param y - 世界坐标 Y
   * @returns `id` 与 `#rrggbbaa` 颜色。没有该通道时颜色为 null
   *
   * @example
   * ```ts
   * const sample = canvas.sampleStyleAt(12, 8);
   * if (sample.fill) canvas.applySampledStyleAt(12, 8);
   * ```
   */
  sampleStyleAt(x: number, y: number): { id: number | null; fill: string | null; stroke: string | null } {
    return this.requireWasm().sample_style_at(x, y) as {
      id: number | null;
      fill: string | null;
      stroke: string | null;
    };
  }

  /**
   * 把该点采样到的填充和描边写到当前选择，一条撤销记录。
   *
   * 空选、全部锁定、全部隐藏、点在空白，或采样没有颜色时返回 false，不入栈。
   *
   * @param x - 世界坐标 X
   * @param y - 世界坐标 Y
   * @returns 是否写入了撤销栈
   */
  applySampledStyleAt(x: number, y: number): boolean {
    return this.requireWasm().apply_sampled_style_at(x, y);
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



























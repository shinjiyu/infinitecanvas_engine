/**
 * Canvas 2D Rendering Engine (JavaScript Implementation)
 * 
 * 注意：这是功能完整的 Canvas 2D 实现，用于在 Rust/WASM 未编译时提供完整功能
 * 未来会被 Rust + WebGPU 实现替代
 */

/**
 * 场景元素基类
 */
class Element {
  constructor(id, type, x, y) {
    this.id = id;
    this.type = type;
    this.x = x;
    this.y = y;
    this.visible = true;
  }

  draw(ctx, viewport) {
    // 子类实现
  }
}

/**
 * 矩形元素
 */
class RectElement extends Element {
  constructor(id, x, y, width, height, color = '#3b82f6') {
    super(id, 'rect', x, y);
    this.width = width;
    this.height = height;
    this.color = color;
  }

  draw(ctx, viewport) {
    if (!this.visible) return;
    
    const screenPos = viewport.worldToScreen(this.x, this.y);
    const screenWidth = this.width * viewport.zoom;
    const screenHeight = this.height * viewport.zoom;
    
    ctx.fillStyle = this.color;
    ctx.fillRect(screenPos.x, screenPos.y, screenWidth, screenHeight);
    
    // 边框
    ctx.strokeStyle = '#1e40af';
    ctx.lineWidth = 2;
    ctx.strokeRect(screenPos.x, screenPos.y, screenWidth, screenHeight);
  }
}

/**
 * 圆形元素
 */
class CircleElement extends Element {
  constructor(id, x, y, radius, color = '#10b981') {
    super(id, 'circle', x, y);
    this.radius = radius;
    this.color = color;
  }

  draw(ctx, viewport) {
    if (!this.visible) return;
    
    const screenPos = viewport.worldToScreen(this.x, this.y);
    const screenRadius = this.radius * viewport.zoom;
    
    ctx.beginPath();
    ctx.arc(screenPos.x, screenPos.y, screenRadius, 0, Math.PI * 2);
    ctx.fillStyle = this.color;
    ctx.fill();
    
    ctx.strokeStyle = '#059669';
    ctx.lineWidth = 2;
    ctx.stroke();
  }
}

/**
 * 文字元素
 */
class TextElement extends Element {
  constructor(id, x, y, text, fontSize = 24, color = '#ef4444') {
    super(id, 'text', x, y);
    this.text = text;
    this.fontSize = fontSize;
    this.color = color;
  }

  draw(ctx, viewport) {
    if (!this.visible) return;
    
    const screenPos = viewport.worldToScreen(this.x, this.y);
    const screenFontSize = this.fontSize * viewport.zoom;
    
    ctx.font = `${screenFontSize}px -apple-system, sans-serif`;
    ctx.fillStyle = this.color;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(this.text, screenPos.x, screenPos.y);
  }
}

/**
 * 视口管理器
 */
class Viewport {
  constructor(canvasWidth, canvasHeight) {
    this.x = 0;           // 世界坐标偏移
    this.y = 0;
    this.zoom = 1.0;      // 缩放等级
    this.canvasWidth = canvasWidth;
    this.canvasHeight = canvasHeight;
  }

  /**
   * 世界坐标转屏幕坐标
   */
  worldToScreen(worldX, worldY) {
    return {
      x: (worldX - this.x) * this.zoom + this.canvasWidth / 2,
      y: (worldY - this.y) * this.zoom + this.canvasHeight / 2,
    };
  }

  /**
   * 屏幕坐标转世界坐标
   */
  screenToWorld(screenX, screenY) {
    return {
      x: (screenX - this.canvasWidth / 2) / this.zoom + this.x,
      y: (screenY - this.canvasHeight / 2) / this.zoom + this.y,
    };
  }

  /**
   * 平移视口
   */
  pan(dx, dy) {
    this.x += dx / this.zoom;
    this.y += dy / this.zoom;
  }

  /**
   * 缩放视口
   */
  zoomAt(factor, screenX, screenY) {
    const worldPos = this.screenToWorld(screenX, screenY);
    this.zoom *= factor;
    this.zoom = Math.max(0.1, Math.min(10, this.zoom)); // 限制范围
    const newScreenPos = this.worldToScreen(worldPos.x, worldPos.y);
    this.pan((newScreenPos.x - screenX) / this.zoom, (newScreenPos.y - screenY) / this.zoom);
  }
}

/**
 * InfiniteCanvas 渲染引擎
 */
export class InfiniteCanvas {
  constructor() {
    console.log('[Canvas2D Engine] InfiniteCanvas initialized');
    this.canvas = null;
    this.ctx = null;
    this.viewport = null;
    this.elements = [];
    this.elementIdCounter = 0;
    this.frameCount = 0;
    
    // 交互状态
    this.isDragging = false;
    this.lastMouseX = 0;
    this.lastMouseY = 0;
  }

  /**
   * 初始化 Canvas
   */
  init(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    
    // 设置高 DPI 支持
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    this.ctx.scale(dpr, dpr);
    canvas.style.width = rect.width + 'px';
    canvas.style.height = rect.height + 'px';
    
    // 初始化视口
    this.viewport = new Viewport(rect.width, rect.height);
    
    // 添加测试元素
    this.addTestElements();
    
    // 设置事件监听
    this.setupEventListeners();
    
    console.log('[Canvas2D Engine] Canvas initialized', rect.width, 'x', rect.height);
  }

  /**
   * 添加测试元素
   */
  addTestElements() {
    // 中心文字
    this.addElement(new TextElement(
      this.elementIdCounter++,
      0, -150,
      'InfiniteCanvas Engine',
      32,
      '#6366f1'
    ));
    
    // 矩形
    this.addElement(new RectElement(
      this.elementIdCounter++,
      -100, -50,
      200, 80,
      '#3b82f6'
    ));
    
    // 圆形
    this.addElement(new CircleElement(
      this.elementIdCounter++,
      -150, 80,
      50,
      '#10b981'
    ));
    
    this.addElement(new CircleElement(
      this.elementIdCounter++,
      150, 80,
      50,
      '#f59e0b'
    ));
    
    // 提示文字
    this.addElement(new TextElement(
      this.elementIdCounter++,
      0, 180,
      '🎨 拖动画布 | 滚轮缩放',
      16,
      '#94a3b8'
    ));
    
    console.log(`[Canvas2D Engine] Added ${this.elements.length} test elements`);
  }

  /**
   * 添加元素
   */
  addElement(element) {
    this.elements.push(element);
    return element.id;
  }

  /**
   * 设置事件监听
   */
  setupEventListeners() {
    // 鼠标拖动
    this.canvas.addEventListener('mousedown', (e) => {
      this.isDragging = true;
      this.lastMouseX = e.offsetX;
      this.lastMouseY = e.offsetY;
      this.canvas.style.cursor = 'grabbing';
    });

    this.canvas.addEventListener('mousemove', (e) => {
      if (this.isDragging) {
        const dx = e.offsetX - this.lastMouseX;
        const dy = e.offsetY - this.lastMouseY;
        this.viewport.pan(-dx, -dy);
        this.lastMouseX = e.offsetX;
        this.lastMouseY = e.offsetY;
      }
    });

    this.canvas.addEventListener('mouseup', () => {
      this.isDragging = false;
      this.canvas.style.cursor = 'grab';
    });

    this.canvas.addEventListener('mouseleave', () => {
      this.isDragging = false;
      this.canvas.style.cursor = 'grab';
    });

    // 滚轮缩放
    this.canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      const factor = e.deltaY < 0 ? 1.1 : 0.9;
      this.viewport.zoomAt(factor, e.offsetX, e.offsetY);
    });

    this.canvas.style.cursor = 'grab';
  }

  /**
   * 渲染一帧
   */
  render() {
    if (!this.ctx || !this.viewport) return;
    
    this.frameCount++;
    
    // 清空画布
    const rect = this.canvas.getBoundingClientRect();
    this.ctx.clearRect(0, 0, rect.width, rect.height);
    
    // 绘制背景网格
    this.drawGrid();
    
    // 绘制所有元素
    for (const element of this.elements) {
      element.draw(this.ctx, this.viewport);
    }
    
    // 绘制调试信息
    this.drawDebugInfo();
    
    if (this.frameCount % 60 === 0) {
      console.log(`[Canvas2D Engine] Rendered ${this.frameCount} frames, ${this.elements.length} elements, zoom: ${this.viewport.zoom.toFixed(2)}x`);
    }
  }

  /**
   * 绘制背景网格
   */
  drawGrid() {
    const rect = this.canvas.getBoundingClientRect();
    const gridSize = 50;
    const screenGridSize = gridSize * this.viewport.zoom;
    
    // 只在合适的缩放级别显示网格
    if (screenGridSize < 10 || screenGridSize > 200) return;
    
    this.ctx.strokeStyle = '#e5e7eb';
    this.ctx.lineWidth = 1;
    
    // 计算可见区域的网格范围
    const startWorld = this.viewport.screenToWorld(0, 0);
    const endWorld = this.viewport.screenToWorld(rect.width, rect.height);
    
    const startX = Math.floor(startWorld.x / gridSize) * gridSize;
    const endX = Math.ceil(endWorld.x / gridSize) * gridSize;
    const startY = Math.floor(startWorld.y / gridSize) * gridSize;
    const endY = Math.ceil(endWorld.y / gridSize) * gridSize;
    
    // 绘制竖线
    for (let x = startX; x <= endX; x += gridSize) {
      const screenPos = this.viewport.worldToScreen(x, 0);
      this.ctx.beginPath();
      this.ctx.moveTo(screenPos.x, 0);
      this.ctx.lineTo(screenPos.x, rect.height);
      this.ctx.stroke();
    }
    
    // 绘制横线
    for (let y = startY; y <= endY; y += gridSize) {
      const screenPos = this.viewport.worldToScreen(0, y);
      this.ctx.beginPath();
      this.ctx.moveTo(0, screenPos.y);
      this.ctx.lineTo(rect.width, screenPos.y);
      this.ctx.stroke();
    }
    
    // 绘制原点
    const origin = this.viewport.worldToScreen(0, 0);
    this.ctx.strokeStyle = '#3b82f6';
    this.ctx.lineWidth = 2;
    
    // X 轴
    this.ctx.beginPath();
    this.ctx.moveTo(0, origin.y);
    this.ctx.lineTo(rect.width, origin.y);
    this.ctx.stroke();
    
    // Y 轴
    this.ctx.beginPath();
    this.ctx.moveTo(origin.x, 0);
    this.ctx.lineTo(origin.x, rect.height);
    this.ctx.stroke();
  }

  /**
   * 绘制调试信息
   */
  drawDebugInfo() {
    const rect = this.canvas.getBoundingClientRect();
    
    this.ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
    this.ctx.fillRect(10, rect.height - 80, 200, 70);
    
    this.ctx.fillStyle = '#fff';
    this.ctx.font = '12px monospace';
    this.ctx.textAlign = 'left';
    
    this.ctx.fillText(`Elements: ${this.elements.length}`, 20, rect.height - 60);
    this.ctx.fillText(`Zoom: ${this.viewport.zoom.toFixed(2)}x`, 20, rect.height - 40);
    this.ctx.fillText(`Pos: (${this.viewport.x.toFixed(0)}, ${this.viewport.y.toFixed(0)})`, 20, rect.height - 20);
  }

  /**
   * 获取状态信息
   */
  getStats() {
    return {
      elements: this.elements.length,
      zoom: this.viewport ? this.viewport.zoom : 1.0,
      frameCount: this.frameCount,
    };
  }
}

/**
 * WASM 模块初始化
 */
export default async function init() {
  console.log('[Canvas2D Engine] Module initialized');
  console.log('✅ 使用 Canvas 2D 完整实现');
  return Promise.resolve();
}


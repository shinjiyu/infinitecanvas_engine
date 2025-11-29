/**
 * 纹理渲染测试
 */

import init, { InfiniteCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';

// 创建测试图片（程序生成的渐变图案）
function createTestImage(width, height) {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  
  // 绘制渐变背景
  const gradient = ctx.createLinearGradient(0, 0, width, height);
  gradient.addColorStop(0, '#ff0000');
  gradient.addColorStop(0.5, '#00ff00');
  gradient.addColorStop(1, '#0000ff');
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);
  
  // 添加文字
  ctx.fillStyle = 'white';
  ctx.font = 'bold 32px Arial';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText('Test Image', width / 2, height / 2);
  
  return ctx.getImageData(0, 0, width, height);
}

// 创建棋盘图案
function createCheckerboard(size, cellSize) {
  const canvas = document.createElement('canvas');
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext('2d');
  
  for (let y = 0; y < size; y += cellSize) {
    for (let x = 0; x < size; x += cellSize) {
      const isEven = (Math.floor(x / cellSize) + Math.floor(y / cellSize)) % 2 === 0;
      ctx.fillStyle = isEven ? '#ffffff' : '#cccccc';
      ctx.fillRect(x, y, cellSize, cellSize);
    }
  }
  
  return ctx.getImageData(0, 0, size, size);
}

// 状态更新
const updateStatus = (id, text, isError = false) => {
  const el = document.getElementById(id);
  if (el) {
    el.textContent = text;
    el.classList.toggle('error', isError);
  }
};

// FPS 计数器
let frameCount = 0;
let lastTime = performance.now();

const updateFPS = () => {
  frameCount++;
  const now = performance.now();
  if (now - lastTime >= 1000) {
    const fps = Math.round((frameCount * 1000) / (now - lastTime));
    updateStatus('fps', fps.toString());
    frameCount = 0;
    lastTime = now;
  }
  requestAnimationFrame(updateFPS);
};

// 主函数
async function main() {
  try {
    updateStatus('status', '初始化中...');
    updateStatus('wasm-status', '加载中...');
    
    // 初始化 WASM
    await init();
    const canvas = new InfiniteCanvas();
    
    // 初始化画布
    const htmlCanvas = document.querySelector('#canvas');
    await canvas.init(htmlCanvas);
    
    updateStatus('wasm-status', '✓ 已加载');
    updateStatus('status', '添加简单矩形...');
    
    // 只添加一个简单的矩形（应该是两个三角形 = 6个顶点）
    console.log('=== ADDING SINGLE RECTANGLE ===');
    canvas.add_rectangle(100, 100, 200, 150, '#FF6B6B', null, 0);
    console.log('=== Rectangle should create 6 vertices (2 triangles) ===');
    
    updateStatus('status', '✓ 运行中');
    
    // 启动渲染循环
    let animationId;
    const renderLoop = () => {
      canvas.render();
      animationId = requestAnimationFrame(renderLoop);
    };
    renderLoop();
    
    // 启动 FPS 计数
    updateFPS();
    
    console.log('✅ Texture rendering test started successfully');
    console.log('📊 Element count:', canvas.get_element_count());
    
  } catch (error) {
    console.error('❌ Failed to initialize:', error);
    updateStatus('status', '初始化失败', true);
    updateStatus('wasm-status', error.message, true);
  }
}

main();




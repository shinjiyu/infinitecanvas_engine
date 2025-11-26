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
    updateStatus('status', '上传纹理...');
    
    // 创建并上传测试图片
    console.log('Creating test images...');
    const testImage = createTestImage(256, 256);
    const checkerboard = createCheckerboard(128, 16);
    
    console.log('Uploading textures...');
    canvas.upload_texture('gradient', 256, 256, testImage.data);
    canvas.upload_texture('checker', 128, 128, checkerboard.data);
    
    updateStatus('status', '添加图片元素...');
    
    // 添加图片元素
    const img1 = canvas.add_image(50, 50, 200, 200, 'gradient');
    console.log('Added gradient image:', img1);
    
    const img2 = canvas.add_image(270, 50, 150, 150, 'checker');
    console.log('Added checker image:', img2);
    
    // 添加半透明图片
    const img3 = canvas.add_image(100, 270, 180, 180, 'gradient');
    canvas.set_image_alpha(img3, 0.5);
    console.log('Added semi-transparent image:', img3);
    
    // 同时添加一些矩形和圆形（混合渲染测试）
    canvas.add_rectangle(450, 100, 80, 80, '#ff6b6b', '#000000', 2);
    canvas.add_circle(550, 200, 40, '#4ecdc4', '#000000', 2);
    
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



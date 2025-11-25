/**
 * InfiniteCanvas Demo
 */

import { InfiniteCanvas } from '@infinitecanvas/core';

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
    
    // 创建画布实例
    const canvas = new InfiniteCanvas();
    
    // 初始化
    await canvas.init('#canvas');
    
    updateStatus('wasm-status', '✓ 已加载');
    updateStatus('status', '✓ 运行中');
    
    // 启动渲染循环
    canvas.startRenderLoop();
    
    // 启动 FPS 计数
    updateFPS();
    
    console.log('✅ InfiniteCanvas started successfully');
    
  } catch (error) {
    console.error('❌ Failed to initialize:', error);
    updateStatus('status', '初始化失败', true);
    updateStatus('wasm-status', error.message, true);
  }
}

main();


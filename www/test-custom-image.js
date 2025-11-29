/**
 * 自定义图片渲染测试
 * 使用 InfiniteCanvas Engine 渲染用户提供的图片
 */

import init, { InfiniteCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';

// 状态更新辅助函数
const updateStatus = (id, text, statusClass = 'loading') => {
  const el = document.getElementById(id);
  if (el) {
    el.textContent = text;
    el.className = `status ${statusClass}`;
  }
};

// 更新统计信息
const updateStat = (id, value) => {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
};

// FPS 统计（按实际渲染帧数刷新）
let frameCount = 0;
let lastTime = performance.now();
const recordFrame = () => {
  frameCount += 1;
  const now = performance.now();
  const elapsed = now - lastTime;
  if (elapsed >= 1000) {
    const fps = Math.round((frameCount * 1000) / elapsed);
    updateStat('fps', fps);
    frameCount = 0;
    lastTime = now;
  }
};

// 加载图片并转换为 ImageData（使用可见 canvas）
async function loadImageAsImageData(url, targetCanvas) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    
    img.onload = () => {
      // 使用传入的可见 canvas
      const canvas = targetCanvas;
      canvas.width = img.width;
      canvas.height = img.height;
      const ctx = canvas.getContext('2d');
      
      // 清空画布
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      
      // 绘制图片
      ctx.drawImage(img, 0, 0);
      
      console.log(`✅ Image drawn to source canvas: ${img.width}x${img.height}`);
      
      // 获取像素数据
      const imageData = ctx.getImageData(0, 0, img.width, img.height);
      
      resolve({
        imageData,
        width: img.width,
        height: img.height
      });
    };
    
    img.onerror = () => {
      reject(new Error(`Failed to load image: ${url}`));
    };
    
    img.src = url;
  });
}

// 全局变量
let canvas = null;
let currentTextureId = null;
let currentImageElement = null;
let imageWidth = 0;
let imageHeight = 0;
let animationFrameId = null;
let isAutoRendering = false;

const renderScene = () => {
  if (!canvas) return;
  canvas.render();
  recordFrame();
};

const stopRenderLoop = () => {
  if (animationFrameId !== null) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
  isAutoRendering = false;
  updateStatus('render-loop-status', '单帧模式', 'info');
  console.log('🛑 Render loop stopped');
};

const startRenderLoop = () => {
  if (isAutoRendering) return;
  isAutoRendering = true;
  updateStatus('render-loop-status', '✓ 持续渲染', 'success');
  console.log('▶️ Render loop started');
  
  const loop = () => {
    if (!isAutoRendering) return;
    renderScene();
    animationFrameId = requestAnimationFrame(loop);
  };
  
  animationFrameId = requestAnimationFrame(loop);
};

const renderSingleFrame = () => {
  stopRenderLoop();
  renderScene();
  updateStatus('render-loop-status', '已渲染 1 帧', 'success');
  console.log('🖼️ Rendered single frame');
};

// 更新图片显示
function updateImageDisplay(size, opacity) {
  if (!canvas || !currentTextureId) return;
  
  // 移除旧的图片元素（如果存在）
  if (currentImageElement !== null) {
    canvas.remove_element(currentImageElement);
  }
  
  // 计算居中位置
  const canvasEl = document.querySelector('#canvas');
  const x = (canvasEl.width - size) / 2;
  const y = (canvasEl.height - size) / 2;
  
  console.log(`📐 Image display params:
  - Canvas: ${canvasEl.width} × ${canvasEl.height}
  - Position: (${x}, ${y})
  - Size: ${size} × ${size}
  - Texture ID: ${currentTextureId}
  - Opacity: ${opacity}`);
  
  // 添加新的图片元素
  currentImageElement = canvas.add_image(
    x, 
    y, 
    size, 
    size,
    currentTextureId
  );
  
  console.log(`✅ Image element created with ID: ${currentImageElement}`);
  
  // 设置透明度
  canvas.set_image_alpha(currentImageElement, opacity);
  
  updateStat('element-count', canvas.get_element_count());
}

// 主函数
async function main() {
  try {
    console.log('🚀 Starting InfiniteCanvas Engine...');
    updateStatus('status', '初始化引擎...', 'loading');
    
    // 1. 初始化 WASM
    await init();
    updateStatus('wasm-status', '✓ 已加载', 'success');
    
    // 2. 创建 Canvas 实例
    canvas = new InfiniteCanvas();
    const webgpuCanvas = document.querySelector('#canvas');
    await canvas.init(webgpuCanvas);
    updateStatus('status', '✓ 引擎就绪', 'success');
    
    // 3. 加载图片到源 canvas
    updateStatus('image-status', '加载图片中...', 'loading');
    console.log('📷 Loading custom image...');
    
    const sourceCanvas = document.querySelector('#source-canvas');
    const { imageData, width, height } = await loadImageAsImageData('./woman.jpeg', sourceCanvas);
    imageWidth = width;
    imageHeight = height;
    
    updateStat('image-size', `${width} × ${height}`);
    updateStatus('image-status', '✓ 已加载', 'success');
    
    console.log(`✅ Image loaded: ${width}x${height}`);
    
    // 4. 上传纹理到 GPU
    console.log('🎨 Uploading texture to GPU...');
    const textureId = 'custom-woman-image';
    
    // 检查图片数据
    console.log(`📊 Image data stats:
  - Width: ${width}
  - Height: ${height}
  - Data length: ${imageData.data.length}
  - Expected length: ${width * height * 4}
  - Data type: ${imageData.data.constructor.name}
  - First 20 bytes: [${Array.from(imageData.data.slice(0, 20)).join(', ')}]
  - Sample pixel at (0,0): R=${imageData.data[0]}, G=${imageData.data[1]}, B=${imageData.data[2]}, A=${imageData.data[3]}`);
    
    canvas.upload_texture(
      textureId,
      width,
      height,
      imageData.data
    );
    currentTextureId = textureId;
    
    updateStat('texture-count', '1');
    console.log(`✅ Texture uploaded with ID: ${currentTextureId}`);
    
    // 5. 添加图片到场景
    const initialSize = 200;
    const initialOpacity = 1.0;
    updateImageDisplay(initialSize, initialOpacity);
    
    updateStatus('status', '✓ 渲染中', 'success');
    console.log('✅ Image added to scene');
    
    // 7. 设置控制器
    const sizeSlider = document.getElementById('size-slider');
    const opacitySlider = document.getElementById('opacity-slider');
    const sizeDisplay = document.getElementById('size-display');
    const opacityDisplay = document.getElementById('opacity-display');
    
    sizeSlider.addEventListener('input', (e) => {
      const size = parseInt(e.target.value);
      sizeDisplay.textContent = size;
      updateImageDisplay(size, opacitySlider.value / 100);
    });
    
    opacitySlider.addEventListener('input', (e) => {
      const opacity = e.target.value / 100;
      opacityDisplay.textContent = opacity.toFixed(2);
      updateImageDisplay(parseInt(sizeSlider.value), opacity);
    });
    
    // 8. 渲染控制（默认单帧）
    renderSingleFrame();
    
    const renderOnceBtn = document.getElementById('render-once');
    const toggleRenderBtn = document.getElementById('toggle-render');
    
    renderOnceBtn?.addEventListener('click', () => {
      renderSingleFrame();
    });
    
    toggleRenderBtn?.addEventListener('click', () => {
      if (!toggleRenderBtn) return;
      if (isAutoRendering) {
        stopRenderLoop();
        toggleRenderBtn.textContent = '开始持续渲染';
      } else {
        startRenderLoop();
        toggleRenderBtn.textContent = '停止持续渲染';
      }
    });
    
    // 9. 初始按钮状态
    if (toggleRenderBtn) {
      toggleRenderBtn.textContent = '开始持续渲染';
    }
    
    console.log('🎉 All systems ready! Rendering at 60 FPS');
    console.log('📊 Stats:');
    console.log(`  - Elements: ${canvas.get_element_count()}`);
    console.log(`  - Textures: 1`);
    console.log(`  - Image: ${width}x${height}`);
    
  } catch (error) {
    console.error('❌ Initialization failed:', error);
    updateStatus('status', `错误: ${error.message}`, 'error');
    updateStatus('wasm-status', '加载失败', 'error');
  }
}

// 启动应用
main();


// 性能测试脚本
import init, { InfiniteCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';

let wasmInstance = null;
let frameCount = 0;
let lastFpsTime = performance.now();
let lastRenderTime = 0;
let elementCount = 0;

// UI 元素
const elementsUI = {
    elementCount: document.getElementById('element-count'),
    fps: document.getElementById('fps'),
    renderTime: document.getElementById('render-time'),
    memory: document.getElementById('memory'),
    engineStatus: document.getElementById('engine-status'),
    status: document.getElementById('status')
};

/**
 * 更新 UI 统计信息
 */
function updateStats() {
    // FPS 计算
    frameCount++;
    const now = performance.now();
    const elapsed = now - lastFpsTime;
    
    if (elapsed >= 1000) {
        const fps = Math.round((frameCount * 1000) / elapsed);
        elementsUI.fps.textContent = fps;
        elementsUI.fps.className = 'stat-value ' + getFpsClass(fps);
        
        frameCount = 0;
        lastFpsTime = now;
    }
    
    // 元素数量
    elementsUI.elementCount.textContent = elementCount.toLocaleString();
    
    // 渲染时间
    const renderMs = lastRenderTime.toFixed(2);
    elementsUI.renderTime.textContent = `${renderMs} ms`;
    elementsUI.renderTime.className = 'stat-value ' + getRenderTimeClass(lastRenderTime);
    
    // 内存使用（如果浏览器支持）
    if (performance.memory) {
        const memoryMB = (performance.memory.usedJSHeapSize / 1024 / 1024).toFixed(1);
        elementsUI.memory.textContent = `${memoryMB} MB`;
        elementsUI.memory.className = 'stat-value ' + getMemoryClass(memoryMB);
    } else {
        elementsUI.memory.textContent = 'N/A';
    }
}

/**
 * 获取 FPS 对应的样式类
 */
function getFpsClass(fps) {
    if (fps >= 55) return 'good';
    if (fps >= 30) return 'warning';
    return 'bad';
}

/**
 * 获取渲染时间对应的样式类
 */
function getRenderTimeClass(ms) {
    if (ms <= 5) return 'good';
    if (ms <= 16) return 'warning';
    return 'bad';
}

/**
 * 获取内存使用对应的样式类
 */
function getMemoryClass(mb) {
    if (mb <= 50) return 'good';
    if (mb <= 100) return 'warning';
    return 'bad';
}

/**
 * 更新状态消息
 */
function updateStatus(message, isError = false) {
    elementsUI.status.textContent = message;
    elementsUI.status.style.borderLeftColor = isError ? '#ef4444' : '#3b82f6';
    elementsUI.status.style.color = isError ? '#fca5a5' : '#93c5fd';
}

/**
 * 生成随机颜色（十六进制）
 */
function randomColor() {
    const colors = [
        '#ef4444', '#f59e0b', '#10b981', '#3b82f6', '#8b5cf6',
        '#ec4899', '#06b6d4', '#14b8a6', '#f97316', '#84cc16'
    ];
    return colors[Math.floor(Math.random() * colors.length)];
}

/**
 * 生成随机位置
 */
function randomPosition(maxX, maxY) {
    return {
        x: Math.random() * maxX,
        y: Math.random() * maxY
    };
}

/**
 * 添加随机元素
 */
window.addElements = function(count) {
    if (!wasmInstance) {
        updateStatus('引擎未初始化', true);
        return;
    }
    
    const canvas = document.getElementById('canvas');
    const maxX = canvas.width;
    const maxY = canvas.height;
    
    const startTime = performance.now();
    
    for (let i = 0; i < count; i++) {
        const type = Math.random() < 0.5 ? 'rect' : 'circle';
        const pos = randomPosition(maxX, maxY);
        const color = randomColor();
        
        if (type === 'rect') {
            const width = 30 + Math.random() * 100;
            const height = 30 + Math.random() * 100;
            wasmInstance.add_rectangle(pos.x, pos.y, width, height, color, null, 0);
        } else {
            const radius = 15 + Math.random() * 50;
            wasmInstance.add_circle(pos.x, pos.y, radius, color, null, 0);
        }
    }
    
    elementCount += count;
    const elapsed = (performance.now() - startTime).toFixed(2);
    updateStatus(`✅ 添加了 ${count} 个元素 (耗时 ${elapsed}ms)`);
};

/**
 * 清空场景
 */
window.clearScene = function() {
    if (!wasmInstance) {
        updateStatus('引擎未初始化', true);
        return;
    }
    
    wasmInstance.clear();
    elementCount = 0;
    updateStatus('✅ 场景已清空');
};

/**
 * 运行压力测试
 */
window.runStressTest = async function() {
    if (!wasmInstance) {
        updateStatus('引擎未初始化', true);
        return;
    }
    
    updateStatus('🚀 压力测试进行中...');
    elementsUI.engineStatus.textContent = '压力测试中';
    elementsUI.engineStatus.className = 'stat-value warning';
    
    // 清空现有场景
    clearScene();
    
    // 分批添加 10000 个元素
    const totalElements = 10000;
    const batchSize = 1000;
    const batches = Math.ceil(totalElements / batchSize);
    
    for (let i = 0; i < batches; i++) {
        addElements(batchSize);
        await new Promise(resolve => setTimeout(resolve, 100)); // 让 UI 有时间更新
    }
    
    // 等待 3 秒观察性能
    await new Promise(resolve => setTimeout(resolve, 3000));
    
    const currentFps = parseInt(elementsUI.fps.textContent) || 0;
    const passed = currentFps >= 30;
    
    elementsUI.engineStatus.textContent = passed ? '✅ 测试通过' : '❌ 测试失败';
    elementsUI.engineStatus.className = 'stat-value ' + (passed ? 'good' : 'bad');
    
    updateStatus(
        passed 
            ? `✅ 压力测试通过！10K 元素保持 ${currentFps} FPS`
            : `❌ 压力测试失败：FPS 低于 30 (当前 ${currentFps})`,
        !passed
    );
};

/**
 * 渲染循环
 */
function renderLoop() {
    if (wasmInstance) {
        const startTime = performance.now();
        wasmInstance.render();
        lastRenderTime = performance.now() - startTime;
        updateStats();
    }
    requestAnimationFrame(renderLoop);
}

/**
 * 初始化
 */
async function initEngine() {
    try {
        elementsUI.engineStatus.textContent = '加载中...';
        updateStatus('⏳ 正在加载 WASM 模块...');
        
        // 初始化 WASM
        await init();
        
        updateStatus('⏳ 正在初始化 WebGPU 引擎...');
        
        // 创建画布实例
        wasmInstance = new InfiniteCanvas();
        const canvas = document.getElementById('canvas');
        await wasmInstance.init(canvas);
        
        elementsUI.engineStatus.textContent = '✅ WebGPU';
        elementsUI.engineStatus.className = 'stat-value good';
        
        updateStatus('✅ 引擎初始化成功！可以开始测试');
        
        // 清空初始测试场景
        wasmInstance.clear();
        elementCount = 0;
        
        // 启动渲染循环
        renderLoop();
        
        console.log('✅ Performance test ready');
        
    } catch (error) {
        console.error('❌ Initialization failed:', error);
        elementsUI.engineStatus.textContent = '❌ 初始化失败';
        elementsUI.engineStatus.className = 'stat-value bad';
        updateStatus(`❌ 初始化失败: ${error.message}`, true);
    }
}

// 启动
initEngine();


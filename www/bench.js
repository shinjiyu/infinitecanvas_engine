/**
 * 可复现的帧时间压测。
 *
 * 场景使用引擎自带的 `load_stress_scene`：填充矩形，400 列，间距 24，尺寸 16×12。
 * 不关闭脏标记，也不改视口裁剪（当前绘制路径会提交顶点缓存里的全部矩形）。
 * 平移只改视口 uniform。稳态采样结束时 `geometryUnchanged` 必须为 true。
 *
 * 查询参数（给 `scripts/bench.mjs` 用）：
 * - `autorun=1` 打开后立即跑
 * - `counts=1000,10000,100000`
 * - `warmup` 丢弃的预热帧数
 * - `frames` 计入统计的采样帧数
 *
 * 结果写到 `window.__BENCH_RESULT__`。失败写到 `window.__BENCH_ERROR__`。
 */
import init, { InfiniteCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';
import { summarizeFrames } from '../scripts/bench-stats.mjs';

const params = new URLSearchParams(location.search);

/**
 * @param {string} name
 * @param {number} fallback
 * @returns {number}
 */
function readInt(name, fallback) {
    const value = Number(params.get(name));
    return Number.isFinite(value) && value > 0 ? Math.floor(value) : fallback;
}

/**
 * @returns {number[]}
 */
function readCounts() {
    const raw = params.get('counts');
    if (!raw) {
        return [1000, 10000, 100000];
    }
    return raw
        .split(',')
        .map((part) => Number(part.trim()))
        .filter((count) => Number.isFinite(count) && count > 0);
}

/**
 * 等一帧，让浏览器有机会提交上一帧的 surface。
 *
 * @returns {Promise<void>}
 */
function nextFrame() {
    return new Promise((resolve) => {
        requestAnimationFrame(() => resolve());
    });
}

/**
 * @param {string} message
 */
function setStatus(message) {
    window.__BENCH_STATUS__ = message;
    const node = document.getElementById('status');
    if (node) {
        node.textContent = message;
    }
    console.log(`[bench] ${message}`);
}

/**
 * WASM 堆和 JS 堆。`performance.memory` 只有 Chromium 提供。
 *
 * @param {WebAssembly.Memory} memory
 * @returns {{ wasmHeapBytes: number, jsHeapUsedBytes: number | null, jsHeapTotalBytes: number | null }}
 */
function readMemory(memory) {
    const heap = performance.memory;
    return {
        wasmHeapBytes: memory.buffer.byteLength,
        jsHeapUsedBytes: heap ? heap.usedJSHeapSize : null,
        jsHeapTotalBytes: heap ? heap.totalJSHeapSize : null,
    };
}

/**
 * 采样 `frames` 次 `render()`。
 * 帧间隔取相邻两次 `render()` 返回时刻之差（含等待下一帧）。
 * 提交耗时只包住 `render()` 本身。
 *
 * @param {InfiniteCanvas} canvas
 * @param {number} frames
 * @returns {Promise<{ submitMs: number[], frameMs: number[], stats: any }>}
 */
async function sampleFrames(canvas, frames) {
    const submitMs = [];
    const frameMs = [];
    const tight = params.get('tight') === '1';
    const readback = params.get('readback') === '1';
    const source = readback ? document.getElementById('canvas') : null;
    let scratchCtx = null;
    if (readback) {
        const scratch = document.createElement('canvas');
        scratch.width = 4;
        scratch.height = 4;
        scratchCtx = scratch.getContext('2d', { willReadFrequently: true });
    }
    let previous = performance.now();
    for (let index = 0; index < frames; index += 1) {
        if (!tight) {
            await nextFrame();
        }
        const started = performance.now();
        canvas.render();
        if (readback && scratchCtx && source) {
            scratchCtx.drawImage(source, 0, 0, 4, 4);
            scratchCtx.getImageData(0, 0, 1, 1);
        }
        const ended = performance.now();
        submitMs.push(ended - started);
        if (index > 0) {
            frameMs.push(ended - previous);
        }
        previous = ended;
    }
    return {
        submitMs,
        frameMs,
        stats: canvas.get_performance_stats(),
    };
}

/**
 * @param {InfiniteCanvas} canvas
 * @param {WebAssembly.Memory} memory
 * @param {number} count
 * @param {number} warmup
 * @param {number} frames
 */
async function runScale(canvas, memory, count, warmup, frames) {
    setStatus(`加载 ${count} 个矩形`);
    const loadStarted = performance.now();
    canvas.load_stress_scene(count);
    const loadMs = performance.now() - loadStarted;

    setStatus(`${count} 冷启动帧（顶点缓存全量上传）`);
    const coldStarted = performance.now();
    canvas.render();
    const coldFrameMs = performance.now() - coldStarted;
    const coldStats = canvas.get_performance_stats();

    setStatus(`${count} 预热 ${warmup} 帧`);
    for (let index = 0; index < warmup; index += 1) {
        await nextFrame();
        canvas.render();
    }

    setStatus(`${count} 采样 ${frames} 帧`);
    const steady = await sampleFrames(canvas, frames);
    const summary = summarizeFrames(steady.frameMs, steady.submitMs);

    canvas.pan(24, 16);
    const panStarted = performance.now();
    canvas.render();
    const panFrameMs = performance.now() - panStarted;
    const panStats = canvas.get_performance_stats();

    return {
        elements: count,
        scene: 'filled-rectangles',
        loadMs,
        coldFrameMs,
        coldRegeneratedElements: coldStats.regeneratedElements,
        coldGpuBytesWritten: coldStats.gpuBytesWritten,
        warmup,
        ...summary,
        steadyGeometryUnchanged: !!steady.stats.geometryUnchanged,
        steadyRegeneratedElements: steady.stats.regeneratedElements,
        steadyVertexCount: steady.stats.vertexCount,
        steadyDrawCalls: steady.stats.drawCalls,
        panFrameMs,
        panGeometryUnchanged: !!panStats.geometryUnchanged,
        panRegeneratedElements: panStats.regeneratedElements,
        memory: readMemory(memory),
    };
}

/**
 * 读一次适配器信息，写入结果，方便和 README 里的环境对照。
 * 引擎自己的 `requestAdapter` 仍走原来的高性能偏好，这里不改渲染路径。
 *
 * @returns {Promise<Record<string, unknown>>}
 */
async function readAdapter() {
    if (!navigator.gpu) {
        return { available: false };
    }
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
    if (!adapter) {
        return { available: true, adapter: null };
    }
    const info = adapter.info || {};
    return {
        available: true,
        vendor: info.vendor || '',
        architecture: info.architecture || '',
        device: info.device || '',
        description: info.description || '',
        isFallbackAdapter: !!adapter.isFallbackAdapter,
        maxBufferSize: adapter.limits ? adapter.limits.maxBufferSize : null,
    };
}

/**
 * @param {number[]} counts
 * @returns {Promise<void>}
 */
async function runBench(counts) {
    const warmup = readInt('warmup', 30);
    const frames = readInt('frames', 120);
    window.__BENCH_RESULT__ = null;
    window.__BENCH_ERROR__ = null;
    try {
        setStatus('初始化 WASM / WebGPU');
        const wasm = await init();
        const canvas = new InfiniteCanvas();
        const element = document.getElementById('canvas');
        await canvas.init(element);
        const adapter = await readAdapter();
        const scales = [];
        for (const count of counts) {
            scales.push(await runScale(canvas, wasm.memory, count, warmup, frames));
        }
        const invalid = scales.find((scale) => !scale.steadyGeometryUnchanged || !scale.panGeometryUnchanged);
        const result = {
            ok: !invalid,
            canvas: { width: element.width, height: element.height },
            userAgent: navigator.userAgent,
            adapter,
            warmup,
            frames,
            scales,
            error: invalid
                ? `脏标记检查失败：${invalid.elements} 个元素在稳态或平移后仍重生成顶点`
                : null,
        };
        window.__BENCH_RESULT__ = result;
        renderTable(result);
        if (result.error) {
            window.__BENCH_ERROR__ = result.error;
            setStatus(result.error);
        } else {
            setStatus('完成');
        }
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        window.__BENCH_ERROR__ = message;
        setStatus(message);
        console.error(error);
    }
}

/**
 * @param {any} result
 */
function renderTable(result) {
    const body = document.getElementById('rows');
    if (!body) {
        return;
    }
    body.replaceChildren();
    for (const scale of result.scales) {
        const row = document.createElement('tr');
        const cells = [
            scale.elements,
            scale.avgFrameMs == null ? '' : scale.avgFrameMs.toFixed(2),
            scale.p95FrameMs == null ? '' : scale.p95FrameMs.toFixed(2),
            scale.avgFps == null ? '' : scale.avgFps.toFixed(1),
            scale.sustained60 ? 'yes' : 'no',
            scale.avgSubmitMs == null ? '' : scale.avgSubmitMs.toFixed(2),
            scale.memory.wasmHeapBytes,
        ];
        for (const value of cells) {
            const cell = document.createElement('td');
            cell.textContent = String(value);
            row.appendChild(cell);
        }
        body.appendChild(row);
    }
}

window.runBench = runBench;

if (params.get('autorun') === '1') {
    runBench(readCounts());
}

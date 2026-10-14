/**
 * InfiniteCanvas 首页 Demo。
 *
 * 把选择、直线、画笔、压力测试、纹理和小地图放在同一张画布上。
 * 选中元素后右侧属性面板可改填充、描边、线宽和不透明度，文本还可改字号和颜色。
 * 方向键按世界单位微调选中元素（Shift 为 10 倍）；输入框内不触发。
 * 多选时工具栏和属性面板可对齐或均匀分布，只改平移，进撤销栈。
 * 保存/打开走版本化场景 JSON，视口或选区可导出 PNG，场景或选区可导出 SVG，改动会写入 localStorage。
 * 右侧图层面板和画布选中双向同步，拖拽排序走撤销栈。
 * `]` / `[` / Ctrl+] / Ctrl+[ 前移、后移、置顶、置底，面板上有同样的四个按钮。
 * Ctrl+C / Ctrl+V / Ctrl+D 复制、粘贴、克隆，都进撤销栈。
 *
 * @example
 * ```html
 * <script type="module" src="./main.js"></script>
 * ```
 */

import init, { InfiniteCanvas as WasmCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';
import wasmUrl from '../packages/core/wasm/infinitecanvas_wasm_bg.wasm?url';
import { attachLayerPanel } from './layer-panel.js';
import {
  PASTE_STEP,
  createPasteSession,
  readTextClipboard,
  screenToWorld,
  writeTextClipboard,
} from './clipboard.js';
import { arrowNudge } from './nudge-keys.js';
import { stackOrderAction } from './stack-order-keys.js';
import { rasterizeText } from './text-raster.js';

const TEXTURE_ID = 'landing-badge';
const STORAGE_KEY = 'infinitecanvas.scene.v1';
const COLORS = ['#4090f2', '#36d399', '#f2c94c', '#f24040', '#9b59b6', '#e91e63', '#00bcd4', '#ff9800'];

/** @type {WasmCanvas | null} */
let wasm = null;
/** @type {HTMLCanvasElement | null} */
let canvasEl = null;
let textureReady = false;
let adding = false;
let frameCount = 0;
let lastFpsTime = performance.now();
let spaceHeld = false;
/** @type {{ x: number, y: number } | null} */
let lastPointer = null;
/** 指针当前是否在画布里。离开后粘贴不再对准最后一次坐标。 */
let pointerInsideCanvas = false;
const pasteSession = createPasteSession();
/** 剪贴板读写串行，避免连续 Ctrl+V 抢同一份计数。 */
let clipboardQueue = Promise.resolve();
let autosaveTimer = 0;
/** @type {{ id: bigint | number, node: HTMLTextAreaElement } | null} */
let textEditor = null;
let textEditorOpening = false;
/** @type {ReturnType<typeof attachLayerPanel> | null} */
let layerPanel = null;
/** @type {string | null} 正在拖动的属性控件，避免每帧把滑块写回去 */
let activeStyleControl = null;
const MINIMAP_W = 200;
const MINIMAP_H = 150;
const MINIMAP_PAD = 8;
const MINIMAP_REBUILD_MS = 150;
let minimapVisible = true;
let minimapEpoch = null;
let minimapBuiltAt = 0;
let minimapForce = false;
/** @type {HTMLCanvasElement | null} */
let minimapElements = null;
/** @type {ArrayLike<number> | null} */
let minimapViewport = null;
/** @type {{ offsetX: number, offsetY: number } | null} */
let minimapDrag = null;

/**
 * 浏览器没有可用的 WebGPU 时抛出，用来展示友好提示而不是原始异常。
 */
class WebGPUUnavailableError extends Error {
  /**
   * @param {string} message 给访客看的原因
   */
  constructor(message) {
    super(message);
    this.name = 'WebGPUUnavailableError';
  }
}

/**
 * 显示或隐藏 WebGPU 不可用提示。
 *
 * @param {string | null} detail 具体原因；传 null 时关掉提示
 */
function showWebGPUMessage(detail) {
  const panel = document.getElementById('webgpu-fallback');
  const detailEl = document.getElementById('webgpu-detail');
  if (!panel || !detailEl) return;
  if (detail) {
    detailEl.textContent = detail;
    panel.hidden = false;
    setStatus(detail, true);
  } else {
    detailEl.textContent = '';
    panel.hidden = true;
  }
}

/**
 * @param {string} text
 * @param {boolean} [isError]
 */
function setStatus(text, isError = false) {
  const el = document.getElementById('status');
  if (!el) return;
  el.textContent = text;
  el.className = isError ? 'err' : 'ok';
}

/**
 * @param {string} id
 * @param {string | number} value
 */
function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = String(value);
}

/**
 * 确认浏览器能提供 WebGPU 适配器。
 *
 * @returns {Promise<void>}
 */
async function assertWebGPU() {
  if (!navigator.gpu) {
    throw new WebGPUUnavailableError('未检测到 WebGPU（navigator.gpu 不存在）。');
  }
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    throw new WebGPUUnavailableError('没有可用的 GPU 适配器。请开启硬件加速后重试。');
  }
}

/**
 * 让画布像素尺寸跟上舞台，初始化前调用一次。
 * 指针坐标按显示尺寸换算，避免 CSS 拉伸后点选偏移。
 *
 * @param {HTMLCanvasElement} canvas
 */
function fitCanvas(canvas) {
  const stage = canvas.parentElement;
  if (!stage) return;
  const width = Math.max(320, Math.floor(stage.clientWidth));
  const height = Math.max(240, Math.floor(stage.clientHeight));
  canvas.width = width;
  canvas.height = height;
}

/**
 * @param {PointerEvent | WheelEvent | MouseEvent} event
 * @param {HTMLCanvasElement} canvas
 * @returns {{ x: number, y: number }}
 */
function canvasPoint(event, canvas) {
  const rect = canvas.getBoundingClientRect();
  const scaleX = rect.width > 0 ? canvas.width / rect.width : 1;
  const scaleY = rect.height > 0 ? canvas.height / rect.height : 1;
  return {
    x: (event.clientX - rect.left) * scaleX,
    y: (event.clientY - rect.top) * scaleY,
  };
}

/**
 * 生成一张渐变加棋盘的演示纹理。
 *
 * @param {number} size
 * @returns {ImageData}
 */
function createBadge(size) {
  const surface = document.createElement('canvas');
  surface.width = size;
  surface.height = size;
  const ctx = surface.getContext('2d');
  if (!ctx) {
    throw new Error('无法创建纹理画布');
  }

  const gradient = ctx.createLinearGradient(0, 0, size, size);
  gradient.addColorStop(0, '#4090f2');
  gradient.addColorStop(0.55, '#7c5cff');
  gradient.addColorStop(1, '#36d399');
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, size, size);

  const cell = 16;
  for (let y = 0; y < size; y += cell) {
    for (let x = 0; x < size; x += cell) {
      const even = (Math.floor(x / cell) + Math.floor(y / cell)) % 2 === 0;
      if (!even) continue;
      ctx.fillStyle = 'rgba(255,255,255,0.16)';
      ctx.fillRect(x, y, cell, cell);
    }
  }

  ctx.fillStyle = '#ffffff';
  ctx.font = 'bold 42px sans-serif';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText('Texture', size / 2, size / 2 - 8);
  ctx.font = '18px sans-serif';
  ctx.fillText('WebGPU', size / 2, size / 2 + 28);
  return ctx.getImageData(0, 0, size, size);
}

/**
 * 上传演示纹理。重复调用是安全的。
 *
 * @param {WasmCanvas} engine
 */
function ensureTexture(engine) {
  if (textureReady) return;
  const image = createBadge(256);
  engine.upload_texture(TEXTURE_ID, image.width, image.height, image.data);
  textureReady = true;
}

/**
 * @param {WasmCanvas} engine
 * @param {number} width
 * @param {number} height
 */
function addRandomShape(engine, width, height) {
  const x = 40 + Math.random() * Math.max(40, width - 80);
  const y = 40 + Math.random() * Math.max(40, height - 80);
  const size = 16 + Math.random() * 48;
  const color = COLORS[Math.floor(Math.random() * COLORS.length)];
  if (Math.random() > 0.5) {
    engine.add_rectangle(x, y, size, size * 0.7, color, null, 0);
  } else {
    engine.add_circle(x, y, size / 2, color, null, 0);
  }
}

/**
 * 放入一组可框选的图形和一张纹理，方便第一次打开就能操作。
 *
 * @param {WasmCanvas} engine
 * @param {HTMLCanvasElement} canvas
 */
function loadSample(engine, canvas) {
  engine.clear_scene();
  const w = canvas.width;
  const h = canvas.height;
  const blue = engine.add_rectangle(w * 0.32, h * 0.38, 200, 130, '#4090f2', '#d6e6ff', 2);
  const gold = engine.add_rectangle(w * 0.52, h * 0.58, 150, 96, '#f2c94c', null, 0);
  engine.group_elements([blue, gold]);
  engine.add_circle(w * 0.66, h * 0.34, 58, '#36d399', '#0f5c40', 3);
  engine.add_circle(w * 0.76, h * 0.66, 40, '#f24040', null, 0);
  engine.add_rectangle(w * 0.22, h * 0.7, 90, 70, '#9b59b6', null, 0);
  ensureTexture(engine);
  engine.add_image(w * 0.08, h * 0.12, 168, 168, TEXTURE_ID);
  engine.add_line(w * 0.12, h * 0.48, w * 0.4, h * 0.22, '#111827', 3, true);
  engine.add_path(
    new Float32Array([w * 0.18, h * 0.84, w * 0.3, h * 0.64, w * 0.44, h * 0.82]),
    '#7c5cff',
    3,
  );
  engine.add_text(w * 0.12, h * 0.18, 'Hello 你好', 36, '#f4f7fb', 'left');
}

/**
 * @param {unknown} error
 * @returns {string}
 */
function errorText(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * 把当前场景写入 localStorage。失败时只更新状态，不打断绘制。
 *
 * @param {WasmCanvas} engine
 */
function persistLocal(engine) {
  try {
    localStorage.setItem(STORAGE_KEY, engine.export_json());
  } catch (error) {
    setStatus(`自动保存失败：${errorText(error)}`, true);
  }
}

/**
 * 合并连续编辑，稍后再写入 localStorage。
 *
 * @param {WasmCanvas} engine
 */
function scheduleAutosave(engine) {
  window.clearTimeout(autosaveTimer);
  autosaveTimer = window.setTimeout(() => persistLocal(engine), 600);
}

/**
 * @param {string} filename
 * @param {Blob} blob
 */
function downloadBlob(filename, blob) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

/**
 * 下载场景 JSON，并同步一份到 localStorage。
 *
 * @param {WasmCanvas} engine
 */
function saveSceneFile(engine) {
  try {
    const json = engine.export_json();
    downloadBlob('scene.json', new Blob([json], { type: 'application/json' }));
    persistLocal(engine);
    setStatus('已保存场景 JSON');
  } catch (error) {
    setStatus(`保存失败：${errorText(error)}`, true);
  }
}

/**
 * @param {WasmCanvas} engine
 * @param {boolean} selectionOnly
 */
function exportPng(engine, selectionOnly) {
  try {
    const bytes = engine.export_png(selectionOnly);
    const name = selectionOnly ? 'selection.png' : 'viewport.png';
    downloadBlob(name, new Blob([bytes], { type: 'image/png' }));
    setStatus(selectionOnly ? '已导出选区 PNG' : '已导出视口 PNG');
  } catch (error) {
    setStatus(errorText(error), true);
  }
}

/**
 * 下载场景或选中元素的 SVG。
 *
 * 全部导出时 viewBox 是可见元素的包围盒；选区导出只包含当前选中元素。
 *
 * @param {WasmCanvas} engine
 * @param {boolean} selectedOnly
 */
function exportSvg(engine, selectedOnly) {
  try {
    const svg = engine.export_svg(selectedOnly);
    if (selectedOnly && svg.includes('nothing selected')) {
      setStatus('没有选中元素', true);
      return;
    }
    const name = selectedOnly ? 'selection.svg' : 'scene.svg';
    downloadBlob(name, new Blob([svg], { type: 'image/svg+xml' }));
    setStatus(selectedOnly ? '已导出选区 SVG' : '已导出 SVG');
  } catch (error) {
    setStatus(errorText(error), true);
  }
}

/**
 * @param {WasmCanvas} engine
 * @returns {boolean} 是否恢复了本地场景
 */
function restoreLocal(engine) {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (!saved) return false;
  engine.import_json(saved);
  return true;
}

/**
 * @param {WasmCanvas} engine
 * @param {HTMLCanvasElement} canvas
 * @param {number} count
 * @returns {Promise<void>}
 */
async function addElements(engine, canvas, count) {
  if (adding) return;
  adding = true;
  setStatus(`正在添加 ${count} 个元素…`);
  const buttons = document.querySelectorAll('[data-count]');
  buttons.forEach((button) => {
    button.disabled = true;
  });
  try {
    for (let i = 0; i < count; i += 1) {
      addRandomShape(engine, canvas.width, canvas.height);
      if (i > 0 && i % 250 === 0) {
        await new Promise((resolve) => {
          requestAnimationFrame(resolve);
        });
      }
    }
    setStatus(`已添加 ${count} 个元素`);
    scheduleAutosave(engine);
  } finally {
    adding = false;
    buttons.forEach((button) => {
      button.disabled = false;
    });
  }
}

/**
 * @param {string} toolName
 */
function setTool(toolName) {
  if (!wasm) return;
  wasm.set_tool(toolName);
  document.querySelectorAll('.tool').forEach((button) => {
    button.classList.toggle('active', button.dataset.tool === toolName);
  });
  setText('current-tool', wasm.get_current_tool());
}

/**
 * 元素外框的离屏层。相机变化时不重画。
 *
 * @returns {HTMLCanvasElement}
 */
function minimapElementLayer() {
  if (!minimapElements) {
    minimapElements = document.createElement('canvas');
    minimapElements.width = MINIMAP_W;
    minimapElements.height = MINIMAP_H;
  }
  return minimapElements;
}

/**
 * 视口框是否盖住这个小地图像素。框太小时扩大命中，方便拖拽。
 *
 * @param {number} x
 * @param {number} y
 * @param {ArrayLike<number> | null} viewport
 * @returns {boolean}
 */
function hitsMinimapViewport(x, y, viewport) {
  if (!viewport || viewport.length < 4) return false;
  const min = 12;
  let left = viewport[0];
  let top = viewport[1];
  let width = viewport[2];
  let height = viewport[3];
  if (width < min) {
    left -= (min - width) / 2;
    width = min;
  }
  if (height < min) {
    top -= (min - height) / 2;
    height = min;
  }
  return x >= left && x <= left + width && y >= top && y <= top + height;
}

/**
 * @param {boolean} visible
 */
function setMinimapVisible(visible) {
  minimapVisible = visible;
  const node = document.getElementById('minimap');
  if (node) node.hidden = !visible;
  document.getElementById('minimap-btn')?.classList.toggle('active', visible);
  if (visible) minimapForce = true;
}

/**
 * 几何版本变化时重画元素外框。拖动中最多约每 150ms 一次。
 *
 * @param {WasmCanvas} engine
 */
function rebuildMinimapElements(engine) {
  const layer = minimapElementLayer();
  const ctx = layer.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, MINIMAP_W, MINIMAP_H);
  const rects = engine.minimap_element_rects(MINIMAP_W, MINIMAP_H, MINIMAP_PAD);
  ctx.fillStyle = 'rgba(126, 182, 255, 0.92)';
  for (let i = 0; i < rects.length; i += 4) {
    ctx.fillRect(rects[i], rects[i + 1], rects[i + 2], rects[i + 3]);
  }
}

/**
 * 贴上元素外框，并画出当前视口。主画布平移/缩放只走这一条。
 *
 * @param {WasmCanvas} engine
 */
function paintMinimap(engine) {
  const canvas = document.getElementById('minimap');
  if (!(canvas instanceof HTMLCanvasElement) || !minimapVisible) return;
  const epoch = engine.minimap_epoch();
  const now = performance.now();
  if (epoch !== minimapEpoch && (minimapForce || minimapEpoch == null || now - minimapBuiltAt >= MINIMAP_REBUILD_MS)) {
    rebuildMinimapElements(engine);
    minimapEpoch = epoch;
    minimapBuiltAt = now;
    minimapForce = false;
  } else if (epoch === minimapEpoch) {
    minimapForce = false;
  }
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, MINIMAP_W, MINIMAP_H);
  ctx.drawImage(minimapElementLayer(), 0, 0);
  const viewport = engine.minimap_viewport_rect(MINIMAP_W, MINIMAP_H, MINIMAP_PAD);
  minimapViewport = viewport;
  const x = viewport[0];
  const y = viewport[1];
  const width = Math.max(viewport[2], 1);
  const height = Math.max(viewport[3], 1);
  ctx.fillStyle = 'rgba(242, 201, 76, 0.16)';
  ctx.fillRect(x, y, width, height);
  ctx.strokeStyle = '#f2c94c';
  ctx.lineWidth = 1.5;
  ctx.strokeRect(x + 0.5, y + 0.5, width, height);
}

/**
 * 点击把相机中心移到指针下的世界点；按在视口框上则记下相对中心的偏移。
 *
 * @param {WasmCanvas} engine
 * @param {{ x: number, y: number }} local
 */
function beginMinimapPointer(engine, local) {
  if (hitsMinimapViewport(local.x, local.y, minimapViewport)) {
    const viewport = minimapViewport;
    minimapDrag = {
      offsetX: local.x - (viewport[0] + viewport[2] / 2),
      offsetY: local.y - (viewport[1] + viewport[3] / 2),
    };
    return;
  }
  const world = engine.minimap_to_world(local.x, local.y, MINIMAP_W, MINIMAP_H, MINIMAP_PAD);
  engine.set_camera_center(world[0], world[1]);
  minimapDrag = { offsetX: 0, offsetY: 0 };
  paintMinimap(engine);
}

/**
 * 串行执行剪贴板任务，连续按键时位移计数不会交叉。
 *
 * @param {() => Promise<void> | void} task
 */
function enqueueClipboard(task) {
  clipboardQueue = clipboardQueue.then(task, task);
}

/**
 * Ctrl+C：把选中元素写成 version 1 JSON。系统剪贴板失败时留在内存里。
 *
 * @param {WasmCanvas} engine
 * @returns {Promise<void>}
 */
async function copySelection(engine) {
  let json = '';
  try {
    json = engine.copy_selection_json();
  } catch (error) {
    setStatus(`复制失败：${errorText(error)}`, true);
    return;
  }
  if (!json) return;
  pasteSession.arm(json);
  await writeTextClipboard(json);
  setStatus('已复制');
}

/**
 * Ctrl+V：解析剪贴板并插入。指针在画布内时以指针为中心，否则从原位置偏移。
 *
 * @param {WasmCanvas} engine
 * @returns {Promise<void>}
 */
async function pasteSelection(engine) {
  const json = await readTextClipboard();
  if (!json || !json.trim()) return;
  const inside = pointerInsideCanvas && lastPointer && canvasEl;
  const placement = pasteSession.nextPlacement(json, Boolean(inside));
  /** @type {number | undefined} */
  let focusX;
  /** @type {number | undefined} */
  let focusY;
  if (inside && canvasEl && lastPointer) {
    const world = screenToWorld(
      lastPointer,
      { width: canvasEl.width, height: canvasEl.height },
      { x: engine.get_viewport_center_x(), y: engine.get_viewport_center_y() },
      engine.get_zoom(),
    );
    focusX = world.x;
    focusY = world.y;
  }
  try {
    const ids = engine.paste_elements(json, placement.dx, placement.dy, focusX, focusY);
    const count = ids?.length ?? 0;
    if (count === 0) {
      setStatus('剪贴板里没有可粘贴的元素', true);
      return;
    }
    scheduleAutosave(engine);
    setStatus(`已粘贴 ${count} 个`);
  } catch (error) {
    setStatus(`粘贴失败：${errorText(error)}`, true);
  }
}

/**
 * Ctrl+D：按固定步长原地克隆，新元素成为选中。
 *
 * @param {WasmCanvas} engine
 */
function duplicateSelection(engine) {
  const ids = engine.duplicate_selection(PASTE_STEP, PASTE_STEP);
  const count = ids?.length ?? 0;
  if (count === 0) return;
  scheduleAutosave(engine);
  setStatus(`已克隆 ${count} 个`);
}

/**
 * @param {HTMLCanvasElement} canvas
 * @param {WasmCanvas} engine
 */
function bindMinimap(canvas, engine) {
  canvas.addEventListener('pointerdown', (event) => {
    if (!minimapVisible) return;
    event.preventDefault();
    event.stopPropagation();
    beginMinimapPointer(engine, canvasPoint(event, canvas));
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener('pointermove', (event) => {
    if (!minimapDrag) return;
    event.preventDefault();
    const local = canvasPoint(event, canvas);
    const world = engine.minimap_to_world(
      local.x - minimapDrag.offsetX,
      local.y - minimapDrag.offsetY,
      MINIMAP_W,
      MINIMAP_H,
      MINIMAP_PAD,
    );
    engine.set_camera_center(world[0], world[1]);
    paintMinimap(engine);
  });
  const endDrag = (event) => {
    if (!minimapDrag) return;
    minimapDrag = null;
    minimapForce = true;
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
  };
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', endDrag);
  canvas.addEventListener('wheel', (event) => {
    event.preventDefault();
    event.stopPropagation();
  }, { passive: false });
  document.getElementById('minimap-btn')?.addEventListener('click', () => {
    setMinimapVisible(!minimapVisible);
  });
}

/**
 * @param {HTMLCanvasElement} canvas
 * @param {WasmCanvas} engine
 */
/**
 * 结束一次属性拖动，并把前后差值收成一条撤销记录。
 *
 * @param {WasmCanvas} engine
 */
function finishStyleEdit(engine) {
  activeStyleControl = null;
  engine.commit_style_preview();
  scheduleAutosave(engine);
  syncPropertyPanel(engine);
}

/**
 * 把 `#rrggbbaa` 收成颜色输入框要的 `#rrggbb`。
 *
 * @param {unknown} value
 * @returns {string | null}
 */
function cssHex(value) {
  if (typeof value !== 'string' || value.length < 7) return null;
  return value.slice(0, 7);
}

/**
 * @param {string} colorId
 * @param {string | null} checkId
 * @param {string} mixedId
 * @param {string | null} value
 * @param {boolean} mixed
 * @param {boolean} none
 * @param {boolean} supported
 */
function syncColorRow(colorId, checkId, mixedId, value, mixed, none, supported) {
  const mixedEl = document.getElementById(mixedId);
  if (mixedEl) mixedEl.hidden = !mixed;
  const color = document.getElementById(colorId);
  if (color instanceof HTMLInputElement && activeStyleControl !== colorId) {
    const hex = cssHex(value);
    if (hex) color.value = hex;
    color.disabled = !supported || Boolean(none);
  }
  if (!checkId) return;
  const check = document.getElementById(checkId);
  if (check instanceof HTMLInputElement && activeStyleControl !== checkId) {
    check.indeterminate = Boolean(mixed);
    check.checked = Boolean(supported) && !none && !mixed;
    check.disabled = !supported;
  }
}

/**
 * @param {string} id
 * @param {string} labelId
 * @param {number | null} value
 * @param {boolean} mixed
 * @param {(value: number) => string} format
 */
function syncRange(id, labelId, value, mixed, format) {
  const mixedEl = document.getElementById(`${id}-mixed`);
  if (mixedEl) mixedEl.hidden = !mixed;
  const input = document.getElementById(id);
  if (!(input instanceof HTMLInputElement) || activeStyleControl === id) return;
  if (typeof value === 'number' && Number.isFinite(value)) {
    input.value = String(value);
    setText(labelId, format(value));
  } else if (mixed) {
    setText(labelId, '多种');
  }
}

/**
 * 按当前选择显示或隐藏属性面板，并把公共样式填进控件。
 * 多选不一致时标成「多种」，下一次编辑会写到全部选中元素。
 *
 * @param {WasmCanvas} engine
 */
function syncPropertyPanel(engine) {
  const panel = document.getElementById('property-panel');
  if (!panel || typeof engine.get_selection_style !== 'function') return;
  const style = engine.get_selection_style();
  const visible = Boolean(style && (style.hasShape || style.hasText));
  panel.hidden = !visible;
  if (!visible || !style) return;

  setText('prop-count', style.count ?? 0);
  const fillRow = document.getElementById('prop-fill-row');
  if (fillRow) fillRow.hidden = !style.hasShape;
  syncColorRow(
    'prop-fill',
    'prop-fill-enabled',
    'prop-fill-mixed',
    style.fill,
    style.fillMixed,
    style.fillNone,
    style.fillSupported,
  );
  syncColorRow(
    'prop-stroke',
    'prop-stroke-enabled',
    'prop-stroke-mixed',
    style.stroke,
    style.strokeMixed,
    style.strokeNone,
    style.strokeSupported,
  );
  syncRange(
    'prop-stroke-width',
    'prop-stroke-width-label',
    style.strokeWidth,
    style.strokeWidthMixed,
    (value) => String(value),
  );
  syncRange(
    'prop-opacity',
    'prop-opacity-label',
    style.opacity,
    style.opacityMixed,
    (value) => `${Math.round(value * 100)}%`,
  );

  const textFields = document.getElementById('prop-text-fields');
  if (textFields) textFields.hidden = !style.hasText;
  syncRange(
    'prop-font-size',
    'prop-font-size-label',
    style.fontSize,
    style.fontSizeMixed,
    (value) => String(Math.round(value)),
  );
  syncColorRow(
    'prop-text-color',
    null,
    'prop-text-color-mixed',
    style.textColor,
    style.textColorMixed,
    false,
    style.textColorSupported,
  );
}

/**
 * 绑定右侧属性面板。拖动过程中只预览，松手后记一条历史。
 *
 * @param {WasmCanvas} engine
 */
/**
 * 多选时才允许对齐和分布。单选或空选保持禁用。
 *
 * @param {WasmCanvas} engine
 */
function syncAlignControls(engine) {
  const enabled = engine.get_selection_count() >= 2;
  document.querySelectorAll('[data-align], [data-distribute]').forEach((button) => {
    if (button instanceof HTMLButtonElement) button.disabled = !enabled;
  });
}

/**
 * 对齐和分布各记一条撤销记录。拖拽吸附不受这些按钮影响。
 *
 * @param {WasmCanvas} engine
 */
function bindAlignControls(engine) {
  document.querySelectorAll('[data-align]').forEach((button) => {
    button.addEventListener('click', () => {
      const mode = button.getAttribute('data-align');
      if (!mode || typeof engine.align_selection !== 'function') return;
      if (engine.align_selection(mode)) {
        scheduleAutosave(engine);
        setStatus('已对齐');
      }
    });
  });
  document.querySelectorAll('[data-distribute]').forEach((button) => {
    button.addEventListener('click', () => {
      const axis = button.getAttribute('data-distribute');
      if (!axis || typeof engine.distribute_selection !== 'function') return;
      if (engine.distribute_selection(axis)) {
        scheduleAutosave(engine);
        setStatus('已分布');
      }
    });
  });
}

function bindPropertyPanel(engine) {
  const bindRange = (id, key, labelId, format) => {
    const input = document.getElementById(id);
    if (!(input instanceof HTMLInputElement)) return;
    input.addEventListener('input', () => {
      const value = Number(input.value);
      setText(labelId, format(value));
      activeStyleControl = id;
      engine.preview_selection_style({ [key]: value });
    });
    input.addEventListener('change', () => finishStyleEdit(engine));
  };
  bindRange('prop-stroke-width', 'strokeWidth', 'prop-stroke-width-label', (value) => String(value));
  bindRange('prop-opacity', 'opacity', 'prop-opacity-label', (value) => `${Math.round(value * 100)}%`);
  bindRange('prop-font-size', 'fontSize', 'prop-font-size-label', (value) => String(Math.round(value)));

  const bindColor = (id, key) => {
    const input = document.getElementById(id);
    if (!(input instanceof HTMLInputElement)) return;
    input.addEventListener('input', () => {
      activeStyleControl = id;
      engine.preview_selection_style({ [key]: input.value });
    });
    input.addEventListener('change', () => finishStyleEdit(engine));
  };
  bindColor('prop-fill', 'fill');
  bindColor('prop-stroke', 'stroke');
  bindColor('prop-text-color', 'textColor');

  document.getElementById('prop-fill-enabled')?.addEventListener('change', (event) => {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    const color = document.getElementById('prop-fill');
    const hex = color instanceof HTMLInputElement ? color.value : '#000000';
    engine.set_selection_style(input.checked ? { fill: hex } : { fill: null });
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
  });

  document.getElementById('prop-stroke-enabled')?.addEventListener('change', (event) => {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    const color = document.getElementById('prop-stroke');
    const hex = color instanceof HTMLInputElement ? color.value : '#000000';
    const widthInput = document.getElementById('prop-stroke-width');
    const width = widthInput instanceof HTMLInputElement ? Number(widthInput.value) : 1;
    if (input.checked) {
      /** @type {Record<string, string | number | null>} */
      const patch = { stroke: hex };
      if (!Number.isFinite(width) || width <= 0) patch.strokeWidth = 1;
      engine.set_selection_style(patch);
    } else {
      engine.set_selection_style({ stroke: null, strokeWidth: 0 });
    }
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
  });
}

function bindInput(canvas, engine) {
  bindPropertyPanel(engine);
  bindAlignControls(engine);
  canvas.addEventListener('pointerdown', (event) => {
    const point = canvasPoint(event, canvas);
    engine.handle_pointer_event(
      'down',
      event.pointerId,
      point.x,
      point.y,
      event.movementX,
      event.movementY,
      event.button,
      event.buttons,
      event.shiftKey,
      event.ctrlKey,
      event.altKey,
      event.metaKey,
    );
    canvas.setPointerCapture(event.pointerId);
  });

  canvas.addEventListener('pointerenter', () => {
    pointerInsideCanvas = true;
  });
  canvas.addEventListener('pointerleave', () => {
    pointerInsideCanvas = false;
  });

  canvas.addEventListener('pointermove', (event) => {
    const point = canvasPoint(event, canvas);
    lastPointer = point;
    pointerInsideCanvas = true;
    engine.handle_pointer_event(
      'move',
      event.pointerId,
      point.x,
      point.y,
      event.movementX,
      event.movementY,
      event.button,
      event.buttons,
      event.shiftKey,
      event.ctrlKey,
      event.altKey,
      event.metaKey,
    );
  });

  canvas.addEventListener('pointerup', (event) => {
    const point = canvasPoint(event, canvas);
    engine.handle_pointer_event(
      'up',
      event.pointerId,
      point.x,
      point.y,
      event.movementX,
      event.movementY,
      event.button,
      event.buttons,
      event.shiftKey,
      event.ctrlKey,
      event.altKey,
      event.metaKey,
    );
    if (canvas.hasPointerCapture(event.pointerId)) {
      canvas.releasePointerCapture(event.pointerId);
    }
    scheduleAutosave(engine);
    minimapForce = true;
  });

  canvas.addEventListener('dblclick', (event) => {
    const point = canvasPoint(event, canvas);
    const info = engine.begin_text_edit_at(point.x, point.y);
    if (info) openTextEditor(engine, info);
  });

  canvas.addEventListener('contextmenu', (event) => event.preventDefault());

  const editor = document.getElementById('text-editor');
  if (editor instanceof HTMLTextAreaElement) {
    editor.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        closeTextEditor(engine, false);
      } else if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        closeTextEditor(engine, true);
      }
    });
    editor.addEventListener('blur', () => {
      if (textEditorOpening) return;
      closeTextEditor(engine, true);
    });
  }

  canvas.addEventListener('wheel', (event) => {
    event.preventDefault();
    const point = canvasPoint(event, canvas);
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    engine.zoom_at(point.x, point.y, factor);
  }, { passive: false });

  document.querySelectorAll('.tool').forEach((button) => {
    button.addEventListener('click', () => {
      const toolName = button.dataset.tool;
      if (toolName) setTool(toolName);
    });
  });

  document.getElementById('undo-btn')?.addEventListener('click', () => {
    activeStyleControl = null;
    engine.undo();
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
  });
  document.getElementById('redo-btn')?.addEventListener('click', () => {
    activeStyleControl = null;
    engine.redo();
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
  });
  document.getElementById('sample-btn')?.addEventListener('click', () => {
    loadSample(engine, canvas);
    scheduleAutosave(engine);
    setStatus('已载入示例');
  });
  document.getElementById('texture-btn')?.addEventListener('click', () => {
    ensureTexture(engine);
    const x = 24 + Math.random() * Math.max(24, canvas.width - 200);
    const y = 24 + Math.random() * Math.max(24, canvas.height - 200);
    engine.add_image(x, y, 160, 160, TEXTURE_ID);
    scheduleAutosave(engine);
    setStatus('已添加纹理');
  });
  document.getElementById('clear-btn')?.addEventListener('click', () => {
    engine.clear_scene();
    scheduleAutosave(engine);
    setStatus('场景已清空');
  });
  document.getElementById('clear-selection-btn')?.addEventListener('click', () => {
    engine.clear_selection();
    scheduleAutosave(engine);
  });
  document.getElementById('save-btn')?.addEventListener('click', () => saveSceneFile(engine));
  document.getElementById('open-btn')?.addEventListener('click', () => {
    document.getElementById('open-file')?.click();
  });
  document.getElementById('open-file')?.addEventListener('change', async (event) => {
    const input = event.target;
    if (!(input instanceof HTMLInputElement)) return;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      engine.import_json(await file.text());
      persistLocal(engine);
      setStatus(`已打开 ${file.name}`);
    } catch (error) {
      setStatus(`打开失败：${errorText(error)}`, true);
    }
  });
  document.getElementById('png-btn')?.addEventListener('click', () => exportPng(engine, false));
  document.getElementById('png-selection-btn')?.addEventListener('click', () => exportPng(engine, true));
  document.getElementById('svg-btn')?.addEventListener('click', () => exportSvg(engine, false));
  document.getElementById('svg-selection-btn')?.addEventListener('click', () => exportSvg(engine, true));
  document.getElementById('snap-btn')?.addEventListener('click', () => {
    engine.set_snap_enabled(!engine.is_snap_enabled());
    syncSnapControls(engine);
    drawAlignmentGuides(engine);
  });
  document.getElementById('snap-threshold')?.addEventListener('input', (event) => {
    const input = event.currentTarget;
    if (!(input instanceof HTMLInputElement)) return;
    engine.set_snap_threshold(Number(input.value));
    syncSnapControls(engine);
  });
  document.querySelectorAll('[data-count]').forEach((button) => {
    button.addEventListener('click', () => {
      const count = Number(button.dataset.count);
      if (count > 0) addElements(engine, canvas, count);
    });
  });
  for (const [id, action] of [
    ['layer-front-btn', 'front'],
    ['layer-back-btn', 'back'],
    ['layer-forward-btn', 'forward'],
    ['layer-backward-btn', 'backward'],
  ]) {
    document.getElementById(id)?.addEventListener('click', () => {
      if (engine.reorder_selection(action)) {
        scheduleAutosave(engine);
        layerPanel?.sync();
      }
    });
  }

  document.addEventListener('keydown', (event) => {
    const inField = event.target instanceof HTMLElement
      && (event.target.tagName === 'INPUT' || event.target.tagName === 'TEXTAREA');
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z' && !event.shiftKey) {
      event.preventDefault();
      activeStyleControl = null;
      engine.undo();
      scheduleAutosave(engine);
      syncPropertyPanel(engine);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === 'y' || (event.key.toLowerCase() === 'z' && event.shiftKey))) {
      event.preventDefault();
      activeStyleControl = null;
      engine.redo();
      scheduleAutosave(engine);
      syncPropertyPanel(engine);
      return;
    }
    if (inField) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
      event.preventDefault();
      saveSceneFile(engine);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'c') {
      event.preventDefault();
      enqueueClipboard(() => copySelection(engine));
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'v') {
      event.preventDefault();
      enqueueClipboard(() => pasteSelection(engine));
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'd') {
      event.preventDefault();
      duplicateSelection(engine);
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'o') {
      event.preventDefault();
      document.getElementById('open-file')?.click();
      return;
    }
    const stackAction = stackOrderAction(event);
    if (stackAction) {
      event.preventDefault();
      if (engine.reorder_selection(stackAction)) {
        scheduleAutosave(engine);
        layerPanel?.sync();
      }
      return;
    }
    if (event.key === ' ' && !spaceHeld) {
      event.preventDefault();
      spaceHeld = true;
      engine.push_hand_tool();
      return;
    }
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    const nudge = arrowNudge(event);
    if (nudge) {
      event.preventDefault();
      if (engine.nudge_selection(nudge.direction, nudge.shift)) {
        scheduleAutosave(engine);
        syncPropertyPanel(engine);
      }
      return;
    }
    switch (event.key.toLowerCase()) {
      case 'v':
        setTool('select');
        break;
      case 'l':
        setTool('line');
        break;
      case 'p':
        setTool('pen');
        break;
      case 'r':
        setTool('rectangle');
        break;
      case 'o':
        setTool('circle');
        break;
      case 'h':
        setTool('pan');
        break;
      case 't':
        setTool('text');
        break;
      case 'm':
        setMinimapVisible(!minimapVisible);
        break;
      default:
        break;
    }
  });

  document.addEventListener('keyup', (event) => {
    if (event.key === ' ' && spaceHeld) {
      spaceHeld = false;
      engine.pop_temporary_tool();
    }
  });
}

/**
 * 刷新 FPS 和场景统计。按渲染帧计数，约每 500ms 更新一次帧率。
 *
 * @param {WasmCanvas} engine
 */
function updateHud(engine) {
  frameCount += 1;
  const now = performance.now();
  const elapsed = now - lastFpsTime;
  if (elapsed >= 500) {
    const fps = Math.round((frameCount * 1000) / elapsed);
    const fpsEl = document.getElementById('fps');
    if (fpsEl) {
      fpsEl.textContent = String(fps);
      fpsEl.className = fps < 30 ? 'bad' : fps < 50 ? 'warn' : '';
    }
    frameCount = 0;
    lastFpsTime = now;
  }

  const stats = engine.get_performance_stats();
  setText('element-count', stats.elementCount?.toLocaleString?.() ?? engine.get_element_count());
  setText('selection-count', engine.get_selection_count());
  setText('vertex-count', stats.vertexCount?.toLocaleString?.() ?? engine.get_last_vertex_count());
  setText('draw-calls', stats.drawCalls ?? engine.get_last_draw_calls());
  setText('zoom', engine.get_zoom().toFixed(2));
  setText('current-tool', engine.get_current_tool());
  canvasEl.style.cursor = lastPointer
    ? engine.cursor_at(lastPointer.x, lastPointer.y)
    : engine.get_tool_cursor();

  const undoBtn = document.getElementById('undo-btn');
  const redoBtn = document.getElementById('redo-btn');
  if (undoBtn) undoBtn.disabled = !engine.can_undo();
  if (redoBtn) redoBtn.disabled = !engine.can_redo();
  syncPropertyPanel(engine);
  syncAlignControls(engine);
  syncSnapControls(engine);
  drawAlignmentGuides(engine);
  layerPanel?.sync();
  paintMinimap(engine);
}

/**
 * 同步吸附开关和像素阈值。滑块正在拖动时不回写，避免抢光标。
 *
 * @param {WasmCanvas} engine
 */
function syncSnapControls(engine) {
  const enabled = engine.is_snap_enabled();
  document.getElementById('snap-btn')?.classList.toggle('active', enabled);
  const threshold = engine.get_snap_threshold();
  setText('snap-threshold-label', Math.round(threshold));
  const slider = document.getElementById('snap-threshold');
  if (slider instanceof HTMLInputElement && document.activeElement !== slider) {
    slider.value = String(threshold);
  }
}

/**
 * 把当前对齐参考线画到 WebGPU 画布上方。坐标是世界坐标，按视口换到屏幕像素。
 *
 * @param {WasmCanvas} engine
 */
function drawAlignmentGuides(engine) {
  const overlay = document.getElementById('guides');
  if (!(overlay instanceof HTMLCanvasElement) || !canvasEl) return;
  if (overlay.width !== canvasEl.width || overlay.height !== canvasEl.height) {
    overlay.width = canvasEl.width;
    overlay.height = canvasEl.height;
  }
  const ctx = overlay.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, overlay.width, overlay.height);

  const guides = engine.get_alignment_guides();
  const count = guides.length ?? 0;
  if (count === 0) return;

  const zoom = engine.get_zoom();
  const centerX = engine.get_viewport_center_x();
  const centerY = engine.get_viewport_center_y();
  const originX = overlay.width / 2;
  const originY = overlay.height / 2;

  ctx.save();
  ctx.strokeStyle = '#f2c94c';
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 4]);
  for (let index = 0; index < count; index += 1) {
    const guide = guides[index];
    ctx.beginPath();
    if (guide.axis === 'vertical') {
      const x = (guide.position - centerX) * zoom + originX;
      ctx.moveTo(x, 0);
      ctx.lineTo(x, overlay.height);
    } else {
      const y = (guide.position - centerY) * zoom + originY;
      ctx.moveTo(0, y);
      ctx.lineTo(overlay.width, y);
    }
    ctx.stroke();
  }
  ctx.restore();
}

/**
 * 把屏幕像素坐标换成舞台里的 CSS 像素。
 *
 * @param {number} x
 * @param {number} y
 * @returns {{ left: number, top: number, scaleX: number, scaleY: number }}
 */
function canvasCssPoint(x, y) {
  if (!canvasEl) {
    return { left: x, top: y, scaleX: 1, scaleY: 1 };
  }
  const rect = canvasEl.getBoundingClientRect();
  const stage = canvasEl.parentElement?.getBoundingClientRect();
  const scaleX = canvasEl.width > 0 ? rect.width / canvasEl.width : 1;
  const scaleY = canvasEl.height > 0 ? rect.height / canvasEl.height : 1;
  const originLeft = stage ? rect.left - stage.left : 0;
  const originTop = stage ? rect.top - stage.top : 0;
  return {
    left: originLeft + x * scaleX,
    top: originTop + y * scaleY,
    scaleX,
    scaleY,
  };
}

/**
 * 把 DOM 编辑框贴到文本的屏幕矩形上。
 *
 * @param {object} info `get_text_editor_metrics` 的返回值
 */
function placeTextEditor(info) {
  if (!textEditor) return;
  const point = canvasCssPoint(info.x, info.y);
  const fontPx = Math.max(8, (info.screenFontSize || 16) * point.scaleY);
  textEditor.node.style.left = `${point.left}px`;
  textEditor.node.style.top = `${point.top}px`;
  textEditor.node.style.width = `${Math.max(fontPx * 2, (info.width || 0) * point.scaleX)}px`;
  textEditor.node.style.height = `${Math.max(fontPx * 1.4, (info.height || 0) * point.scaleY)}px`;
  textEditor.node.style.fontSize = `${fontPx}px`;
}

/**
 * 打开覆盖在文本上的输入框。
 *
 * @param {WasmCanvas} engine
 * @param {object} info
 */
function openTextEditor(engine, info) {
  const node = document.getElementById('text-editor');
  if (!(node instanceof HTMLTextAreaElement) || info == null) return;
  if (textEditor && textEditor.id !== info.id) {
    closeTextEditor(engine, true);
  }
  textEditor = { id: info.id, node };
  textEditorOpening = true;
  node.hidden = false;
  node.value = info.content ?? '';
  node.style.color = info.color || '#f4f7fb';
  node.style.fontFamily = info.fontFamily || 'sans-serif';
  node.style.textAlign = info.align || 'left';
  placeTextEditor(info);
  engine.set_text_editing(info.id, true);
  node.focus();
  if (info.selectAll) node.select();
  requestAnimationFrame(() => {
    textEditorOpening = false;
  });
}

/**
 * 关闭编辑框。`commit` 为 true 时把内容写回引擎。
 *
 * @param {WasmCanvas} engine
 * @param {boolean} commit
 */
function closeTextEditor(engine, commit) {
  if (!textEditor) return;
  const { id, node } = textEditor;
  const value = node.value;
  textEditor = null;
  node.hidden = true;
  if (commit) {
    engine.commit_text_content(id, value);
    scheduleAutosave(engine);
  } else {
    engine.set_text_editing(id, false);
  }
}

/**
 * 光栅化待绘制文本，并处理 T 工具留下的编辑请求。
 *
 * @param {WasmCanvas} engine
 */
function syncText(engine) {
  const jobs = engine.take_text_raster_jobs();
  const count = jobs.length ?? 0;
  for (let i = 0; i < count; i += 1) {
    const job = jobs[i];
    try {
      const raster = rasterizeText(job);
      const pixels = new Uint8Array(raster.data.buffer, raster.data.byteOffset, raster.data.byteLength);
      engine.upload_texture(job.textureId, raster.width, raster.height, pixels);
      const accepted = engine.commit_text_raster(
        job.id,
        job.revision,
        job.lod,
        job.textureId,
        raster.worldWidth,
        raster.worldHeight,
      );
      if (!accepted) {
        console.warn('Text raster rejected', job.textureId);
      }
    } catch (error) {
      console.error('Text raster failed', error);
      engine.abandon_text_raster(job.id);
    }
  }

  const request = engine.take_text_edit_request();
  if (request) openTextEditor(engine, request);

  if (textEditor) {
    const metrics = engine.get_text_editor_metrics(textEditor.id);
    if (!metrics) {
      closeTextEditor(engine, false);
    } else {
      placeTextEditor(metrics);
    }
  }
}

/**
 * 启动渲染循环。WebGPU 渲染失败时停下来并给出提示。
 *
 * @param {WasmCanvas} engine
 */
function startLoop(engine) {
  const loop = () => {
    try {
      engine.render();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      showWebGPUMessage(message);
      return;
    }
    try {
      syncText(engine);
    } catch (error) {
      console.error('Text sync failed', error);
    }
    updateHud(engine);
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

/**
 * 页面入口：检测 WebGPU，初始化引擎，并放入示例场景。
 *
 * @returns {Promise<void>}
 */
async function main() {
  canvasEl = document.querySelector('#canvas');
  if (!(canvasEl instanceof HTMLCanvasElement)) {
    throw new Error('找不到画布');
  }

  try {
    setStatus('检查 WebGPU…');
    await assertWebGPU();
    showWebGPUMessage(null);

    setStatus('加载 WASM…');
    fitCanvas(canvasEl);
    await init(wasmUrl);
    wasm = new WasmCanvas();
    await wasm.init(canvasEl);
    layerPanel = attachLayerPanel(wasm, {
      onEdit() {
        scheduleAutosave(wasm);
      },
      setStatus,
    });
    bindInput(canvasEl, wasm);
    const minimap = document.getElementById('minimap');
    if (minimap instanceof HTMLCanvasElement) bindMinimap(minimap, wasm);
    try {
      if (restoreLocal(wasm)) {
        setStatus('已从本地恢复场景');
      } else {
        loadSample(wasm, canvasEl);
        persistLocal(wasm);
        setStatus('运行中');
      }
    } catch (error) {
      loadSample(wasm, canvasEl);
      setStatus(`本地场景无法读入，已载入示例：${errorText(error)}`, true);
    }
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') persistLocal(wasm);
    });
    window.addEventListener('beforeunload', () => persistLocal(wasm));
    startLoop(wasm);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error('InfiniteCanvas demo failed:', error);
    if (error instanceof WebGPUUnavailableError || /webgpu/i.test(message)) {
      showWebGPUMessage(message);
    } else {
      showWebGPUMessage(`初始化失败：${message}`);
    }
  }
}

main();

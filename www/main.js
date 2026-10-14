/**
 * InfiniteCanvas 首页 Demo。
 *
 * 把选择、直线、画笔、压力测试、纹理和小地图放在同一张画布上。
 * 选中元素后右侧属性面板可改填充、描边、线宽、线型、线帽、线结和不透明度，矩形还可改圆角，文本还可改字号和颜色。文本描边暂不支持线型。
 * 键盘操作只读 `www/shortcuts.js`：方向键按世界单位微调（Shift 为 10 倍），
 * Ctrl+C / Ctrl+V / Ctrl+D 复制、粘贴、克隆，都进撤销栈。输入框、下拉框内不触发。
 * 多选时工具栏和属性面板可对齐或均匀分布，只改平移，进撤销栈。
 * 选中后 Shift+H / Shift+V 翻转，Shift+R / Shift+Alt+R 转 90°，和工具栏、属性面板的按钮一样，进撤销栈。
 * 顶边和左边标尺可拖出参考线。G 显示或隐藏标尺和参考线。参考线进场景 JSON，导出默认不画。
 * I 切换吸管（不带 Shift、Ctrl、Alt）。单击采样最上层可见元素的填充和描边，经 SetStyle 写到当前选区。
 * Ctrl+' 显示或隐藏背景网格。选择写入 localStorage，下次打开保持上次的结果。第一次打开默认显示。
 * 保存/打开走版本化场景 JSON，视口或选区可导出 PNG，场景或选区可导出 SVG，改动会写入 localStorage。
 * 右侧图层面板和画布选中双向同步，拖拽排序走撤销栈。
 * `]` / `[` / Ctrl+] / Ctrl+[ 前移、后移、置顶、置底，和面板上的四个按钮一样，都进撤销栈。
 * Ctrl+L 切换锁定，Ctrl+Shift+H 切换隐藏，和图层面板的锁、眼睛同一条命令。
 * Shift+1 适应全部，Shift+2 缩放到选中。Ctrl+= / Ctrl++ 放大，Ctrl+- 缩小，Ctrl+0 回到 100%。
 * 步进倍数来自 `zoom_step`，和滚轮相同；键盘绕视口中心，滚轮绕指针。只改相机，不进撤销栈。
 * 页脚显示缩放百分比。焦点在输入框、下拉框或按钮上时，Ctrl/Cmd 缩放留给浏览器。
 * 右键空白处可粘贴、全选、缩放到适合；右键元素可复制、粘贴、克隆、删除、成组和调整层级。菜单项调用 `dispatchShortcut`。菜单里的删除只删元素。
 * 把 PNG/JPEG 拖进画布会放进资源表。`.svg` 拖进画布或工具栏「导入 SVG」解析成元素，viewBox 原点落在指针处。Ctrl+V 时剪贴板里有图片则粘贴图片，否则粘贴元素 JSON。
 *
 * @example
 * ```html
 * <script type="module" src="./main.js"></script>
 * ```
 */

import init, { InfiniteCanvas as WasmCanvas } from '../packages/core/wasm/infinitecanvas_wasm.js';
import wasmUrl from '../packages/core/wasm/infinitecanvas_wasm_bg.wasm?url';
import { attachLayerPanel, idKey, wasmId } from './layer-panel.js';
import {
  PASTE_STEP,
  createPasteSession,
  hasClipboardText,
  peekMemoryClipboard,
  readTextClipboard,
  screenToWorld,
  writeTextClipboard,
} from './clipboard.js';
import {
  canvasPointerReachesEngine,
  clampMenuPosition,
  contextMenuItems,
  mountContextMenu,
  resolveContextTarget,
} from './context-menu.js';
import { rasterizeText } from './text-raster.js';
import { SVG_IMPORT_DEFAULT_ANCHOR, isRasterImageFile, isSvgFile } from './svg-drop.js';
import {
  SHORTCUTS,
  escapeAction,
  formatChord,
  formatZoomPercent,
  helpEntries,
  isEditableTarget,
  matchShortcut,
  prefersMacModifier,
  shouldApplyShortcut,
} from './shortcuts.js';
import { readGridVisible, writeGridVisible } from './grid-pref.js';

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
/**
 * Ctrl+V 的序号。paste 事件若已经按图片处理，就把已消费序号抬到这里，
 * 元素 JSON 路径不再跑。
 */
let imagePasteSerial = 0;
/** 已经被图片粘贴占用的最大 Ctrl+V 序号。 */
let imagePasteConsumed = 0;
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
/** 背景网格是否显示。第一次打开为 true，之后跟 localStorage。 */
let gridVisible = true;
let minimapEpoch = null;
let minimapBuiltAt = 0;
let minimapForce = false;
/** @type {HTMLCanvasElement | null} */
let minimapElements = null;
/** @type {ArrayLike<number> | null} */
let minimapViewport = null;
/** @type {{ offsetX: number, offsetY: number } | null} */
let minimapDrag = null;
/** 右键菜单是否开着。开着时这一下指针不进入引擎。 */
let contextMenuOpen = false;
/** 打开菜单的世代。异步读剪贴板回来时，旧的一代不再改菜单。 */
let contextMenuToken = 0;
/** 右键时的画布像素。菜单上的粘贴对准这里，而不是菜单按钮上的指针。 */
/** @type {{ x: number, y: number } | null} */
let contextPastePoint = null;

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
 * 把屏幕上的点换成世界坐标。拖放和图片粘贴用它当图片左上角。
 *
 * 元素 JSON 粘贴用 `clipboard.js` 的 `screenToWorld`，这里走引擎视口，避免同名。
 *
 * @param {WasmCanvas} engine
 * @param {number} x
 * @param {number} y
 * @returns {{ x: number, y: number }}
 */
function engineScreenToWorld(engine, x, y) {
  const world = engine.screen_to_world(x, y);
  return { x: world[0], y: world[1] };
}

/**
 * 把 PNG/JPEG 文件放进场景。同一张图多次放入只存一份像素。
 *
 * @param {WasmCanvas} engine
 * @param {HTMLCanvasElement} canvas
 * @param {File[]} files
 * @param {{ x: number, y: number }} screen
 * @returns {Promise<number>}
 */
async function placeImageFiles(engine, canvas, files, screen) {
  const origin = engineScreenToWorld(engine, screen.x, screen.y);
  let placed = 0;
  for (const file of files) {
    const mime = file.type || '';
    if (mime && !mime.startsWith('image/png') && !mime.startsWith('image/jpeg') && mime !== 'image/jpg') {
      continue;
    }
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const offset = placed * 24;
      engine.add_image_bytes(bytes, mime, origin.x + offset, origin.y + offset);
      placed += 1;
    } catch (error) {
      setStatus(`图片导入失败：${errorText(error)}`, true);
    }
  }
  if (placed > 0) {
    scheduleAutosave(engine);
    setStatus(placed === 1 ? '已添加图片' : `已添加 ${placed} 张图片`);
  }
  return placed;
}

/**
 * 把 SVG 文件解析进场景。`screen` 为空时 viewBox 原点落在 {@link SVG_IMPORT_DEFAULT_ANCHOR}。
 *
 * @param {WasmCanvas} engine
 * @param {File[]} files
 * @param {{ x: number, y: number } | null} screen
 * @returns {Promise<number>}
 */
async function placeSvgFiles(engine, files, screen) {
  const origin = screen
    ? engineScreenToWorld(engine, screen.x, screen.y)
    : SVG_IMPORT_DEFAULT_ANCHOR;
  let placed = 0;
  for (const file of files) {
    if (!isSvgFile(file)) continue;
    try {
      const text = await file.text();
      const offset = placed * 24;
      const ids = engine.import_svg(text, origin.x + offset, origin.y + offset);
      if (ids.length > 0) placed += 1;
    } catch (error) {
      setStatus(`SVG 导入失败：${errorText(error)}`, true);
    }
  }
  if (placed > 0) {
    scheduleAutosave(engine);
    setStatus(placed === 1 ? '已导入 SVG' : `已导入 ${placed} 个 SVG`);
  }
  return placed;
}

/**
 * 画布接受拖入的图片和 SVG。PNG/JPEG 仍走图片元素，`.svg` 解析成形状。
 *
 * @param {HTMLCanvasElement} canvas
 * @param {WasmCanvas} engine
 */
function bindImageDrop(canvas, engine) {
  const stage = canvas.parentElement ?? canvas;
  stage.addEventListener('dragover', (event) => {
    const items = event.dataTransfer?.items;
    if (!items) return;
    for (const item of items) {
      if (item.kind === 'file') {
        event.preventDefault();
        if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
        return;
      }
    }
  });
  stage.addEventListener('drop', (event) => {
    const files = [...(event.dataTransfer?.files ?? [])];
    const images = files.filter((file) => isRasterImageFile(file));
    const svgs = files.filter((file) => isSvgFile(file));
    if (images.length === 0 && svgs.length === 0) return;
    event.preventDefault();
    const point = canvasPoint(event, canvas);
    if (images.length > 0) placeImageFiles(engine, canvas, images, point);
    if (svgs.length > 0) placeSvgFiles(engine, svgs, point);
  });
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
 * 帮助面板是否打开。
 *
 * @returns {boolean}
 */
function isHelpOpen() {
  const panel = document.getElementById('shortcut-help');
  return Boolean(panel) && !panel.hidden;
}

/**
 * 打开或关闭快捷键帮助。打开时按当前平台重绘，Mac 显示 ⌘。
 *
 * @param {boolean} open
 */
function setHelpOpen(open) {
  const panel = document.getElementById('shortcut-help');
  if (!panel) return;
  if (open) renderShortcutHelp();
  panel.hidden = !open;
}

/**
 * 用快捷键表填满帮助面板。面板上的每一行都来自这张表。
 */
function renderShortcutHelp() {
  const body = document.getElementById('shortcut-help-body');
  if (!body) return;
  const mac = prefersMacModifier();
  const fragment = document.createDocumentFragment();
  for (const row of helpEntries(SHORTCUTS, mac)) {
    const line = document.createElement('div');
    line.className = 'shortcut-row';

    const keys = document.createElement('div');
    keys.className = 'shortcut-keys';
    row.chords.forEach((chord, index) => {
      if (index > 0) {
        const separator = document.createElement('span');
        separator.className = 'shortcut-or';
        separator.textContent = ' / ';
        keys.append(separator);
      }
      const kbd = document.createElement('kbd');
      kbd.textContent = chord;
      keys.append(kbd);
    });

    const action = document.createElement('div');
    action.className = 'shortcut-action';
    action.textContent = row.action;

    const tool = document.createElement('div');
    tool.className = 'shortcut-tool';
    tool.textContent = row.toolLabel;

    line.append(keys, action, tool);
    fragment.append(line);
  }
  body.replaceChildren(fragment);
}

/**
 * 工具按钮上的键帽跟快捷键表保持一致。
 */
function syncShortcutLabels() {
  const mac = prefersMacModifier();
  for (const shortcut of SHORTCUTS) {
    if (shortcut.handler !== 'tool') continue;
    const kbd = document.querySelector(`.tool[data-tool="${shortcut.tool}"] kbd`);
    if (kbd) kbd.textContent = formatChord(shortcut, mac);
  }
  const help = SHORTCUTS.find((shortcut) => shortcut.handler === 'toggleHelp');
  const helpKbd = document.querySelector('#shortcuts-btn kbd');
  if (help && helpKbd) helpKbd.textContent = formatChord(help, mac);
  const grid = SHORTCUTS.find((shortcut) => shortcut.handler === 'toggleGrid');
  const gridKbd = document.querySelector('#grid-btn kbd');
  if (grid && gridKbd) gridKbd.textContent = formatChord(grid, mac);
}

/**
 * 把当前选择收成一个组，并改选新组。
 *
 * 选择里如果同时有组和它的子孙，只把组交出去。少于两个，或引擎拒绝时，选择保持不变。
 *
 * @param {WasmCanvas} engine
 */
function groupSelection(engine) {
  const tree = engine.get_scene_tree();
  const ids = selectionRoots([...engine.get_selected_ids()], tree?.roots);
  if (ids.length < 2) {
    setStatus('成组至少需要两个元素');
    return;
  }
  const groupId = engine.group_elements(ids);
  if (groupId === 0n || groupId === 0) {
    setStatus('无法成组');
    return;
  }
  engine.clear_selection();
  engine.select_element(groupId, false);
  scheduleAutosave(engine);
  setStatus('已成组');
}

/**
 * 解散当前选中的组，并改选原来的直接子元素。
 *
 * 不是组的选中项会被 `ungroup` 拒绝。子元素列表在解散前从场景树读出。
 *
 * @param {WasmCanvas} engine
 */
function ungroupSelection(engine) {
  const ids = [...engine.get_selected_ids()];
  const tree = engine.get_scene_tree();
  /** @type {Array<bigint | number>} */
  const children = [];
  let changed = false;
  for (const id of ids) {
    const node = findTreeNode(tree?.roots, id);
    if (!engine.ungroup(id)) continue;
    changed = true;
    for (const child of node?.children ?? []) {
      children.push(child.id);
    }
  }
  if (!changed) return;
  engine.clear_selection();
  for (const childId of children) {
    engine.select_element(childId, true);
  }
  scheduleAutosave(engine);
  setStatus('已取消分组');
}

/**
 * 去掉那些祖先也在选择里的 id。成组时只提交这批根。
 *
 * @param {Array<bigint | number>} ids
 * @param {Array<{ id: bigint | number, parentId: bigint | number | null, children?: unknown[] }> | undefined} roots
 * @returns {Array<bigint | number>}
 */
function selectionRoots(ids, roots) {
  const selected = new Set(ids.map((id) => String(id)));
  return ids.filter((id) => {
    let parentId = findTreeNode(roots, id)?.parentId;
    while (parentId != null) {
      if (selected.has(String(parentId))) return false;
      parentId = findTreeNode(roots, parentId)?.parentId;
    }
    return true;
  });
}

/**
 * 在场景树里按 id 查找节点。数字和 bigint 都按十进制文本比较。
 *
 * @param {Array<{ id: bigint | number, parentId?: bigint | number | null, children?: unknown[] }> | undefined} nodes
 * @param {bigint | number} id
 * @returns {any}
 */
function findTreeNode(nodes, id) {
  if (!nodes) return null;
  for (const node of nodes) {
    if (String(node.id) === String(id)) return node;
    const found = findTreeNode(node.children, id);
    if (found) return found;
  }
  return null;
}

/**
 * 执行快捷键表命中的动作。
 *
 * @param {import('./shortcuts.js').Shortcut} shortcut
 * @param {WasmCanvas} engine
 */
function dispatchShortcut(shortcut, engine) {
  switch (shortcut.handler) {
    case 'undo':
      if (engine.can_undo()) {
        activeStyleControl = null;
        engine.undo();
        scheduleAutosave(engine);
        syncPropertyPanel(engine);
      }
      break;
    case 'redo':
      if (engine.can_redo()) {
        activeStyleControl = null;
        engine.redo();
        scheduleAutosave(engine);
        syncPropertyPanel(engine);
      }
      break;
    case 'toggleMinimap':
      setMinimapVisible(!minimapVisible);
      break;
    case 'toggleRulers':
      engine.set_rulers_visible(!engine.are_rulers_visible());
      syncRulerControls(engine);
      drawAlignmentGuides(engine);
      break;
    case 'toggleGrid':
      setGridVisible(engine, !currentGridVisible(engine), { persist: true });
      break;
    case 'save':
      saveSceneFile(engine);
      break;
    case 'open':
      document.getElementById('open-file')?.click();
      break;
    case 'pushHand':
      if (!spaceHeld) {
        spaceHeld = true;
        engine.push_hand_tool();
      }
      break;
    case 'popHand':
      if (spaceHeld) {
        spaceHeld = false;
        engine.pop_temporary_tool();
      }
      break;
    case 'tool':
      setTool(shortcut.tool);
      break;
    case 'delete':
      if (engine.delete_selected_guide()) {
        scheduleAutosave(engine);
        drawAlignmentGuides(engine);
        break;
      }
      if (engine.delete_selection() > 0) scheduleAutosave(engine);
      break;
    case 'selectAll':
      engine.select_all();
      scheduleAutosave(engine);
      break;
    case 'group':
      groupSelection(engine);
      break;
    case 'ungroup':
      ungroupSelection(engine);
      break;
    case 'copy':
      enqueueClipboard(() => copySelection(engine));
      break;
    case 'paste': {
      // 序号先于 paste 事件。图片路径会把已消费序号抬到这里，元素 JSON 就不再跑。
      const serial = ++imagePasteSerial;
      enqueueClipboard(() => new Promise((resolve) => {
        setTimeout(async () => {
          try {
            if (imagePasteConsumed >= serial) return;
            await pasteSelection(engine);
          } finally {
            resolve();
          }
        }, 0);
      }));
      break;
    }
    case 'duplicate':
      duplicateSelection(engine);
      break;
    case 'nudge':
      nudgeSelection(engine, shortcut);
      break;
    case 'reorder':
      reorderSelection(engine, shortcut.order);
      break;
    case 'flip':
      flipSelection(engine, shortcut.axis);
      break;
    case 'rotate':
      rotateSelectionQuarter(engine, shortcut.dir);
      break;
    case 'toggleLock':
      toggleSelectionLocked(engine);
      break;
    case 'toggleHide':
      toggleSelectionVisible(engine);
      break;
    case 'escape':
      if (escapeAction(isHelpOpen()) === 'closeHelp') {
        setHelpOpen(false);
      } else {
        engine.clear_selection();
        scheduleAutosave(engine);
      }
      break;
    case 'zoomFit':
      fitScene(engine);
      break;
    case 'zoomSelection':
      zoomToSelection(engine);
      break;
    case 'zoomIn':
      engine.zoom_by(engine.zoom_step());
      break;
    case 'zoomOut':
      engine.zoom_by(1 / engine.zoom_step());
      break;
    case 'zoom100':
      engine.set_zoom(1);
      setStatus('已缩放到 100%');
      break;
    case 'toggleHelp':
      setHelpOpen(!isHelpOpen());
      break;
    default:
      break;
  }
}

/**
 * 方向键微调当前选中。普通步长 1 个世界单位，Shift 为 10 倍，由引擎计算。
 *
 * @param {WasmCanvas} engine
 * @param {import('./shortcuts.js').Shortcut} shortcut
 */
function nudgeSelection(engine, shortcut) {
  const direction = shortcut.direction;
  if (direction !== 'left' && direction !== 'right' && direction !== 'up' && direction !== 'down') {
    return;
  }
  if (engine.nudge_selection(direction, shortcut.shift === true)) {
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
  }
}

/**
 * 把当前选中按同一层的兄弟顺序重排。动作名交给引擎。
 *
 * @param {WasmCanvas} engine
 * @param {string | undefined} order `front`、`back`、`forward` 或 `backward`
 */
function reorderSelection(engine, order) {
  if (order !== 'front' && order !== 'back' && order !== 'forward' && order !== 'backward') {
    return;
  }
  if (engine.reorder_selection(order)) {
    scheduleAutosave(engine);
    layerPanel?.sync();
  }
}

/**
 * 绕选区中心水平或垂直翻转。按住不重复，避免一次长按堆很多条撤销。
 *
 * @param {WasmCanvas} engine
 * @param {string | undefined} axis `horizontal` 或 `vertical`
 */
function flipSelection(engine, axis) {
  if (axis !== 'horizontal' && axis !== 'vertical') return;
  if (typeof engine.flip_selection !== 'function') return;
  if (engine.flip_selection(axis)) {
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
    setStatus(axis === 'vertical' ? '已垂直翻转' : '已水平翻转');
  }
}

/**
 * 切换当前选中的锁定。与图层面板的锁按钮共用引擎命令，空选不入栈。
 *
 * @param {WasmCanvas} engine
 */
function toggleSelectionLocked(engine) {
  if (typeof engine.toggle_selection_locked !== 'function') return;
  if (engine.toggle_selection_locked()) {
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
    layerPanel?.sync();
    setStatus('已切换锁定');
  }
}

/**
 * 切换当前选中的可见性。与图层面板的眼睛按钮共用引擎命令，空选不入栈。
 *
 * @param {WasmCanvas} engine
 */
function toggleSelectionVisible(engine) {
  if (typeof engine.toggle_selection_visible !== 'function') return;
  if (engine.toggle_selection_visible()) {
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
    layerPanel?.sync();
    setStatus('已切换隐藏');
  }
}

/**
 * 绕选区中心转 90°。`cw` 顺时针，`ccw` 逆时针。
 *
 * @param {WasmCanvas} engine
 * @param {string | undefined} dir `cw` 或 `ccw`
 */
function rotateSelectionQuarter(engine, dir) {
  if (dir !== 'cw' && dir !== 'ccw') return;
  if (typeof engine.rotate_selection_quarter !== 'function') return;
  if (engine.rotate_selection_quarter(dir)) {
    scheduleAutosave(engine);
    syncPropertyPanel(engine);
    setStatus(dir === 'ccw' ? '已逆时针旋转 90°' : '已顺时针旋转 90°');
  }
}

/**
 * 工具栏高亮跟引擎里的当前工具对齐。吸管松手后引擎会自己切回去。
 *
 * @param {string} toolName
 */
function syncActiveTool(toolName) {
  document.querySelectorAll('.tool').forEach((button) => {
    button.classList.toggle('active', button.dataset.tool === toolName);
  });
  setText('current-tool', toolName);
}

/**
 * @param {string} toolName
 */
function setTool(toolName) {
  if (!wasm) return;
  wasm.set_tool(toolName);
  syncActiveTool(wasm.get_current_tool());
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
/**
 * 当前网格是否显示。引擎还没有这个方法时用页面上记住的值。
 *
 * @param {WasmCanvas} engine
 * @returns {boolean}
 */
function currentGridVisible(engine) {
  if (engine && typeof engine.grid_visible === 'function') {
    return engine.grid_visible();
  }
  return gridVisible;
}

/**
 * 显示或隐藏背景网格，并同步工具栏按钮。
 *
 * `persist` 为 true 时写入 localStorage。加载页面时不写，避免把第一次访问记成显式选择。
 *
 * @param {WasmCanvas} engine
 * @param {boolean} visible
 * @param {{ persist?: boolean }} [options]
 */
function setGridVisible(engine, visible, options = {}) {
  gridVisible = visible;
  if (engine && typeof engine.set_grid_visible === 'function') {
    engine.set_grid_visible(visible);
  }
  document.getElementById('grid-btn')?.classList.toggle('active', visible);
  if (options.persist) {
    writeGridVisible(globalThis.localStorage, visible);
    setStatus(visible ? '已显示网格' : '已隐藏网格');
  }
}

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
 * 菜单项 id 对应 `dispatchShortcut` 的 handler。层级项带上 `order`。
 *
 * @type {Record<string, { handler: string, order?: string }>}
 */
const CONTEXT_ACTIONS = {
  copy: { handler: 'copy' },
  paste: { handler: 'paste' },
  duplicate: { handler: 'duplicate' },
  delete: { handler: 'delete' },
  group: { handler: 'group' },
  ungroup: { handler: 'ungroup' },
  front: { handler: 'reorder', order: 'front' },
  back: { handler: 'reorder', order: 'back' },
  forward: { handler: 'reorder', order: 'forward' },
  backward: { handler: 'reorder', order: 'backward' },
  'select-all': { handler: 'selectAll' },
  'zoom-fit': { handler: 'zoomFit' },
};

/**
 * 关掉右键菜单。世代号前进，迟到的剪贴板读取不会把它再打开。
 */
function closeContextMenu() {
  contextMenuOpen = false;
  contextMenuToken += 1;
  contextPastePoint = null;
  const menu = document.getElementById('context-menu');
  if (menu) {
    menu.hidden = true;
    menu.replaceChildren();
  }
}

/**
 * 执行菜单项。每一项都调用 `dispatchShortcut`，和对应快捷键同一条路径。
 *
 * @param {WasmCanvas} engine
 * @param {string} action
 * @param {{ x: number, y: number } | null} pastePoint
 */
function runContextAction(engine, action, pastePoint) {
  const shortcut = CONTEXT_ACTIONS[action];
  closeContextMenu();
  if (!shortcut) return;
  if (shortcut.handler === 'paste' && pastePoint) {
    lastPointer = pastePoint;
    pointerInsideCanvas = true;
  }
  if (shortcut.handler === 'delete') {
    if (textEditor) closeTextEditor(engine, false);
    // 键盘 Delete 在参考线选中时先删参考线。菜单上的「删除」只删元素。
    if (engine.delete_selection() > 0) scheduleAutosave(engine);
    return;
  }
  dispatchShortcut(shortcut, engine);
}

/**
 * 在右键落点弹出菜单。命中未选中元素时先单选它。
 *
 * @param {HTMLCanvasElement} canvas
 * @param {WasmCanvas} engine
 * @param {MouseEvent} event
 */
function openContextMenu(canvas, engine, event) {
  const menu = document.getElementById('context-menu');
  if (!menu) return;
  const point = canvasPoint(event, canvas);
  contextPastePoint = point;
  lastPointer = point;
  pointerInsideCanvas = true;

  const hits = engine.hit_test_screen_point(point.x, point.y, 1);
  const hit = hits && hits.length ? hits[0] : null;
  const selectedIds = Array.from(engine.get_selected_ids?.() || []).map((id) => idKey(id));
  const parentOf = (id) => {
    const info = engine.get_element_info(wasmId(id));
    if (!info || info.parentId == null) return null;
    return idKey(info.parentId);
  };
  let target = resolveContextTarget(hit ? idKey(hit.id) : null, selectedIds, parentOf);
  if (target.selectHit && hit && !engine.select_element(wasmId(hit.id), false)) {
    target = { kind: 'canvas', selectHit: false };
  } else if (target.selectHit && hit) {
    scheduleAutosave(engine);
  }

  const ids = Array.from(engine.get_selected_ids?.() || []);
  const canGroup = target.kind === 'selection' && ids.length >= 2;
  let canUngroup = false;
  if (target.kind === 'selection' && ids.length === 1) {
    const info = engine.get_element_info(wasmId(ids[0]));
    canUngroup = info?.type === 'Group';
  }

  const token = ++contextMenuToken;
  const anchor = { x: event.clientX, y: event.clientY };
  const pastePoint = point;
  const paint = (clipboardHasContent) => {
    if (token !== contextMenuToken) return;
    const items = contextMenuItems({
      kind: target.kind,
      clipboardHasContent,
      canGroup,
      canUngroup,
    });
    mountContextMenu(menu, items, (action) => runContextAction(engine, action, pastePoint));
    menu.hidden = false;
    menu.style.left = '0px';
    menu.style.top = '0px';
    const rect = menu.getBoundingClientRect();
    const pos = clampMenuPosition(
      anchor.x,
      anchor.y,
      rect.width,
      rect.height,
      window.innerWidth,
      window.innerHeight,
    );
    menu.style.left = `${pos.x}px`;
    menu.style.top = `${pos.y}px`;
    contextMenuOpen = true;
  };
  paint(hasClipboardText(peekMemoryClipboard()));
  readTextClipboard().then((text) => {
    paint(hasClipboardText(text));
  }).catch(() => {});
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
  document.getElementById('fit-scene-btn')?.addEventListener('click', () => {
    fitScene(engine);
  });
  document.getElementById('zoom-selection-btn')?.addEventListener('click', () => {
    zoomToSelection(engine);
  });
}

/**
 * 适应全部可见叶子。空场景时引擎把相机放回原点、缩放 1。
 *
 * 工具栏和 `SHORTCUTS` 的 `zoomFit`（Shift+1）都走这里。
 * 调用 `zoom_to_fit`；`fit_scene` 是同一个命令。
 *
 * @param {WasmCanvas} engine
 */
function fitScene(engine) {
  if (engine.zoom_to_fit()) {
    setStatus('已适应全部');
  } else {
    setStatus('空场景，相机已回到默认');
  }
}

/**
 * 缩放到当前选中。空选或没有可见内容时不改相机。
 *
 * 工具栏和 `SHORTCUTS` 的 `zoomSelection`（Shift+2）都走这里。
 *
 * @param {WasmCanvas} engine
 */
function zoomToSelection(engine) {
  if (engine.zoom_to_selection()) {
    setStatus('已缩放到选中');
  } else {
    setStatus('没有可缩放的选中内容');
  }
}

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
 * 只选中图片时只留下不透明度；填充和描边仍只出现在形状或文本上。
 *
 * @param {WasmCanvas} engine
 */
function syncPropertyPanel(engine) {
  const panel = document.getElementById('property-panel');
  if (!panel || typeof engine.get_selection_style !== 'function') return;
  const style = engine.get_selection_style();
  const visible = Boolean(style && (style.hasShape || style.hasText || style.hasImage));
  panel.hidden = !visible;
  if (!visible || !style) return;

  setText('prop-count', style.count ?? 0);
  const fillRow = document.getElementById('prop-fill-row');
  if (fillRow) fillRow.hidden = !style.hasShape;
  const strokeRow = document.getElementById('prop-stroke-row');
  if (strokeRow) strokeRow.hidden = !style.strokeSupported;
  const strokeWidthRow = document.getElementById('prop-stroke-width-row');
  if (strokeWidthRow) strokeWidthRow.hidden = !style.strokeWidthSupported;
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
  syncCornerRadius(style);
  syncPolygonFields(style);

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

  const lineStyle = document.getElementById('prop-line-style');
  if (lineStyle) lineStyle.hidden = !style.hasShape;
  syncDash(style);
  syncChoice('prop-stroke-cap', style.strokeCap, style.strokeCapMixed);
  syncChoice('prop-stroke-join', style.strokeJoin, style.strokeJoinMixed);
}

/**
 * 把虚线数组收成属性面板的预设。不认识的图案标成自定义。
 *
 * @param {unknown} dash
 * @returns {'solid' | 'dashed' | 'dotted' | 'custom'}
 */
function dashPreset(dash) {
  if (!Array.isArray(dash) || dash.length === 0) return 'solid';
  if (dash.length === 2 && Number(dash[0]) === 8 && Number(dash[1]) === 4) return 'dashed';
  if (dash.length === 2 && Number(dash[0]) === 2 && Number(dash[1]) === 4) return 'dotted';
  return 'custom';
}

/**
 * @param {Record<string, unknown>} style
 */
function syncDash(style) {
  const mixedEl = document.getElementById('prop-stroke-dash-mixed');
  if (mixedEl) mixedEl.hidden = !style.strokeDashMixed;
  const input = document.getElementById('prop-stroke-dash');
  if (!(input instanceof HTMLSelectElement)) return;
  if (style.strokeDashMixed || !Array.isArray(style.strokeDash)) {
    input.value = '';
    return;
  }
  input.value = dashPreset(style.strokeDash);
}

/**
 * @param {string} id
 * @param {unknown} value
 * @param {unknown} mixed
 */
function syncChoice(id, value, mixed) {
  const mixedEl = document.getElementById(`${id}-mixed`);
  if (mixedEl) mixedEl.hidden = !mixed;
  const input = document.getElementById(id);
  if (!(input instanceof HTMLSelectElement)) return;
  input.value = mixed || typeof value !== 'string' ? '' : value;
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
 * 有选中元素时才允许翻转和 90° 旋转。空选保持禁用。
 *
 * @param {WasmCanvas} engine
 */
function syncOrientControls(engine) {
  const enabled = engine.get_selection_count() >= 1;
  document.querySelectorAll('[data-flip], [data-rotate]').forEach((button) => {
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

/**
 * 翻转和 90° 旋转各记一条撤销记录。不改组的父子关系。
 *
 * @param {WasmCanvas} engine
 */
function bindOrientControls(engine) {
  document.querySelectorAll('[data-flip]').forEach((button) => {
    button.addEventListener('click', () => {
      const axis = button.getAttribute('data-flip');
      if (!axis || typeof engine.flip_selection !== 'function') return;
      if (engine.flip_selection(axis)) {
        scheduleAutosave(engine);
        syncPropertyPanel(engine);
        setStatus(axis === 'vertical' ? '已垂直翻转' : '已水平翻转');
      }
    });
  });
  document.querySelectorAll('[data-rotate]').forEach((button) => {
    button.addEventListener('click', () => {
      const dir = button.getAttribute('data-rotate');
      if (!dir || typeof engine.rotate_selection_quarter !== 'function') return;
      if (engine.rotate_selection_quarter(dir)) {
        scheduleAutosave(engine);
        syncPropertyPanel(engine);
        setStatus(dir === 'ccw' ? '已逆时针旋转 90°' : '已顺时针旋转 90°');
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
  bindCornerRadius(engine);
  bindPolygonFields(engine);
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

  const bindChoice = (id, key, mapValue) => {
    const input = document.getElementById(id);
    if (!(input instanceof HTMLSelectElement)) return;
    input.addEventListener('change', () => {
      if (!input.value || input.value === 'custom') return;
      engine.set_selection_style({ [key]: mapValue(input.value) });
      scheduleAutosave(engine);
      syncPropertyPanel(engine);
    });
  };
  bindChoice('prop-stroke-dash', 'strokeDash', (value) => {
    if (value === 'dashed') return [8, 4];
    if (value === 'dotted') return [2, 4];
    return [];
  });
  bindChoice('prop-stroke-cap', 'strokeCap', (value) => value);
  bindChoice('prop-stroke-join', 'strokeJoin', (value) => value);
}

/**
 * 圆角只在选中矩形时出现。滑块和数字框写同一个 `cornerRadius`，松手收成一条历史。
 *
 * @param {Record<string, unknown>} style
 */
function syncCornerRadius(style) {
  const row = document.getElementById('prop-corner-radius-row');
  if (row) row.hidden = !style.cornerRadiusSupported;
  const mixedEl = document.getElementById('prop-corner-radius-mixed');
  if (mixedEl) mixedEl.hidden = !style.cornerRadiusMixed;
  const active =
    activeStyleControl === 'prop-corner-radius' ||
    activeStyleControl === 'prop-corner-radius-number';
  if (active) return;
  const range = document.getElementById('prop-corner-radius');
  const number = document.getElementById('prop-corner-radius-number');
  if (typeof style.cornerRadius === 'number' && Number.isFinite(style.cornerRadius)) {
    if (range instanceof HTMLInputElement) range.value = String(style.cornerRadius);
    if (number instanceof HTMLInputElement) number.value = String(style.cornerRadius);
  }
}

/**
 * @param {WasmCanvas} engine
 */
/**
 * 边数在选中多边形或星形时出现。内径比只在选中星形时出现。
 * 滑块和数字框写同一个字段，松手收成一条 `SetStyle` 历史。
 *
 * @param {Record<string, unknown>} style
 */
function syncPolygonFields(style) {
  const sidesRow = document.getElementById('prop-sides-row');
  if (sidesRow) sidesRow.hidden = !style.sidesSupported;
  const sidesMixed = document.getElementById('prop-sides-mixed');
  if (sidesMixed) sidesMixed.hidden = !style.sidesMixed;
  const sidesLabel = document.getElementById('prop-sides-label');
  if (sidesLabel) {
    sidesLabel.textContent = style.hasStar && !style.hasPolygon ? '角数' : '边数';
  }
  const sidesActive =
    activeStyleControl === 'prop-sides' || activeStyleControl === 'prop-sides-number';
  if (!sidesActive && typeof style.sides === 'number' && Number.isFinite(style.sides)) {
    const range = document.getElementById('prop-sides');
    const number = document.getElementById('prop-sides-number');
    const shown = String(Math.round(style.sides));
    if (range instanceof HTMLInputElement) range.value = shown;
    if (number instanceof HTMLInputElement) number.value = shown;
  }

  const innerRow = document.getElementById('prop-inner-radius-row');
  if (innerRow) innerRow.hidden = !style.innerRadiusSupported;
  const innerMixed = document.getElementById('prop-inner-radius-mixed');
  if (innerMixed) innerMixed.hidden = !style.innerRadiusMixed;
  const innerActive =
    activeStyleControl === 'prop-inner-radius' ||
    activeStyleControl === 'prop-inner-radius-number';
  if (
    !innerActive &&
    typeof style.innerRadius === 'number' &&
    Number.isFinite(style.innerRadius)
  ) {
    const range = document.getElementById('prop-inner-radius');
    const number = document.getElementById('prop-inner-radius-number');
    if (range instanceof HTMLInputElement) range.value = String(style.innerRadius);
    if (number instanceof HTMLInputElement) number.value = String(style.innerRadius);
  }
}

/**
 * @param {WasmCanvas} engine
 */
function bindPolygonFields(engine) {
  const bindPair = (rangeId, numberId, key, integer) => {
    const range = document.getElementById(rangeId);
    const number = document.getElementById(numberId);
    const preview = (source, value) => {
      if (!Number.isFinite(value)) return;
      const next = integer ? Math.round(value) : value;
      if (integer && (next < 3 || next > 64)) return;
      if (!integer && (next < 0.05 || next > 0.95)) return;
      activeStyleControl = source;
      if (range instanceof HTMLInputElement && source !== rangeId) range.value = String(next);
      if (number instanceof HTMLInputElement && source !== numberId) number.value = String(next);
      engine.preview_selection_style({ [key]: next });
    };
    if (range instanceof HTMLInputElement) {
      range.addEventListener('input', () => preview(rangeId, Number(range.value)));
      range.addEventListener('change', () => finishStyleEdit(engine));
    }
    if (number instanceof HTMLInputElement) {
      number.addEventListener('input', () => preview(numberId, Number(number.value)));
      number.addEventListener('change', () => finishStyleEdit(engine));
    }
  };
  bindPair('prop-sides', 'prop-sides-number', 'sides', true);
  bindPair('prop-inner-radius', 'prop-inner-radius-number', 'innerRadius', false);
}

function bindCornerRadius(engine) {
  const range = document.getElementById('prop-corner-radius');
  const number = document.getElementById('prop-corner-radius-number');
  const preview = (source, value) => {
    if (!Number.isFinite(value) || value < 0) return;
    activeStyleControl = source;
    if (range instanceof HTMLInputElement && source !== 'prop-corner-radius') {
      range.value = String(Math.min(value, Number(range.max)));
    }
    if (number instanceof HTMLInputElement && source !== 'prop-corner-radius-number') {
      number.value = String(value);
    }
    engine.preview_selection_style({ cornerRadius: value });
  };
  if (range instanceof HTMLInputElement) {
    range.addEventListener('input', () => preview('prop-corner-radius', Number(range.value)));
    range.addEventListener('change', () => finishStyleEdit(engine));
  }
  if (number instanceof HTMLInputElement) {
    number.addEventListener('input', () => preview('prop-corner-radius-number', Number(number.value)));
    number.addEventListener('change', () => finishStyleEdit(engine));
  }
}

function bindInput(canvas, engine) {
  bindPropertyPanel(engine);
  bindAlignControls(engine);
  bindOrientControls(engine);
  document.addEventListener('pointerdown', (event) => {
    if (!contextMenuOpen) return;
    const menu = document.getElementById('context-menu');
    if (menu && event.target instanceof Node && menu.contains(event.target)) return;
    const onCanvas = event.target === canvas;
    closeContextMenu();
    // 关掉菜单的这一下已经发生在菜单打开期间，不能再拿去平移或框选。
    if (onCanvas && !canvasPointerReachesEngine(true)) {
      event.preventDefault();
      event.stopPropagation();
    }
  }, true);
  canvas.addEventListener('pointerdown', (event) => {
    if (!canvasPointerReachesEngine(contextMenuOpen)) {
      closeContextMenu();
      event.preventDefault();
      event.stopPropagation();
      return;
    }
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
    if (engine.delete_guide_at(point.x, point.y)) {
      scheduleAutosave(engine);
      return;
    }
    const info = engine.begin_text_edit_at(point.x, point.y);
    if (info) openTextEditor(engine, info);
  });

  canvas.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    openContextMenu(canvas, engine, event);
  });
  document.getElementById('context-menu')?.addEventListener('contextmenu', (event) => {
    event.preventDefault();
  });
  bindImageDrop(canvas, engine);

  const editor = document.getElementById('text-editor');
  if (editor instanceof HTMLTextAreaElement) {
    editor.addEventListener('keydown', (event) => {
      const shortcut = matchShortcut(SHORTCUTS, event, 'down', 'text-editor');
      if (!shortcut) return;
      event.preventDefault();
      event.stopPropagation();
      if (shortcut.handler === 'commitText') closeTextEditor(engine, true);
      if (shortcut.handler === 'cancelText') closeTextEditor(engine, false);
    });
    editor.addEventListener('blur', () => {
      if (textEditorOpening) return;
      closeTextEditor(engine, true);
    });
  }

  canvas.addEventListener('wheel', (event) => {
    event.preventDefault();
    const point = canvasPoint(event, canvas);
    const step = engine.zoom_step();
    const factor = event.deltaY < 0 ? step : 1 / step;
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
  document.getElementById('import-svg-btn')?.addEventListener('click', () => {
    document.getElementById('import-svg-file')?.click();
  });
  document.getElementById('import-svg-file')?.addEventListener('change', async (event) => {
    const input = event.target;
    if (!(input instanceof HTMLInputElement)) return;
    const files = [...(input.files ?? [])];
    input.value = '';
    if (files.length === 0) return;
    await placeSvgFiles(engine, files, lastPointer);
  });
  document.getElementById('snap-btn')?.addEventListener('click', () => {
    engine.set_snap_enabled(!engine.is_snap_enabled());
    syncSnapControls(engine);
    drawAlignmentGuides(engine);
  });

  document.getElementById('rulers-btn')?.addEventListener('click', () => {
    engine.set_rulers_visible(!engine.are_rulers_visible());
    syncRulerControls(engine);
    drawAlignmentGuides(engine);
  });
  document.getElementById('grid-btn')?.addEventListener('click', () => {
    setGridVisible(engine, !currentGridVisible(engine), { persist: true });
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

  document.addEventListener('paste', (event) => {
    if (event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement) {
      return;
    }
    const items = event.clipboardData?.items;
    if (!items) return;
    const imageFiles = [];
    const svgFiles = [];
    for (const item of items) {
      if (item.kind !== 'file') continue;
      const file = item.getAsFile();
      if (!file) continue;
      if (isSvgFile(file) || item.type === 'image/svg+xml') svgFiles.push(file);
      else if (item.type.startsWith('image/')) imageFiles.push(file);
    }
    if (imageFiles.length > 0 || svgFiles.length > 0) {
      event.preventDefault();
      imagePasteConsumed = imagePasteSerial;
      const point = lastPointer ?? { x: canvas.width / 2, y: canvas.height / 2 };
      if (imageFiles.length > 0) placeImageFiles(engine, canvas, imageFiles, point);
      if (svgFiles.length > 0) placeSvgFiles(engine, svgFiles, point);
      return;
    }
    // 没有图片：这次 Ctrl+V 留给元素 JSON。阻止浏览器把文本写进页面。
    if (imagePasteSerial > imagePasteConsumed) {
      event.preventDefault();
    }
  });

  syncShortcutLabels();
  renderShortcutHelp();
  document.getElementById('shortcuts-btn')?.addEventListener('click', () => {
    setHelpOpen(!isHelpOpen());
  });
  document.getElementById('shortcut-help-close')?.addEventListener('click', () => {
    setHelpOpen(false);
  });
  document.getElementById('shortcut-help')?.addEventListener('click', (event) => {
    if (event.target === event.currentTarget) setHelpOpen(false);
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && contextMenuOpen && !isEditableTarget(event.target)) {
      event.preventDefault();
      closeContextMenu();
      return;
    }
    if (isEditableTarget(event.target)) return;
    if (contextMenuOpen) closeContextMenu();
    const shortcut = matchShortcut(SHORTCUTS, event, 'down');
    // 输入框已在上面返回。会挡住浏览器缩放的和弦还要焦点在画布上，否则不 preventDefault。
    if (!shortcut || !shouldApplyShortcut(shortcut, event.target)) return;
    // Ctrl/Cmd+V 不拦截默认动作：paste 事件要先看到剪贴板里的图片。
    if (shortcut.handler !== 'paste') event.preventDefault();
    if (event.repeat && shortcut.allowRepeat === false) return;
    dispatchShortcut(shortcut, engine);
  });

  document.addEventListener('keyup', (event) => {
    const shortcut = matchShortcut(SHORTCUTS, event, 'up');
    if (!shortcut) return;
    // 空格是按下时记下的。焦点中途进了输入框，松开时也要恢复工具。
    if (isEditableTarget(event.target) && !(shortcut.handler === 'popHand' && spaceHeld)) return;
    event.preventDefault();
    dispatchShortcut(shortcut, engine);
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
  setText('zoom', formatZoomPercent(engine.get_zoom()));
  setText('current-tool', engine.get_current_tool());
  if (typeof engine.take_eyedropper_notice === 'function') {
    const notice = engine.take_eyedropper_notice();
    if (notice) {
      syncActiveTool(engine.get_current_tool());
      if (notice.applied) {
        setStatus('已吸取颜色并应用到选区');
        scheduleAutosave(engine);
      } else if (notice.hit) {
        setStatus('没有可应用到选区的颜色');
      } else {
        setStatus('这里没有可吸取的元素');
      }
    }
  }
  canvasEl.style.cursor = lastPointer
    ? engine.cursor_at(lastPointer.x, lastPointer.y)
    : engine.get_tool_cursor();

  const undoBtn = document.getElementById('undo-btn');
  const redoBtn = document.getElementById('redo-btn');
  if (undoBtn) undoBtn.disabled = !engine.can_undo();
  if (redoBtn) redoBtn.disabled = !engine.can_redo();
  syncPropertyPanel(engine);
  syncAlignControls(engine);
  syncOrientControls(engine);
  syncSnapControls(engine);
  syncRulerControls(engine);
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
 * 同步标尺开关。参考线跟着标尺一起显示或隐藏。
 *
 * @param {WasmCanvas} engine
 */
function syncRulerControls(engine) {
  const visible = engine.are_rulers_visible();
  document.getElementById('rulers-btn')?.classList.toggle('active', visible);
}

/**
 * 把世界坐标换成画布像素。和视口中心、缩放同一套公式。
 *
 * @param {number} worldX
 * @param {number} worldY
 * @param {number} centerX
 * @param {number} centerY
 * @param {number} zoom
 * @param {number} originX
 * @param {number} originY
 * @returns {{ x: number, y: number }}
 */
function worldToOverlay(worldX, worldY, centerX, centerY, zoom, originX, originY) {
  return {
    x: (worldX - centerX) * zoom + originX,
    y: (worldY - centerY) * zoom + originY,
  };
}

/**
 * 主刻度上的数字。步长不小于 1 时取整，否则按步长保留小数。
 *
 * @param {number} world
 * @param {number} step
 * @returns {string}
 */
function formatRulerLabel(world, step) {
  if (!Number.isFinite(world)) return '';
  if (!Number.isFinite(step) || step >= 1) return String(Math.round(world));
  const digits = Math.min(3, Math.max(1, Math.ceil(-Math.log10(step))));
  return world.toFixed(digits);
}

/**
 * 画一条标尺的刻度和数字。
 *
 * @param {CanvasRenderingContext2D} ctx
 * @param {Array<{ world: number, screen: number, major: boolean }>} ticks
 * @param {boolean} horizontal 顶边标尺为 true，刻度沿 x
 * @param {number} thickness
 * @param {number} step
 */
function drawRulerTicks(ctx, ticks, horizontal, thickness, step) {
  const count = ticks.length ?? 0;
  ctx.save();
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.45)';
  ctx.fillStyle = 'rgba(232, 236, 243, 0.82)';
  ctx.lineWidth = 1;
  ctx.font = '10px ui-monospace, monospace';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (let index = 0; index < count; index += 1) {
    const tick = ticks[index];
    const screen = tick.screen;
    if (!Number.isFinite(screen)) continue;
    const length = tick.major ? 10 : 5;
    ctx.beginPath();
    if (horizontal) {
      if (screen < thickness) continue;
      ctx.moveTo(screen, thickness);
      ctx.lineTo(screen, thickness - length);
    } else {
      if (screen < thickness) continue;
      ctx.moveTo(thickness, screen);
      ctx.lineTo(thickness - length, screen);
    }
    ctx.stroke();
    if (!tick.major) continue;
    const label = formatRulerLabel(tick.world, step);
    if (horizontal) {
      if (screen < thickness + 16) continue;
      ctx.fillText(label, screen, 10);
    } else if (screen > thickness + 18) {
      ctx.save();
      ctx.translate(10, screen);
      ctx.rotate(-Math.PI / 2);
      ctx.fillText(label, 0, 0);
      ctx.restore();
    }
  }
  ctx.restore();
}

/**
 * 把当前对齐参考线、手动参考线和标尺画到 WebGPU 画布上方。
 *
 * 手动参考线是实线。拖拽时的对齐参考线仍是黄色虚线，画在最上面。
 * 标尺盖住画布边缘，刻度随视口更新。
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

  const zoom = engine.get_zoom();
  const centerX = engine.get_viewport_center_x();
  const centerY = engine.get_viewport_center_y();
  const originX = overlay.width / 2;
  const originY = overlay.height / 2;
  const showRulers = engine.are_rulers_visible();
  const thickness = showRulers ? engine.get_ruler_thickness() : 0;

  if (showRulers) {
    const manual = engine.get_manual_guides();
    const manualCount = manual.length ?? 0;
    ctx.save();
    ctx.lineWidth = 1;
    ctx.setLineDash([]);
    for (let index = 0; index < manualCount; index += 1) {
      const guide = manual[index];
      ctx.strokeStyle = guide.selected ? '#ffffff' : '#3db7ff';
      ctx.lineWidth = guide.selected ? 1.5 : 1;
      ctx.beginPath();
      if (guide.axis === 'vertical') {
        const x = worldToOverlay(guide.position, 0, centerX, centerY, zoom, originX, originY).x;
        ctx.moveTo(x, thickness);
        ctx.lineTo(x, overlay.height);
      } else {
        const y = worldToOverlay(0, guide.position, centerX, centerY, zoom, originX, originY).y;
        ctx.moveTo(thickness, y);
        ctx.lineTo(overlay.width, y);
      }
      ctx.stroke();
    }
    ctx.restore();

    ctx.save();
    ctx.fillStyle = 'rgba(16, 18, 24, 0.94)';
    ctx.fillRect(0, 0, overlay.width, thickness);
    ctx.fillRect(0, 0, thickness, overlay.height);
    const step = engine.get_ruler_major_step();
    drawRulerTicks(ctx, engine.get_ruler_ticks('horizontal'), true, thickness, step);
    drawRulerTicks(ctx, engine.get_ruler_ticks('vertical'), false, thickness, step);
    ctx.fillStyle = 'rgba(22, 24, 31, 0.98)';
    ctx.fillRect(0, 0, thickness, thickness);
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.16)';
    ctx.strokeRect(0.5, 0.5, overlay.width - 1, thickness - 0.5);
    ctx.strokeRect(0.5, 0.5, thickness - 0.5, overlay.height - 1);
    ctx.restore();
  }

  const guides = engine.get_alignment_guides();
  const count = guides.length ?? 0;
  if (count === 0) return;

  ctx.save();
  ctx.strokeStyle = '#f2c94c';
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 4]);
  for (let index = 0; index < count; index += 1) {
    const guide = guides[index];
    ctx.beginPath();
    if (guide.axis === 'vertical') {
      const x = worldToOverlay(guide.position, 0, centerX, centerY, zoom, originX, originY).x;
      ctx.moveTo(x, thickness);
      ctx.lineTo(x, overlay.height);
    } else {
      const y = worldToOverlay(0, guide.position, centerX, centerY, zoom, originX, originY).y;
      ctx.moveTo(thickness, y);
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
    setGridVisible(wasm, readGridVisible(globalThis.localStorage));
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

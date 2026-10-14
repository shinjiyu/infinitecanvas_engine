/**
 * 复制 / 粘贴的纯逻辑。
 *
 * Ctrl+C 写入系统剪贴板，失败时退回内存。Ctrl+V 先读系统剪贴板，失败再用内存。
 * 鼠标在画布内时粘贴以指针为中心，偏移从 0 开始；否则从原位置 +(10, 10) 起。
 * 同一份文本连续粘贴，偏移每次再加 10。Ctrl+D 不走这里，由引擎按 (10, 10) 克隆当前选中。
 *
 * @example
 * ```js
 * const session = createPasteSession();
 * session.arm('{"version":1}');
 * session.nextPlacement('{"version":1}', false); // { dx: 10, dy: 10 }
 * ```
 */

/** 连续粘贴和原地克隆使用的世界单位步长。 */
export const PASTE_STEP = 10;

/** 内存剪贴板。系统剪贴板不可用时读写都落在这里。 */
let memoryClipboard = '';

/**
 * 文本去掉空白后是否还能粘贴。
 *
 * @param {unknown} text 剪贴板读到的字符串
 * @returns {boolean}
 */
export function hasClipboardText(text) {
  return typeof text === 'string' && text.trim().length > 0;
}

/**
 * 当前内存剪贴板。打开菜单时先用它决定要不要显示粘贴，再异步读系统剪贴板。
 *
 * @returns {string}
 */
export function peekMemoryClipboard() {
  return memoryClipboard;
}

/**
 * 计算这一次粘贴相对文档坐标的额外位移。
 *
 * 指针在画布内时，第 0 次是 0（中心对准指针），之后每次 +10。
 * 指针不在画布内时，第 0 次就是 +10，之后 +20、+30。
 *
 * @param {boolean} pointerInside 鼠标是否在画布内
 * @param {number} generation 同一份剪贴板已经粘贴过的次数，从 0 开始
 * @returns {{ dx: number, dy: number }}
 */
export function pasteOffset(pointerInside, generation) {
  const count = Number.isFinite(generation) ? Math.max(0, generation) : 0;
  const steps = pointerInside ? count : count + 1;
  return { dx: PASTE_STEP * steps, dy: PASTE_STEP * steps };
}

/**
 * 屏幕像素转世界坐标，与视口的 `screen_to_world` 一致。
 *
 * @param {{ x: number, y: number }} point 画布像素
 * @param {{ width: number, height: number }} canvasSize 画布像素尺寸
 * @param {{ x: number, y: number }} center 视口中心（世界坐标）
 * @param {number} zoom 视口缩放
 * @returns {{ x: number, y: number }}
 */
export function screenToWorld(point, canvasSize, center, zoom) {
  const safeZoom = zoom && Number.isFinite(zoom) ? zoom : 1;
  return {
    x: center.x + (point.x - canvasSize.width / 2) / safeZoom,
    y: center.y + (point.y - canvasSize.height / 2) / safeZoom,
  };
}

/**
 * 记住同一份剪贴板文本，并给出下一次粘贴的位移。
 *
 * `arm` 在 Ctrl+C 时把计数清零。文本变了也会清零。
 *
 * @returns {{ arm: (text: string) => void, nextPlacement: (text: string, pointerInside: boolean) => { dx: number, dy: number } }}
 */
export function createPasteSession() {
  let armed = '';
  let generation = 0;
  return {
    /**
     * @param {string} text 刚复制出去的 JSON
     */
    arm(text) {
      armed = text;
      generation = 0;
    },
    /**
     * @param {string} text 这次读到的剪贴板
     * @param {boolean} pointerInside 鼠标是否在画布内
     * @returns {{ dx: number, dy: number }}
     */
    nextPlacement(text, pointerInside) {
      if (text !== armed) {
        armed = text;
        generation = 0;
      }
      const placement = pasteOffset(pointerInside, generation);
      generation += 1;
      return placement;
    },
  };
}

/**
 * 浏览器剪贴板。测试可以传入替身。
 *
 * @returns {{ writeText?: (text: string) => Promise<void>, readText?: () => Promise<string> } | null}
 */
function systemClipboard() {
  if (typeof navigator === 'undefined' || !navigator.clipboard) return null;
  return navigator.clipboard;
}

/**
 * 写入系统剪贴板。失败或没有 API 时只留在内存里，不抛错。
 *
 * @param {string} text
 * @param {{ writeText?: (text: string) => Promise<void> } | null} [clipboard]
 * @returns {Promise<void>}
 */
export async function writeTextClipboard(text, clipboard = systemClipboard()) {
  memoryClipboard = text;
  try {
    if (clipboard && typeof clipboard.writeText === 'function') {
      await clipboard.writeText(text);
    }
  } catch {
    // 权限被拒绝或非安全上下文：内存剪贴板已经记下文本。
  }
}

/**
 * 读取剪贴板。系统剪贴板失败、为空或不可用时返回内存副本。
 *
 * @param {{ readText?: () => Promise<string> } | null} [clipboard]
 * @returns {Promise<string>}
 */
export async function readTextClipboard(clipboard = systemClipboard()) {
  try {
    if (clipboard && typeof clipboard.readText === 'function') {
      const text = await clipboard.readText();
      if (typeof text === 'string' && text.length > 0) {
        return text;
      }
    }
  } catch {
    // 读系统剪贴板失败时用内存副本。
  }
  return memoryClipboard;
}

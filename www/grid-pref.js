/**
 * 首页 Demo 的背景网格显隐偏好。
 *
 * 只认 `localStorage` 里的字符串 `"false"` 为隐藏。缺省、空串和其他值都保持可见，
 * 这样第一次打开和读失败时都和引擎默认一致。
 *
 * @example
 * ```js
 * const visible = readGridVisible(localStorage);
 * writeGridVisible(localStorage, false);
 * ```
 */

/** @type {string} */
export const GRID_VISIBLE_KEY = 'infinitecanvas.gridVisible';

/**
 * 读出上次选择。没有记录或存储抛错时返回 true。
 *
 * @param {{ getItem: (key: string) => string | null } | null | undefined} storage 存储
 * @returns {boolean} 网格是否显示
 *
 * @example
 * ```js
 * readGridVisible({ getItem: () => null });
 * ```
 */
export function readGridVisible(storage) {
  if (!storage || typeof storage.getItem !== 'function') {
    return true;
  }
  try {
    return storage.getItem(GRID_VISIBLE_KEY) !== 'false';
  } catch {
    return true;
  }
}

/**
 * 记下当前选择。存储抛错时忽略，不打断绘制。
 *
 * @param {{ setItem: (key: string, value: string) => void } | null | undefined} storage 存储
 * @param {boolean} visible 网格是否显示
 *
 * @example
 * ```js
 * writeGridVisible({ setItem() {} }, true);
 * ```
 */
export function writeGridVisible(storage, visible) {
  if (!storage || typeof storage.setItem !== 'function') {
    return;
  }
  try {
    storage.setItem(GRID_VISIBLE_KEY, visible ? 'true' : 'false');
  } catch {
    // 隐私模式或配额满时只保留这一次会话里的选择。
  }
}

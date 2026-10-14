/**
 * 方向键微调的按键判断。
 *
 * 步长在引擎里：普通 1 个世界单位，Shift 为 10，与缩放无关。
 * 这里只决定方向和是否加速，不和空格平移、工具键、撤销抢按键。
 * 焦点在输入框、下拉框或 contenteditable 时返回 null，方向键留给文本光标。
 *
 * @example
 * ```js
 * arrowNudge({ key: 'ArrowRight', shiftKey: true });
 * // { direction: 'right', shift: true }
 * ```
 */

/** @type {Record<string, 'left' | 'right' | 'up' | 'down'>} */
const DIRECTIONS = {
  ArrowLeft: 'left',
  ArrowRight: 'right',
  ArrowUp: 'up',
  ArrowDown: 'down',
};

/**
 * 焦点是否在会消费方向键的控件里。
 *
 * @param {{ tagName?: string, isContentEditable?: boolean } | null | undefined} target
 * @returns {boolean}
 */
export function isTypingTarget(target) {
  if (!target || typeof target !== 'object') return false;
  const tag = typeof target.tagName === 'string' ? target.tagName.toUpperCase() : '';
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  return Boolean(target.isContentEditable);
}

/**
 * 这次按键要不要微调选中元素。
 *
 * Ctrl / Alt / Meta、输入框，以及非方向键都返回 null。
 * Shift 只加快步长，不挡住方向键。
 *
 * @param {{ key?: string, shiftKey?: boolean, ctrlKey?: boolean, altKey?: boolean, metaKey?: boolean, target?: { tagName?: string, isContentEditable?: boolean } | null } | null | undefined} event
 * @returns {{ direction: 'left' | 'right' | 'up' | 'down', shift: boolean } | null}
 */
export function arrowNudge(event) {
  if (!event || isTypingTarget(event.target)) return null;
  if (event.ctrlKey || event.metaKey || event.altKey) return null;
  const direction = DIRECTIONS[event.key];
  if (!direction) return null;
  return { direction, shift: Boolean(event.shiftKey) };
}

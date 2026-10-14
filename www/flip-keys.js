/**
 * 翻转和 90° 旋转的按键判断。
 *
 * Shift+H 水平翻转，Shift+V 垂直翻转，Shift+R 顺时针 90°，Shift+Alt+R 逆时针 90°。
 * 不带 Shift 的 H / V / R 仍是平移、选择、矩形工具，这里不拦截。
 * Ctrl / Cmd 不拦截，避免和浏览器的刷新（Cmd+R）以及撤销抢键。
 * 焦点在输入框、下拉框或 contenteditable 时返回 null。按住不重复，避免一次长按堆很多条撤销。
 *
 * #26 的快捷键帮助表面板还没进 master，所以这张表先放在 demo 里。
 * 工具栏和属性面板有同样的四个按钮。
 *
 * @example
 * ```js
 * orientAction({ key: 'h', shiftKey: true });
 * // { kind: 'flip', axis: 'horizontal' }
 * ```
 */

import { isTypingTarget } from './nudge-keys.js';

/**
 * 这次按键要不要翻转或旋转当前选中元素。
 *
 * @param {{ key?: string, shiftKey?: boolean, ctrlKey?: boolean, altKey?: boolean, metaKey?: boolean, repeat?: boolean, target?: { tagName?: string, isContentEditable?: boolean } | null } | null | undefined} event
 * @returns {{ kind: 'flip', axis: 'horizontal' | 'vertical' } | { kind: 'rotate', dir: 'cw' | 'ccw' } | null}
 */
export function orientAction(event) {
  if (!event || event.repeat || isTypingTarget(event.target)) return null;
  if (event.ctrlKey || event.metaKey || !event.shiftKey) return null;
  const key = typeof event.key === 'string' ? event.key.toLowerCase() : '';
  if (key === 'h' && !event.altKey) return { kind: 'flip', axis: 'horizontal' };
  if (key === 'v' && !event.altKey) return { kind: 'flip', axis: 'vertical' };
  if (key === 'r' && !event.altKey) return { kind: 'rotate', dir: 'cw' };
  if (key === 'r' && event.altKey) return { kind: 'rotate', dir: 'ccw' };
  return null;
}

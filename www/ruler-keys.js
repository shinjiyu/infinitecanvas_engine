/**
 * 标尺和手动参考线的按键判断。
 *
 * G 显示或隐藏标尺和参考线。Delete / Backspace 在选中参考线时删除它，
 * 引擎里没有选中参考线时这里仍返回 `delete-guide`，由调用方看返回值决定要不要拦截。
 * 输入框、下拉框和 contenteditable 不拦截。Ctrl / Cmd / Alt / Shift 不拦截。
 *
 * 首页按键分发读 `www/shortcuts.js` 的 `SHORTCUTS`。`G` 是 `toggleRulers`，
 * Delete / Backspace 仍是 `delete`，选中参考线时由页面先删参考线。
 * 这里保留同一套判断，供单测对照。
 *
 * @example
 * ```js
 * rulerKeyAction({ key: 'g' });
 * // 'toggle-rulers'
 * ```
 */

import { isTypingTarget } from './nudge-keys.js';

/**
 * 这次按键要不要切换标尺，或删除选中的参考线。
 *
 * @param {{ key?: string, shiftKey?: boolean, ctrlKey?: boolean, altKey?: boolean, metaKey?: boolean, repeat?: boolean, target?: { tagName?: string, isContentEditable?: boolean } | null } | null | undefined} event
 * @returns {'toggle-rulers' | 'delete-guide' | null}
 */
export function rulerKeyAction(event) {
  if (!event || event.repeat || isTypingTarget(event.target)) return null;
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return null;
  const key = typeof event.key === 'string' ? event.key : '';
  if (key === 'Delete' || key === 'Backspace') return 'delete-guide';
  if (key.toLowerCase() === 'g') return 'toggle-rulers';
  return null;
}

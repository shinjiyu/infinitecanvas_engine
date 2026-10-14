/**
 * 层级快捷键的按键判断。
 *
 * `]` 前移一层，`[` 后移一层，Ctrl/Cmd+] 置顶，Ctrl/Cmd+[ 置底。
 * 动作名交给引擎的 `reorder_selection`。这里不改场景。
 * 焦点在输入框、下拉框或 contenteditable 时返回 null。
 * Shift 或 Alt 不触发，避免和 `}` / `{` 以及其它组合键抢按键。
 *
 * #26 的快捷键帮助表面板还没进 master，所以这张表先放在 demo 里。
 *
 * @example
 * ```js
 * stackOrderAction({ key: ']', code: 'BracketRight' });
 * // 'forward'
 * ```
 */

import { isTypingTarget } from './nudge-keys.js';

/**
 * 这次按键要不要重排当前选中元素。
 *
 * @param {{ key?: string, code?: string, shiftKey?: boolean, ctrlKey?: boolean, altKey?: boolean, metaKey?: boolean, target?: { tagName?: string, isContentEditable?: boolean } | null } | null | undefined} event
 * @returns {'front' | 'back' | 'forward' | 'backward' | null}
 */
export function stackOrderAction(event) {
  if (!event || isTypingTarget(event.target)) return null;
  if (event.altKey || event.shiftKey) return null;
  let front = null;
  if (event.code === 'BracketRight') front = true;
  else if (event.code === 'BracketLeft') front = false;
  else if (event.key === ']') front = true;
  else if (event.key === '[') front = false;
  if (front == null) return null;
  const command = Boolean(event.ctrlKey || event.metaKey);
  if (front) return command ? 'front' : 'forward';
  return command ? 'back' : 'backward';
}

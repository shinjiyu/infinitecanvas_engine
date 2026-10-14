/**
 * 画布右键菜单的纯逻辑。
 *
 * 空白处和选中元素弹出不同的项。动作 id 交给 Demo，由 `dispatchShortcut`
 * 里已有的复制 / 粘贴 / 克隆 / 删除 / 成组 / 层级 / 全选 / 缩放函数执行，这里不改场景。
 * 键帽从 `SHORTCUTS` 用 `formatChord` 取出，和 `?` 帮助面板是同一张表。
 *
 * @example
 * ```js
 * contextMenuItems({ kind: 'canvas', clipboardHasContent: false, canGroup: false, canUngroup: false }, false);
 * // paste, select-all, zoom-fit
 * ```
 */

import { SHORTCUTS, formatChord, prefersMacModifier } from './shortcuts.js';

/**
 * 菜单项 id 对应快捷键表里的 handler。`order` 用来区分置顶 / 置底 / 前移 / 后移。
 *
 * @type {Record<string, { handler: string, order?: string }>}
 */
const MENU_HINT = {
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
 * 右键落点决定菜单种类。
 *
 * 没有命中时是空白菜单。命中已选中的元素，或它的某个祖先已选中时，保持当前选择。
 * 命中未选中的元素时，调用方应先单选它，再弹出选中菜单。
 *
 * @param {string | null | undefined} hitId 命中的元素 id。空白为 null
 * @param {string[]} selectedIds 当前选中 id
 * @param {(id: string) => string | null | undefined} parentOf 父 id。根返回 null
 * @returns {{ kind: 'canvas' | 'selection', selectHit: boolean }}
 */
export function resolveContextTarget(hitId, selectedIds, parentOf) {
  if (hitId == null || hitId === '') {
    return { kind: 'canvas', selectHit: false };
  }
  const selected = new Set(selectedIds || []);
  const seen = new Set();
  let current = hitId;
  while (current && !seen.has(current)) {
    if (selected.has(current)) {
      return { kind: 'selection', selectHit: false };
    }
    seen.add(current);
    current = parentOf(current);
  }
  return { kind: 'selection', selectHit: true };
}

/**
 * 从 `SHORTCUTS` 取出和菜单项对应的键帽。
 *
 * 同一动作有多条（例如 Delete 和 Backspace）时用 ` / ` 连起来。
 * 表里没有对应条目时返回 `undefined`，菜单就不显示键帽。
 *
 * @param {string} id 菜单项 id
 * @param {boolean} [mac] 是否按 Mac 显示。缺省跟随当前平台
 * @returns {string | undefined}
 *
 * @example
 * ```js
 * shortcutHint('copy', false); // Ctrl+C
 * ```
 */
export function shortcutHint(id, mac = prefersMacModifier()) {
  const spec = MENU_HINT[id];
  if (!spec) return undefined;
  const matches = SHORTCUTS.filter((shortcut) => {
    return (shortcut.phase ?? 'down') === 'down'
      && (shortcut.scope ?? 'canvas') === 'canvas'
      && shortcut.help !== false
      && shortcut.handler === spec.handler
      && (spec.order == null || shortcut.order === spec.order);
  });
  if (matches.length === 0) return undefined;
  return matches.map((shortcut) => formatChord(shortcut, mac)).join(' / ');
}

/**
 * 给一项带上快捷键表里的键帽。没有对应条目时不写 `hint`。
 *
 * @param {{ id: string, label: string }} item
 * @param {boolean} mac
 * @returns {{ id: string, label: string, hint?: string }}
 */
function withHint(item, mac) {
  const hint = shortcutHint(item.id, mac);
  return hint ? { ...item, hint } : item;
}

/**
 * 按落点和选择拼出菜单项。
 *
 * 选中菜单的粘贴只在剪贴板有文本时出现。成组至少两个选中才出现，
 * 解组只在唯一选中是分组时出现。空白菜单始终有粘贴、全选、缩放到适合。
 *
 * @param {{ kind: 'canvas' | 'selection', clipboardHasContent?: boolean, canGroup?: boolean, canUngroup?: boolean }} state
 * @param {boolean} [mac] 键帽是否按 Mac 显示
 * @returns {Array<{ id: string, label: string, hint?: string } | { type: 'separator' }>}
 */
export function contextMenuItems(state, mac = prefersMacModifier()) {
  if (state?.kind === 'selection') {
    /** @type {Array<{ id: string, label: string, hint?: string } | { type: 'separator' }>} */
    const items = [withHint({ id: 'copy', label: '复制' }, mac)];
    if (state.clipboardHasContent) {
      items.push(withHint({ id: 'paste', label: '粘贴' }, mac));
    }
    items.push(
      withHint({ id: 'duplicate', label: '克隆' }, mac),
      withHint({ id: 'delete', label: '删除' }, mac),
    );
    /** @type {Array<{ id: string, label: string, hint?: string }>} */
    const structure = [];
    if (state.canGroup) structure.push(withHint({ id: 'group', label: '成组' }, mac));
    if (state.canUngroup) structure.push(withHint({ id: 'ungroup', label: '解组' }, mac));
    if (structure.length > 0) {
      items.push({ type: 'separator' }, ...structure);
    }
    items.push(
      { type: 'separator' },
      withHint({ id: 'front', label: '置顶' }, mac),
      withHint({ id: 'back', label: '置底' }, mac),
      withHint({ id: 'forward', label: '前移' }, mac),
      withHint({ id: 'backward', label: '后移' }, mac),
    );
    return items;
  }
  return [
    withHint({ id: 'paste', label: '粘贴' }, mac),
    withHint({ id: 'select-all', label: '全选' }, mac),
    withHint({ id: 'zoom-fit', label: '缩放到适合' }, mac),
  ];
}

/**
 * 菜单打开时，这一下指针不应进入引擎，否则会开始平移或框选。
 *
 * @param {boolean} menuOpen 菜单是否开着
 * @returns {boolean} 为 true 时 Demo 才把指针事件交给引擎
 */
export function canvasPointerReachesEngine(menuOpen) {
  return !menuOpen;
}

/**
 * 把菜单限制在视口里。菜单比视口还大时贴着边距。
 *
 * @param {number} x 锚点横坐标（视口像素）
 * @param {number} y 锚点纵坐标
 * @param {number} width 菜单宽度
 * @param {number} height 菜单高度
 * @param {number} viewWidth 视口宽度
 * @param {number} viewHeight 视口高度
 * @param {number} [margin] 与视口边缘的间距
 * @returns {{ x: number, y: number }}
 */
export function clampMenuPosition(x, y, width, height, viewWidth, viewHeight, margin = 8) {
  const safeMargin = Number.isFinite(margin) ? margin : 0;
  const maxX = Math.max(safeMargin, viewWidth - width - safeMargin);
  const maxY = Math.max(safeMargin, viewHeight - height - safeMargin);
  const clamp = (value, max) => Math.min(Math.max(Number.isFinite(value) ? value : safeMargin, safeMargin), max);
  return { x: clamp(x, maxX), y: clamp(y, maxY) };
}

/**
 * 把菜单项画进容器。点击一项时调用 `onAction`。
 *
 * @param {HTMLElement} menu 菜单根节点
 * @param {Array<{ id?: string, label?: string, hint?: string, type?: string }>} items [`contextMenuItems`](contextMenuItems) 的结果
 * @param {(id: string) => void} onAction 选中某一项
 */
export function mountContextMenu(menu, items, onAction) {
  menu.replaceChildren();
  for (const item of items) {
    if (item.type === 'separator') {
      const separator = document.createElement('div');
      separator.className = 'context-menu-sep';
      separator.setAttribute('role', 'separator');
      menu.append(separator);
      continue;
    }
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'context-menu-item';
    button.dataset.action = item.id;
    button.setAttribute('role', 'menuitem');
    button.textContent = item.label || '';
    if (item.hint) {
      const hint = document.createElement('kbd');
      hint.textContent = item.hint;
      button.append(hint);
    }
    button.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      onAction(item.id);
    });
    menu.append(button);
  }
}

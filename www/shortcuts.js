/**
 * 首页 Demo 的快捷键表。
 *
 * 按键分发和 `?` 帮助面板都读这张表，避免两处各写一套。
 * 表里的 `tool` 是所属工具；不绑定某个工具的操作记为 `canvas`（画布）。
 *
 * @example
 * ```js
 * const hit = matchShortcut(SHORTCUTS, event, 'down');
 * const rows = helpEntries(SHORTCUTS, prefersMacModifier());
 * ```
 */

/** @typedef {'down' | 'up'} ShortcutPhase */

/**
 * 一条快捷键。
 *
 * @typedef {Object} Shortcut
 * @property {string} id 唯一 id
 * @property {string} action 帮助面板里的作用说明
 * @property {string} tool 所属工具：`select`、`rectangle`、`circle`、`line`、`pen`、`pan` 或 `canvas`
 * @property {string} handler 页面分发用的动作名。切换工具时为 `tool`，具体工具看 `tool`
 * @property {string} label 键帽文字，不含修饰键
 * @property {string[]} [keys] 可匹配的 `event.key`。单字符不区分大小写
 * @property {string} [code] 可匹配的 `event.code`，数字键用它避开布局差异
 * @property {boolean | null} [ctrl] 为 true 时需要 Ctrl 或 ⌘。缺省则这两个键都不能按下。`null` 表示忽略
 * @property {boolean} [shift] true 必须按下，false 必须没按，缺省则忽略
 * @property {boolean | null} [alt] 为 true 时需要 Alt。缺省则不能按下 Alt。`null` 表示忽略
 * @property {ShortcutPhase} [phase] 监听阶段，默认 `down`
 * @property {string} [scope] 作用域。缺省 `canvas`。文本编辑框里的快捷键是 `text-editor`
 * @property {string} [helpGroup] 合并到同一行帮助的组名。缺省用 `id`
 * @property {boolean} [help] 为 false 时不单独成行，但仍可并进 `helpGroup` 的键帽
 * @property {boolean} [allowRepeat] 按住不放时是否重复触发。默认允许
 */

/**
 * 所属工具在帮助面板里的名称。
 *
 * @type {Record<string, string>}
 */
export const TOOL_LABELS = {
  select: '选择',
  rectangle: '矩形',
  circle: '圆形',
  line: '直线',
  pen: '画笔',
  pan: '平移',
  text: '文本',
  canvas: '画布',
};

/**
 * 全部快捷键，顺序即帮助面板的行序。
 *
 * 已有的 V/L/P/R/O/H、空格临时平移、撤销重做、保存打开都留在这里。
 * 字母工具不要求 Shift 的状态，和原来的 `keydown` 一致：Shift+V 仍然切到选择。
 *
 * @type {Shortcut[]}
 */
export const SHORTCUTS = [
  {
    id: 'tool-select',
    action: '切换到选择工具',
    tool: 'select',
    handler: 'tool',
    label: 'V',
    keys: ['v'],
  },
  {
    id: 'tool-line',
    action: '切换到直线工具',
    tool: 'line',
    handler: 'tool',
    label: 'L',
    keys: ['l'],
  },
  {
    id: 'tool-pen',
    action: '切换到画笔工具',
    tool: 'pen',
    handler: 'tool',
    label: 'P',
    keys: ['p'],
  },
  {
    id: 'tool-rectangle',
    action: '切换到矩形工具',
    tool: 'rectangle',
    handler: 'tool',
    label: 'R',
    keys: ['r'],
  },
  {
    id: 'tool-circle',
    action: '切换到圆形工具',
    tool: 'circle',
    handler: 'tool',
    label: 'O',
    keys: ['o'],
  },
  {
    id: 'tool-text',
    action: '切换到文本工具',
    tool: 'text',
    handler: 'tool',
    label: 'T',
    keys: ['t'],
  },
  {
    id: 'tool-pan',
    action: '切换到平移工具',
    tool: 'pan',
    handler: 'tool',
    label: 'H',
    keys: ['h'],
  },
  {
    id: 'minimap',
    action: '显示或隐藏小地图',
    tool: 'canvas',
    handler: 'toggleMinimap',
    label: 'M',
    keys: ['m'],
  },
  {
    id: 'text-commit',
    action: '确认文本编辑',
    tool: 'text',
    handler: 'commitText',
    label: 'Enter',
    keys: ['Enter'],
    shift: false,
    scope: 'text-editor',
  },
  {
    id: 'text-cancel',
    action: '取消文本编辑',
    tool: 'text',
    handler: 'cancelText',
    label: 'Esc',
    keys: ['Escape', 'Esc'],
    scope: 'text-editor',
  },
  {
    id: 'temp-pan',
    action: '按住临时平移',
    tool: 'pan',
    handler: 'pushHand',
    label: 'Space',
    keys: [' ', 'Spacebar'],
    code: 'Space',
    // 原来的 keydown 在检查 Ctrl/Alt 之前就处理空格，所以按住修饰键也临时平移。
    ctrl: null,
    alt: null,
  },
  {
    id: 'temp-pan-up',
    action: '松开恢复上一个工具',
    tool: 'pan',
    handler: 'popHand',
    label: 'Space',
    keys: [' ', 'Spacebar'],
    code: 'Space',
    phase: 'up',
    help: false,
    ctrl: null,
    alt: null,
  },
  {
    id: 'undo',
    action: '撤销',
    tool: 'canvas',
    handler: 'undo',
    label: 'Z',
    keys: ['z'],
    ctrl: true,
    shift: false,
  },
  {
    id: 'redo-y',
    action: '重做',
    tool: 'canvas',
    handler: 'redo',
    label: 'Y',
    keys: ['y'],
    ctrl: true,
    helpGroup: 'redo',
  },
  {
    id: 'redo-shift-z',
    action: '重做',
    tool: 'canvas',
    handler: 'redo',
    label: 'Z',
    keys: ['z'],
    ctrl: true,
    shift: true,
    helpGroup: 'redo',
  },
  {
    id: 'delete',
    action: '删除选中',
    tool: 'canvas',
    handler: 'delete',
    label: 'Delete',
    keys: ['Delete'],
    helpGroup: 'delete',
  },
  {
    id: 'backspace',
    action: '删除选中',
    tool: 'canvas',
    handler: 'delete',
    label: 'Backspace',
    keys: ['Backspace'],
    helpGroup: 'delete',
  },
  {
    id: 'select-all',
    action: '全选',
    tool: 'canvas',
    handler: 'selectAll',
    label: 'A',
    keys: ['a'],
    ctrl: true,
  },
  {
    id: 'escape',
    action: '取消选择或关闭帮助面板',
    tool: 'canvas',
    handler: 'escape',
    label: 'Esc',
    keys: ['Escape', 'Esc'],
  },
  {
    id: 'zoom-fit',
    action: '缩放到适合',
    tool: 'canvas',
    handler: 'zoomFit',
    label: '1',
    keys: ['1', '!'],
    code: 'Digit1',
    shift: true,
  },
  {
    id: 'zoom-100',
    action: '缩放到 100%',
    tool: 'canvas',
    handler: 'zoom100',
    label: '0',
    keys: ['0', ')'],
    code: 'Digit0',
    shift: true,
  },
  {
    id: 'save',
    action: '保存场景 JSON',
    tool: 'canvas',
    handler: 'save',
    label: 'S',
    keys: ['s'],
    ctrl: true,
  },
  {
    id: 'open',
    action: '打开场景 JSON',
    tool: 'canvas',
    handler: 'open',
    label: 'O',
    keys: ['o'],
    ctrl: true,
  },
  {
    id: 'help',
    action: '打开或关闭快捷键帮助',
    tool: 'canvas',
    handler: 'toggleHelp',
    label: '/',
    keys: ['?', '/'],
    code: 'Slash',
    shift: true,
    allowRepeat: false,
  },
];

/**
 * 帮助面板的一行，由快捷键表归并得到。
 *
 * @typedef {Object} HelpEntry
 * @property {string} id 行 id，同一 `helpGroup` 只出现一次
 * @property {string} action 作用
 * @property {string} tool 所属工具 id
 * @property {string} toolLabel 所属工具的显示名
 * @property {string[]} chords 这一行要展示的键帽，含修饰键
 */

/**
 * 判断当前平台是否用 ⌘ 表示 Ctrl 类修饰键。
 *
 * @param {string} [platform] 平台字符串。缺省读 `navigator`
 * @returns {boolean} Mac、iPhone、iPad 为 true
 *
 * @example
 * ```js
 * prefersMacModifier('MacIntel');
 * ```
 */
export function prefersMacModifier(platform = defaultPlatform()) {
  return /mac|iphone|ipad|ipod/i.test(platform);
}

/**
 * 焦点是否在输入控件里。为 true 时画布快捷键不触发。
 *
 * 沿父节点查找 `input`、`textarea`，以及 `contenteditable` 不是 `false` 的元素。
 *
 * @param {EventTarget | null | undefined} target 事件目标
 * @returns {boolean} 处于可编辑焦点时为 true
 *
 * @example
 * ```js
 * if (isEditableTarget(event.target)) return;
 * ```
 */
export function isEditableTarget(target) {
  let node = target;
  while (node) {
    const tag = typeof node.tagName === 'string' ? node.tagName.toUpperCase() : '';
    if (tag === 'INPUT' || tag === 'TEXTAREA') {
      return true;
    }
    if (node.isContentEditable === true) {
      return true;
    }
    if (typeof node.getAttribute === 'function') {
      const value = node.getAttribute('contenteditable');
      if (value != null && String(value).toLowerCase() !== 'false') {
        return true;
      }
    }
    node = node.parentElement ?? null;
  }
  return false;
}

/**
 * 在快捷键表里找与事件匹配的一条。
 *
 * 修饰键约束更严的条目优先，因此 Ctrl+Shift+Z 是重做而不是撤销，
 * Ctrl+O 是打开文件而不是圆形工具。`keys` 与 `code` 同时存在时，命中任意一个即可。
 *
 * @param {readonly Shortcut[]} shortcuts 快捷键表
 * @param {KeyboardEvent | ShortcutEventLike} event 键盘事件
 * @param {ShortcutPhase} phase `down` 或 `up`
 * @param {string} [scope] 只匹配这个作用域，默认 `canvas`
 * @returns {Shortcut | null} 命中的条目；没有则为 null
 *
 * @example
 * ```js
 * const hit = matchShortcut(SHORTCUTS, event, 'down');
 * ```
 */
export function matchShortcut(shortcuts, event, phase, scope = 'canvas') {
  const hits = shortcuts.filter((shortcut) => {
    return (shortcut.scope ?? 'canvas') === scope
      && (shortcut.phase ?? 'down') === phase
      && chordMatches(shortcut, event);
  });
  if (hits.length === 0) {
    return null;
  }
  hits.sort((a, b) => specificity(b) - specificity(a));
  return hits[0];
}

/**
 * 把一条快捷键格式化成帮助面板上的键帽。
 *
 * Mac 使用 ⌘ / ⇧ / ⌥，单字符键直接拼在修饰符后面（⌘Z、⇧1）。
 * 其他平台使用 Ctrl / Shift / Alt，用加号连接（Ctrl+Z、Shift+1）。
 *
 * @param {Shortcut} shortcut 快捷键
 * @param {boolean} mac 是否按 Mac 显示
 * @returns {string} 键帽文字
 *
 * @example
 * ```js
 * formatChord(SHORTCUTS[0], false);
 * ```
 */
export function formatChord(shortcut, mac) {
  const parts = [];
  if (shortcut.ctrl) {
    parts.push(mac ? '⌘' : 'Ctrl');
  }
  if (shortcut.shift === true) {
    parts.push(mac ? '⇧' : 'Shift');
  }
  if (shortcut.alt) {
    parts.push(mac ? '⌥' : 'Alt');
  }
  parts.push(shortcut.label);
  const last = parts[parts.length - 1];
  if (mac) {
    return last.length === 1 ? parts.join('') : parts.join(' ');
  }
  return parts.join('+');
}

/**
 * 由快捷键表生成帮助面板的行。
 *
 * 同一 `helpGroup` 合并成一行，备用键（例如 Ctrl+Y 和 Ctrl+Shift+Z）都列出来。
 * `phase: 'up'` 的条目不进面板。
 *
 * @param {readonly Shortcut[]} shortcuts 快捷键表
 * @param {boolean} mac 是否按 Mac 显示修饰键
 * @returns {HelpEntry[]} 面板行，顺序与表中首次出现的顺序一致
 *
 * @example
 * ```js
 * const rows = helpEntries(SHORTCUTS, true);
 * ```
 */
export function helpEntries(shortcuts, mac) {
  /** @type {Map<string, Shortcut[]>} */
  const groups = new Map();
  for (const shortcut of shortcuts) {
    if ((shortcut.phase ?? 'down') === 'up') {
      continue;
    }
    const key = shortcut.helpGroup ?? shortcut.id;
    const list = groups.get(key);
    if (list) {
      list.push(shortcut);
    } else {
      groups.set(key, [shortcut]);
    }
  }

  /** @type {HelpEntry[]} */
  const rows = [];
  for (const [id, items] of groups) {
    const source = items.find((item) => item.help !== false);
    if (!source) {
      continue;
    }
    rows.push({
      id,
      action: source.action,
      tool: source.tool,
      toolLabel: TOOL_LABELS[source.tool] ?? source.tool,
      chords: items.map((item) => formatChord(item, mac)),
    });
  }
  return rows;
}

/**
 * Esc 的动作：帮助面板开着时先关掉它，否则取消选择。
 *
 * @param {boolean} helpOpen 帮助面板是否打开
 * @returns {'closeHelp' | 'clearSelection'} 这次 Esc 要做的事
 *
 * @example
 * ```js
 * escapeAction(true);
 * ```
 */
export function escapeAction(helpOpen) {
  return helpOpen ? 'closeHelp' : 'clearSelection';
}

/**
 * @typedef {Object} ShortcutEventLike
 * @property {string} [key]
 * @property {string} [code]
 * @property {boolean} [ctrlKey]
 * @property {boolean} [metaKey]
 * @property {boolean} [shiftKey]
 * @property {boolean} [altKey]
 * @property {boolean} [repeat]
 */

/**
 * @returns {string}
 */
function defaultPlatform() {
  if (typeof navigator === 'undefined') {
    return '';
  }
  const data = navigator.userAgentData;
  if (data && typeof data.platform === 'string' && data.platform) {
    return data.platform;
  }
  return navigator.platform || '';
}

/**
 * @param {Shortcut} shortcut
 * @param {ShortcutEventLike} event
 * @returns {boolean}
 */
function chordMatches(shortcut, event) {
  const ctrl = Boolean(event.ctrlKey || event.metaKey);
  const shift = Boolean(event.shiftKey);
  const alt = Boolean(event.altKey);
  if (shortcut.ctrl === true) {
    if (!ctrl) return false;
  } else if (shortcut.ctrl !== null && ctrl) {
    return false;
  }
  if (shortcut.alt === true) {
    if (!alt) return false;
  } else if (shortcut.alt !== null && alt) {
    return false;
  }
  if (shortcut.shift === true && !shift) return false;
  if (shortcut.shift === false && shift) return false;

  const keys = shortcut.keys ?? [];
  const keyHit = keys.some((candidate) => keyEquals(candidate, event.key));
  const codeHit = Boolean(shortcut.code) && event.code === shortcut.code;
  if (shortcut.code && keys.length > 0) {
    return keyHit || codeHit;
  }
  if (shortcut.code) {
    return codeHit;
  }
  return keyHit;
}

/**
 * @param {string} candidate
 * @param {string | undefined} eventKey
 * @returns {boolean}
 */
function keyEquals(candidate, eventKey) {
  if (typeof eventKey !== 'string') return false;
  if (candidate.length === 1 && eventKey.length === 1) {
    return candidate.toLowerCase() === eventKey.toLowerCase();
  }
  return candidate === eventKey;
}

/**
 * @param {Shortcut} shortcut
 * @returns {number}
 */
function specificity(shortcut) {
  let score = 0;
  if (shortcut.ctrl) score += 4;
  if (shortcut.shift === true) score += 2;
  if (shortcut.shift === false) score += 2;
  if (shortcut.alt) score += 2;
  if (shortcut.code) score += 1;
  return score;
}

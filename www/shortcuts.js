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
 * @property {'left' | 'right' | 'up' | 'down'} [direction] 方向键微调的方向。`handler` 为 `nudge` 时使用
 * @property {'front' | 'back' | 'forward' | 'backward'} [order] 层级重排动作。`handler` 为 `reorder` 时使用
 * @property {'horizontal' | 'vertical'} [axis] 翻转轴。`handler` 为 `flip` 时使用
 * @property {'cw' | 'ccw'} [dir] 90° 转向。`handler` 为 `rotate` 时使用
 * @property {string} [helpGroup] 合并到同一行帮助的组名。缺省用 `id`
 * @property {boolean} [help] 为 false 时不单独成行，但仍可并进 `helpGroup` 的键帽
 * @property {boolean} [allowRepeat] 按住不放时是否重复触发。默认允许
 * @property {boolean} [stealsBrowserZoom] 为 true 时会挡住浏览器缩放。只在画布/舞台聚焦时执行，并且只在那时 preventDefault
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
  eyedropper: '吸管',
  canvas: '画布',
};

/**
 * 全部快捷键，顺序即帮助面板的行序。
 *
 * 已有的 V/L/P/R/O/H、空格临时平移、撤销重做、保存打开、复制粘贴克隆和方向键微调都留在这里。
 * 不带修饰键的 G 显示或隐藏标尺和参考线。Ctrl+G 成组，Ctrl+Shift+G 取消分组，按修饰键区分。
 * Ctrl/Cmd+' 显示或隐藏背景网格，不占用 G。
 * 字母工具不要求 Shift 的状态。Shift+H / Shift+V / Shift+R 比工具键更具体，分别翻转或顺时针转 90°；
 * Shift+Alt+R 逆时针转 90°。不带 Shift 的 H / V / R 仍是平移、选择、矩形。
 * Shift+1 缩放到适合，Shift+2 缩放到选中，Shift+0 为 100%。数字键用 `code`，避开 Shift 改写 `key`。
 * Ctrl/Cmd+L 切换锁定，Ctrl/Cmd+Shift+H 切换隐藏。不带修饰键的 L / H 仍是直线和平移。
 * Ctrl/Cmd+= 或 Ctrl/Cmd++ 放大，Ctrl/Cmd+- 缩小，Ctrl/Cmd+0 回到 100%。倍数来自引擎的 `zoom_step`。
 * `I` 是吸管，而且必须不带 Shift、Ctrl 和 Alt。V / L / P / R / O / T / H / M / G 都不是 I，互不占用。
 * Shift+I、Ctrl/Cmd+I、Alt+I 不触发，避免以后的组合键和浏览器快捷键被吸管吃掉。
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
    id: 'tool-eyedropper',
    action: '吸管：单击吸取填充和描边',
    tool: 'eyedropper',
    handler: 'tool',
    label: 'I',
    keys: ['i'],
    // 其他字母工具忽略 Shift。吸管要求没按 Shift，这样 Shift+I 不会抢键。
    shift: false,
    allowRepeat: false,
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
    id: 'rulers',
    action: '显示或隐藏标尺和参考线',
    tool: 'canvas',
    handler: 'toggleRulers',
    label: 'G',
    keys: ['g'],
    shift: false,
    allowRepeat: false,
  },
  {
    id: 'toggle-grid',
    action: '显示或隐藏背景网格',
    tool: 'canvas',
    handler: 'toggleGrid',
    label: "'",
    keys: ["'"],
    code: 'Quote',
    ctrl: true,
    shift: false,
    allowRepeat: false,
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
    id: 'group',
    action: '将选中元素成组',
    tool: 'canvas',
    handler: 'group',
    label: 'G',
    keys: ['g'],
    ctrl: true,
    shift: false,
  },
  {
    id: 'ungroup',
    action: '取消分组',
    tool: 'canvas',
    handler: 'ungroup',
    label: 'G',
    keys: ['g'],
    ctrl: true,
    shift: true,
  },
  {
    id: 'copy',
    action: '复制选中',
    tool: 'canvas',
    handler: 'copy',
    label: 'C',
    keys: ['c'],
    ctrl: true,
    shift: false,
  },
  {
    id: 'paste',
    action: '粘贴',
    tool: 'canvas',
    handler: 'paste',
    label: 'V',
    keys: ['v'],
    ctrl: true,
    shift: false,
  },
  {
    id: 'duplicate',
    action: '原地克隆选中',
    tool: 'canvas',
    handler: 'duplicate',
    label: 'D',
    keys: ['d'],
    ctrl: true,
    shift: false,
  },
  {
    id: 'nudge-left',
    action: '微调选中元素（1 世界单位）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'left',
    label: '←',
    keys: ['ArrowLeft'],
    shift: false,
    helpGroup: 'nudge',
  },
  {
    id: 'nudge-right',
    action: '微调选中元素（1 世界单位）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'right',
    label: '→',
    keys: ['ArrowRight'],
    shift: false,
    helpGroup: 'nudge',
  },
  {
    id: 'nudge-up',
    action: '微调选中元素（1 世界单位）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'up',
    label: '↑',
    keys: ['ArrowUp'],
    shift: false,
    helpGroup: 'nudge',
  },
  {
    id: 'nudge-down',
    action: '微调选中元素（1 世界单位）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'down',
    label: '↓',
    keys: ['ArrowDown'],
    shift: false,
    helpGroup: 'nudge',
  },
  {
    id: 'nudge-left-fast',
    action: '微调选中元素（10 倍步长）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'left',
    label: '←',
    keys: ['ArrowLeft'],
    shift: true,
    helpGroup: 'nudge-fast',
  },
  {
    id: 'nudge-right-fast',
    action: '微调选中元素（10 倍步长）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'right',
    label: '→',
    keys: ['ArrowRight'],
    shift: true,
    helpGroup: 'nudge-fast',
  },
  {
    id: 'nudge-up-fast',
    action: '微调选中元素（10 倍步长）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'up',
    label: '↑',
    keys: ['ArrowUp'],
    shift: true,
    helpGroup: 'nudge-fast',
  },
  {
    id: 'nudge-down-fast',
    action: '微调选中元素（10 倍步长）',
    tool: 'canvas',
    handler: 'nudge',
    direction: 'down',
    label: '↓',
    keys: ['ArrowDown'],
    shift: true,
    helpGroup: 'nudge-fast',
  },
  {
    id: 'stack-forward',
    action: '前移一层',
    tool: 'canvas',
    handler: 'reorder',
    order: 'forward',
    label: ']',
    keys: [']'],
    code: 'BracketRight',
    shift: false,
  },
  {
    id: 'stack-backward',
    action: '后移一层',
    tool: 'canvas',
    handler: 'reorder',
    order: 'backward',
    label: '[',
    keys: ['['],
    code: 'BracketLeft',
    shift: false,
  },
  {
    id: 'stack-front',
    action: '置顶',
    tool: 'canvas',
    handler: 'reorder',
    order: 'front',
    label: ']',
    keys: [']'],
    code: 'BracketRight',
    ctrl: true,
    shift: false,
  },
  {
    id: 'stack-back',
    action: '置底',
    tool: 'canvas',
    handler: 'reorder',
    order: 'back',
    label: '[',
    keys: ['['],
    code: 'BracketLeft',
    ctrl: true,
    shift: false,
  },
  {
    id: 'flip-horizontal',
    action: '水平翻转',
    tool: 'canvas',
    handler: 'flip',
    axis: 'horizontal',
    label: 'H',
    keys: ['h'],
    shift: true,
    allowRepeat: false,
  },
  {
    id: 'flip-vertical',
    action: '垂直翻转',
    tool: 'canvas',
    handler: 'flip',
    axis: 'vertical',
    label: 'V',
    keys: ['v'],
    shift: true,
    allowRepeat: false,
  },
  {
    id: 'rotate-cw',
    action: '顺时针旋转 90°',
    tool: 'canvas',
    handler: 'rotate',
    dir: 'cw',
    label: 'R',
    keys: ['r'],
    shift: true,
    allowRepeat: false,
  },
  {
    id: 'rotate-ccw',
    action: '逆时针旋转 90°',
    tool: 'canvas',
    handler: 'rotate',
    dir: 'ccw',
    label: 'R',
    keys: ['r'],
    shift: true,
    alt: true,
    allowRepeat: false,
  },
  {
    id: 'toggle-lock',
    action: '锁定或解锁选中元素',
    tool: 'canvas',
    handler: 'toggleLock',
    label: 'L',
    keys: ['l'],
    ctrl: true,
    shift: false,
    allowRepeat: false,
  },
  {
    id: 'toggle-hide',
    action: '隐藏或显示选中元素',
    tool: 'canvas',
    handler: 'toggleHide',
    label: 'H',
    keys: ['h'],
    ctrl: true,
    shift: true,
    allowRepeat: false,
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
    id: 'zoom-in',
    action: '放大',
    tool: 'canvas',
    handler: 'zoomIn',
    label: '+',
    keys: ['+', '='],
    code: 'Equal',
    ctrl: true,
    stealsBrowserZoom: true,
  },
  {
    id: 'zoom-in-eq',
    action: '放大',
    tool: 'canvas',
    handler: 'zoomIn',
    label: '=',
    keys: ['='],
    code: 'Equal',
    ctrl: true,
    shift: false,
    help: false,
    helpGroup: 'zoom-in',
    stealsBrowserZoom: true,
  },
  {
    id: 'zoom-out',
    action: '缩小',
    tool: 'canvas',
    handler: 'zoomOut',
    label: '-',
    keys: ['-'],
    code: 'Minus',
    ctrl: true,
    shift: false,
    stealsBrowserZoom: true,
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
    id: 'zoom-selection',
    action: '缩放到选中',
    tool: 'canvas',
    handler: 'zoomSelection',
    label: '2',
    keys: ['2', '@'],
    code: 'Digit2',
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
    id: 'zoom-reset',
    action: '缩放到 100%',
    tool: 'canvas',
    handler: 'zoom100',
    label: '0',
    keys: ['0'],
    code: 'Digit0',
    ctrl: true,
    shift: false,
    helpGroup: 'zoom-100',
    stealsBrowserZoom: true,
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
 * 把引擎缩放倍数格式化成页脚上的百分比。
 *
 * 与 `get_zoom()` 对齐，四舍五入到最接近的整数。`1` 显示为 `100%`。
 * 滚轮和键盘改的是同一个相机，所以页脚每一帧读一次就能两边都跟上。
 *
 * @param {number} zoom 视口缩放倍数
 * @returns {string} 例如 `100%`、`110%`
 *
 * @example
 * ```js
 * formatZoomPercent(1);
 * ```
 */
export function formatZoomPercent(zoom) {
  if (typeof zoom !== 'number' || !Number.isFinite(zoom)) return '100%';
  return `${Math.round(zoom * 100)}%`;
}

/**
 * 焦点是否在画布舞台上，或还没有落到具体控件。
 *
 * 画布通常不可聚焦，点在画布上时焦点落在 `body`。
 * 按钮、输入框、下拉框和图层面板返回 false，键盘缩放不会抢走浏览器缩放。
 *
 * @param {EventTarget | null | undefined} target 事件目标
 * @returns {boolean} 可以接管 Ctrl/Cmd 缩放时为 true
 *
 * @example
 * ```js
 * isCanvasStageFocus({ tagName: 'CANVAS', id: 'canvas' });
 * ```
 */
export function isCanvasStageFocus(target) {
  if (!target || typeof target !== 'object') return false;
  const tag = typeof target.tagName === 'string' ? target.tagName.toUpperCase() : '';
  if (tag === 'BODY' || tag === 'HTML') return true;
  const id = typeof target.id === 'string' ? target.id : '';
  return id === 'stage' || id === 'canvas' || id === 'guides';
}

/**
 * 这次快捷键是否应该执行。
 *
 * 会挡住浏览器缩放的和弦只在 [`isCanvasStageFocus`] 为 true 时执行。
 * 输入框里一律不执行。其他快捷键只避开可编辑控件。
 *
 * @param {Shortcut | null | undefined} shortcut 命中的快捷键
 * @param {EventTarget | null | undefined} target 事件目标
 * @returns {boolean} 应该执行并允许 preventDefault 时为 true
 *
 * @example
 * ```js
 * shouldApplyShortcut(SHORTCUTS.find((item) => item.id === 'zoom-in'), { tagName: 'BODY' });
 * ```
 */
export function shouldApplyShortcut(shortcut, target) {
  if (!shortcut || isEditableTarget(target)) return false;
  if (shortcut.stealsBrowserZoom) return isCanvasStageFocus(target);
  return true;
}

/**
 * 焦点是否在输入控件里。为 true 时画布快捷键不触发。
 *
 * 沿父节点查找 `input`、`textarea`、`select`，以及 `contenteditable` 不是 `false` 的元素。
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
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
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

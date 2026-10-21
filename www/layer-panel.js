/**
 * 图层面板。
 *
 * 从 `get_scene_tree` 读出场景树，虚拟滚动只挂上视口里的行。
 * 点击行选中画布元素，锁定的行也可以选中，但不能拖拽。画布选中反向高亮。
 * 拖拽改兄弟顺序或移入/移出组，走 `reorder_element`（Command，可撤销）。
 * 眼睛和锁调用 `set_element_visible` / `set_element_locked`，和快捷键同一条可撤销命令。
 * 双击名称内联重命名：Enter 提交、Esc 取消；走 `rename_element`（可撤销）。
 * 多选时仍可双击某一行改该行；属性面板仅单选时编辑。
 *
 * 兄弟列表在引擎里从后往前（下标 0 是最底层）。面板把每一层反过来画，
 * 最上面的一行是最前面的元素。
 *
 * @example
 * ```js
 * const panel = attachLayerPanel(engine, {
 *   onEdit() {},
 *   setStatus(text) {},
 * });
 * panel.sync();
 * ```
 */

/** 一行的像素高度。虚拟滚动用它把 scrollTop 换成行号。 */
export const ROW_HEIGHT = 28;

/** 视口上下多渲染的行数，避免快速滚动时露出空白。 */
export const OVERSCAN = 8;

const TYPE_LABELS = {
  Rectangle: '矩形',
  Circle: '圆形',
  Polygon: '多边形',
  Star: '星形',
  Image: '图片',
  Line: '直线',
  Path: '路径',
  Text: '文本',
  Group: '分组',
  Frame: '画板',
};

/**
 * 组和画板都能收纳子元素。解组仍然只针对组。
 *
 * @param {string | undefined} elementType
 * @returns {boolean}
 */
function isContainerType(elementType) {
  return elementType === 'Group' || elementType === 'Frame';
}

/**
 * 有自定义名称时显示名称；否则显示类型默认名 + id。
 * 画板未清空前引擎会带上 Frame::name（缺省 "Frame"）。
 *
 * @param {{ elementType?: string, name?: string }} node
 * @param {string} id
 * @returns {string}
 */
export function rowLabel(node, id) {
  const name = typeof node.name === 'string' ? node.name.trim() : '';
  if (name) return name;
  const type = TYPE_LABELS[node.elementType] || node.elementType || '元素';
  return `${type} ${id}`;
}

/**
 * 内联编辑时的初始文本：自定义名，或当前行上显示的类型默认标签。
 *
 * @param {object} row flattenTree 行
 * @returns {string}
 */
export function renameDraft(row) {
  if (!row) return '';
  const node = row;
  const name = typeof node.name === 'string' ? node.name.trim() : '';
  if (name) return name;
  // flatten 行把 label 已经算好；编辑框用当前可见文案更直观
  if (typeof row.label === 'string' && row.label.trim()) return row.label.trim();
  return '';
}

/**
 * 把元素 ID 收成稳定的字符串键。WASM 的 u64 在 JS 里是 bigint。
 *
 * @param {bigint | number | string | null | undefined} id
 * @returns {string}
 */
export function idKey(id) {
  if (id == null) return '';
  return typeof id === 'bigint' ? id.toString() : String(id);
}

/**
 * 传给 WASM u64 参数的 ID。
 *
 * @param {bigint | number | string} id
 * @returns {bigint}
 */
export function wasmId(id) {
  if (typeof id === 'bigint') return id;
  if (typeof id === 'number' && Number.isFinite(id)) return BigInt(Math.trunc(id));
  return BigInt(id);
}

/**
 * 建立 id → 节点、id → 父 id 的索引，折叠的组也算在内。
 *
 * @param {Array<object>} roots `get_scene_tree().roots`
 * @returns {{ byId: Map<string, object>, parentOf: Map<string, string | null> }}
 */
export function indexTree(roots) {
  /** @type {Map<string, object>} */
  const byId = new Map();
  /** @type {Map<string, string | null>} */
  const parentOf = new Map();
  const walk = (nodes, parentKey) => {
    for (const node of Array.from(nodes || [])) {
      const id = idKey(node.id);
      byId.set(id, node);
      parentOf.set(id, parentKey);
      if (node.children && node.children.length) walk(node.children, id);
    }
  };
  walk(roots, null);
  return { byId, parentOf };
}

/**
 * 把场景树摊成面板行。组可折叠；顺序是从前往后（最上面的一行 z 最大）。
 *
 * @param {Array<object>} roots
 * @param {Set<string>} [collapsed] 折叠的组 id
 * @returns {Array<object>}
 */
export function flattenTree(roots, collapsed) {
  /** @type {Array<object>} */
  const rows = [];
  const walk = (nodes, parentKey, rawParentId, depth, ancestorLocked) => {
    const list = Array.from(nodes || []);
    const display = list.slice().reverse();
    const count = list.length;
    display.forEach((node, visualIndex) => {
      const id = idKey(node.id);
      rows.push({
        id,
        rawId: node.id,
        parentKey,
        rawParentId,
        depth,
        elementType: node.elementType,
        name: typeof node.name === 'string' ? node.name : '',
        visible: node.visible !== false,
        locked: Boolean(node.locked),
        ancestorLocked: Boolean(ancestorLocked),
        storageIndex: count - 1 - visualIndex,
        siblingCount: count,
        label: rowLabel(node, id),
      });
      const childLocked = ancestorLocked || Boolean(node.locked);
      if (
        isContainerType(node.elementType)
        && node.children
        && node.children.length
        && !(collapsed && collapsed.has(id))
      ) {
        walk(node.children, id, node.id, depth + 1, childLocked);
      }
    });
  };
  walk(roots, null, null, 0, false);
  return rows;
}

/**
 * 虚拟滚动窗口。返回 `[start, end)`，含 overscan。
 *
 * @param {number} scrollTop
 * @param {number} viewHeight
 * @param {number} total
 * @param {number} [rowHeight]
 * @param {number} [overscan]
 * @returns {{ start: number, end: number }}
 */
export function visibleRange(scrollTop, viewHeight, total, rowHeight = ROW_HEIGHT, overscan = OVERSCAN) {
  if (total <= 0) return { start: 0, end: 0 };
  const height = Math.max(rowHeight, viewHeight);
  let start = Math.floor(Math.max(0, scrollTop) / rowHeight) - overscan;
  if (start < 0) start = 0;
  let end = Math.ceil((Math.max(0, scrollTop) + height) / rowHeight) + overscan;
  if (end > total) end = total;
  if (start > end) start = end;
  return { start, end };
}

/**
 * 计算一次拖放的目标。`parentId` 为 0 表示根；`index` 是摘下之后的存储下标。
 *
 * 行号小于 0 表示放到根列表的最前面。`place` 为 `before` / `after` / `into`。
 * 锁定、成环时返回 null。
 *
 * @param {Array<object>} rows
 * @param {number} rowIndex
 * @param {'before' | 'after' | 'into'} place
 * @param {string} sourceId
 * @param {{ byId: Map<string, object>, parentOf: Map<string, string | null> }} tree
 * @returns {{ parentId: bigint | number, index: number } | null}
 */
export function dropIndex(rows, rowIndex, place, sourceId, tree) {
  if (!sourceId || !tree?.byId?.has(sourceId)) return null;
  if (nodeLocked(tree, sourceId) || ancestorLocked(tree, sourceId)) return null;

  if (rowIndex < 0) {
    return { parentId: 0, index: rootFrontIndex(rows, sourceId, tree) };
  }
  if (!rows.length) return { parentId: 0, index: 0 };
  if (rowIndex >= rows.length) {
    return dropIndex(rows, rows.length - 1, 'after', sourceId, tree);
  }

  const row = rows[rowIndex];
  if (place === 'into') {
    if (!isContainerType(row.elementType) || row.locked || row.ancestorLocked) return null;
    if (row.id === sourceId || isInside(tree, sourceId, row.id)) return null;
    const kids = Array.from(tree.byId.get(row.id)?.children || []);
    const removed = kids.some((child) => idKey(child.id) === sourceId);
    return { parentId: row.rawId, index: kids.length - (removed ? 1 : 0) };
  }

  if (row.ancestorLocked) return null;
  const parentKey = row.parentKey;
  if (parentKey && (parentKey === sourceId || isInside(tree, sourceId, parentKey))) return null;

  const sameParent = tree.parentOf.get(sourceId) === parentKey;
  let storageIndex = row.storageIndex;
  if (sameParent) {
    const sourceRow = rows.find((item) => item.id === sourceId);
    if (sourceRow && sourceRow.storageIndex < row.storageIndex) storageIndex -= 1;
  }
  const index = place === 'before' ? storageIndex + 1 : storageIndex;
  const parentId = parentKey == null ? 0 : tree.byId.get(parentKey).id;
  return { parentId, index };
}

/**
 * @param {{ byId: Map<string, object> }} tree
 * @param {string} id
 */
function nodeLocked(tree, id) {
  return Boolean(tree.byId.get(id)?.locked);
}

/**
 * @param {{ byId: Map<string, object>, parentOf: Map<string, string | null> }} tree
 * @param {string} id
 */
function ancestorLocked(tree, id) {
  let current = tree.parentOf.get(id);
  while (current) {
    if (nodeLocked(tree, current)) return true;
    current = tree.parentOf.get(current);
  }
  return false;
}

/**
 * `nodeId` 是否是 `ancestorId` 自己或它的子孙。
 *
 * @param {{ parentOf: Map<string, string | null> }} tree
 * @param {string} ancestorId
 * @param {string | null | undefined} nodeId
 */
function isInside(tree, ancestorId, nodeId) {
  let current = nodeId;
  while (current) {
    if (current === ancestorId) return true;
    current = tree.parentOf.get(current);
  }
  return false;
}

/**
 * @param {Array<object>} rows
 * @param {string} sourceId
 * @param {{ parentOf: Map<string, string | null> }} tree
 */
function rootFrontIndex(rows, sourceId, tree) {
  const rootRow = rows.find((row) => row.depth === 0);
  const count = rootRow ? rootRow.siblingCount : 0;
  const sourceIsRoot = tree.parentOf.get(sourceId) == null;
  return sourceIsRoot ? Math.max(0, count - 1) : count;
}

/**
 * 把当前选中收成一组，并选中新组。
 *
 * 图层面板「成组」走这里，调用 `group_elements`。
 * 少于两个、不同父节点或锁定时失败，场景不变。
 *
 * @param {{ get_selected_ids?: () => Array<bigint | number>, group_elements: (ids: unknown[]) => bigint | number, select_element: (id: bigint, additive: boolean) => boolean }} engine
 * @param {{ setStatus?: (text: string, isError?: boolean) => void }} [hooks]
 * @returns {boolean} 是否成组成功
 */
export function groupSelected(engine, hooks = {}) {
  const ids = Array.from(engine.get_selected_ids?.() || []);
  if (ids.length < 2) {
    hooks.setStatus?.('成组至少选择两个元素', true);
    return false;
  }
  const groupId = engine.group_elements(ids);
  if (groupId && groupId !== 0n && groupId !== 0) {
    engine.select_element(wasmId(groupId), false);
    hooks.setStatus?.('已成组');
    return true;
  }
  hooks.setStatus?.('无法成组：需要同一父节点下至少两个未锁定元素', true);
  return false;
}

/**
 * 解散当前唯一选中的分组。
 *
 * 图层面板「解组」走这里，调用 `ungroup`。
 *
 * @param {{ get_selected_ids?: () => Array<bigint | number>, get_element_info?: (id: bigint) => { type?: string } | null, ungroup: (id: bigint) => boolean }} engine
 * @param {{ setStatus?: (text: string, isError?: boolean) => void }} [hooks]
 * @returns {boolean} 是否解组成功
 */
export function ungroupSelected(engine, hooks = {}) {
  const ids = Array.from(engine.get_selected_ids?.() || []);
  if (ids.length !== 1) {
    hooks.setStatus?.('解组请只选择一个分组', true);
    return false;
  }
  const info = engine.get_element_info?.(wasmId(ids[0]));
  if (!info || info.type !== 'Group') {
    hooks.setStatus?.('当前选中的不是分组', true);
    return false;
  }
  if (engine.ungroup(wasmId(ids[0]))) {
    hooks.setStatus?.('已解组');
    return true;
  }
  hooks.setStatus?.('无法解组', true);
  return false;
}

/**
 * 挂上图层面板。页面里需要 `#layer-list`、`#layer-meta` 和成组/解组/压测按钮。
 *
 * @param {object} engine WASM `InfiniteCanvas`
 * @param {{ onEdit?: () => void, setStatus?: (text: string, isError?: boolean) => void }} [hooks]
 * @returns {{ sync: (force?: boolean) => { treeMs: number, paintMs: number, domRows: number, rows: number } }}
 */
export function attachLayerPanel(engine, hooks = {}) {
  const viewport = document.getElementById('layer-list');
  const meta = document.getElementById('layer-meta');
  if (!(viewport instanceof HTMLElement)) {
    return {
      sync() {
        return { treeMs: 0, paintMs: 0, domRows: 0, rows: 0 };
      },
    };
  }

  const onEdit = hooks.onEdit || (() => {});
  const setStatus = hooks.setStatus || (() => {});
  const collapsed = new Set();
  const spacer = document.createElement('div');
  spacer.className = 'layer-spacer';
  const windowEl = document.createElement('div');
  windowEl.className = 'layer-window';
  const indicator = document.createElement('div');
  indicator.className = 'layer-drop';
  indicator.hidden = true;
  viewport.append(spacer, windowEl, indicator);

  /** @type {Array<object>} */
  let roots = [];
  /** @type {Array<object>} */
  let rows = [];
  let tree = indexTree(roots);
  let epoch = '';
  let treeMs = 0;
  let paintMs = 0;
  let domRows = 0;
  /** @type {HTMLElement[]} */
  const pool = [];
  /** @type {Set<string>} */
  let selected = new Set();
  let selectionKey = '';
  let selectionFromPanel = false;
  /** @type {{ id: string, pointerId: number, startX: number, startY: number, active: boolean } | null} */
  let drag = null;
  /** @type {{ id: string, input: HTMLInputElement } | null} */
  let renaming = null;

  const panel = {
    /**
     * 结构版本或显隐/锁定版本变了就重读场景树。选中变化只改高亮。
     *
     * @param {boolean} [force] 忽略版本，强制重读
     */
    sync(force = false) {
      const structure = typeof engine.get_structure_epoch === 'function' ? engine.get_structure_epoch() : 0;
      const flags = typeof engine.get_flags_epoch === 'function' ? engine.get_flags_epoch() : 0;
      const nextEpoch = `${structure}:${flags}`;
      if (force || nextEpoch !== epoch) {
        const started = performance.now();
        const snapshot = engine.get_scene_tree();
        treeMs = performance.now() - started;
        epoch = nextEpoch;
        roots = Array.from(snapshot?.roots || []);
        tree = indexTree(roots);
        rows = flattenTree(roots, collapsed);
        const paintStarted = performance.now();
        render();
        paintMs = performance.now() - paintStarted;
        if (meta) meta.textContent = `${rows.length.toLocaleString()} 项`;
      }
      syncSelection();
      return { treeMs, paintMs, domRows, rows: rows.length };
    },
  };

  document.getElementById('layer-group-btn')?.addEventListener('click', () => {
    if (!groupSelected(engine, { setStatus })) return;
    selectionFromPanel = true;
    onEdit();
    panel.sync(true);
  });

  document.getElementById('layer-ungroup-btn')?.addEventListener('click', () => {
    if (!ungroupSelected(engine, { setStatus })) return;
    onEdit();
    panel.sync(true);
  });

  document.getElementById('layer-bench-btn')?.addEventListener('click', () => {
    const loadStarted = performance.now();
    engine.load_stress_scene(10000);
    const loadMs = performance.now() - loadStarted;
    const timings = panel.sync(true);
    const message = `1万层：载入 ${loadMs.toFixed(1)} ms，场景树 ${timings.treeMs.toFixed(1)} ms，列表 ${timings.paintMs.toFixed(1)} ms，DOM ${timings.domRows} 行 / ${timings.rows} 项`;
    console.info(message);
    if (meta) meta.textContent = message;
    setStatus(message);
    onEdit();
  });

  viewport.addEventListener('scroll', () => render(), { passive: true });

  viewport.addEventListener('pointerdown', (event) => {
    if (!(event instanceof PointerEvent) || event.button !== 0) return;
    if (event.target instanceof Element && event.target.closest('button')) return;
    if (event.target instanceof HTMLInputElement) return;
    if (renaming) cancelRename(true);
    const rowEl = event.target instanceof Element ? event.target.closest('.layer-row') : null;
    if (!(rowEl instanceof HTMLElement)) return;
    const index = Number(rowEl.dataset.index);
    const row = rows[index];
    if (!row) {
      drag = null;
      return;
    }
    if (row.locked || row.ancestorLocked) {
      drag = null;
      selectRow(row.id, event.shiftKey || event.metaKey || event.ctrlKey);
      return;
    }
    drag = {
      id: row.id,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
    };
    viewport.setPointerCapture(event.pointerId);
  });

  viewport.addEventListener('pointermove', (event) => {
    if (!(event instanceof PointerEvent) || !drag || event.pointerId !== drag.pointerId) return;
    const dx = event.clientX - drag.startX;
    const dy = event.clientY - drag.startY;
    if (!drag.active && dx * dx + dy * dy < 16) return;
    drag.active = true;
    const rect = viewport.getBoundingClientRect();
    if (event.clientY < rect.top + 28) viewport.scrollTop -= 14;
    if (event.clientY > rect.bottom - 28) viewport.scrollTop += 14;
    showDrop(event.clientY);
  });

  viewport.addEventListener('pointerup', (event) => {
    if (!(event instanceof PointerEvent) || !drag || event.pointerId !== drag.pointerId) return;
    const current = drag;
    drag = null;
    indicator.hidden = true;
    clearDropInto();
    if (viewport.hasPointerCapture(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
    if (!current.active) {
      selectRow(current.id, event.shiftKey || event.metaKey || event.ctrlKey);
      return;
    }
    const hit = hitPlace(event.clientY);
    const target = dropIndex(rows, hit.rowIndex, hit.place, current.id, tree);
    if (!target) return;
    const source = rows.find((row) => row.id === current.id);
    if (!source) return;
    const parentId = target.parentId === 0 ? 0n : wasmId(target.parentId);
    const moved = engine.reorder_element(wasmId(source.rawId), parentId, target.index);
    if (!moved) return;
    if (hit.place === 'into' && hit.rowIndex >= 0 && hit.rowIndex < rows.length) {
      collapsed.delete(rows[hit.rowIndex].id);
    }
    onEdit();
    panel.sync(true);
  });

  viewport.addEventListener('click', (event) => {
    const button = event.target instanceof Element ? event.target.closest('button') : null;
    if (!(button instanceof HTMLButtonElement)) return;
    const rowEl = button.closest('.layer-row');
    if (!(rowEl instanceof HTMLElement)) return;
    const row = rows[Number(rowEl.dataset.index)];
    if (!row) return;
    event.stopPropagation();
    const action = button.dataset.act;
    if (action === 'twist') {
      if (collapsed.has(row.id)) collapsed.delete(row.id);
      else collapsed.add(row.id);
      rows = flattenTree(roots, collapsed);
      render();
      paintSelection();
      return;
    }
    if (action === 'visible') {
      const next = !row.visible;
      engine.set_element_visible(wasmId(row.rawId), next);
      const node = tree.byId.get(row.id);
      if (node) node.visible = next;
      rows = flattenTree(roots, collapsed);
      render();
      paintSelection();
      onEdit();
      return;
    }
    if (action === 'lock') {
      const next = !row.locked;
      engine.set_element_locked(wasmId(row.rawId), next);
      const node = tree.byId.get(row.id);
      if (node) node.locked = next;
      rows = flattenTree(roots, collapsed);
      onEdit();
      panel.sync(true);
    }
  });

  viewport.addEventListener('dblclick', (event) => {
    if (!(event instanceof MouseEvent)) return;
    if (event.target instanceof Element && event.target.closest('button')) return;
    if (event.target instanceof HTMLInputElement) return;
    const rowEl = event.target instanceof Element ? event.target.closest('.layer-row') : null;
    if (!(rowEl instanceof HTMLElement)) return;
    const row = rows[Number(rowEl.dataset.index)];
    if (!row || row.locked || row.ancestorLocked) return;
    event.preventDefault();
    event.stopPropagation();
    beginRename(row, rowEl);
  });

  /**
   * @param {object} row
   * @param {HTMLElement} rowEl
   */
  function beginRename(row, rowEl) {
    if (typeof engine.rename_element !== 'function') return;
    cancelRename(false);
    const label = rowEl.querySelector('.label');
    if (!(label instanceof HTMLElement)) return;
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'layer-rename';
    input.value = renameDraft(row);
    input.setAttribute('aria-label', '重命名');
    label.replaceChildren(input);
    renaming = { id: row.id, input };
    input.focus();
    input.select();
    input.addEventListener('keydown', (event) => {
      if (!(event instanceof KeyboardEvent)) return;
      if (event.key === 'Enter') {
        event.preventDefault();
        event.stopPropagation();
        commitRename(row);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        cancelRename(true);
      }
    });
    input.addEventListener('blur', () => {
      if (!renaming || renaming.input !== input) return;
      commitRename(row);
    });
  }

  /**
   * @param {object} row
   */
  function commitRename(row) {
    if (!renaming || renaming.id !== row.id) return;
    const next = renaming.input.value;
    renaming = null;
    if (typeof engine.rename_element !== 'function') {
      panel.sync(true);
      return;
    }
    const changed = engine.rename_element(wasmId(row.rawId), next);
    if (changed) {
      onEdit();
      setStatus('已重命名');
    }
    panel.sync(true);
  }

  /**
   * @param {boolean} repaint
   */
  function cancelRename(repaint) {
    if (!renaming) return;
    renaming = null;
    if (repaint) panel.sync(true);
  }

  /**
   * @returns {Array<bigint | number>}
   */
  function selectedList() {
    return Array.from(engine.get_selected_ids?.() || []);
  }

  /**
   * @param {string} id
   * @param {boolean} additive
   */
  function selectRow(id, additive) {
    const row = rows.find((item) => item.id === id);
    if (!row) return;
    selectionFromPanel = true;
    const changed = engine.select_element(wasmId(row.rawId), additive);
    if (!changed && !additive) return;
    syncSelection();
  }

  function syncSelection() {
    const ids = selectedList().map(idKey);
    const signature = ids.slice().sort().join(',');
    const changed = signature !== selectionKey;
    selectionKey = signature;
    selected = new Set(ids);
    if (changed && !selectionFromPanel && ids.length) revealSelection(ids);
    selectionFromPanel = false;
    paintSelection();
  }

  /**
   * @param {string[]} ids
   */
  function revealSelection(ids) {
    let expanded = false;
    for (const id of ids) {
      let parent = tree.parentOf.get(id);
      while (parent) {
        if (collapsed.delete(parent)) expanded = true;
        parent = tree.parentOf.get(parent);
      }
    }
    if (expanded) {
      rows = flattenTree(roots, collapsed);
      render();
    }
    const index = rows.findIndex((row) => ids.includes(row.id));
    if (index < 0) return;
    const top = index * ROW_HEIGHT;
    const view = viewport.clientHeight;
    if (top < viewport.scrollTop || top + ROW_HEIGHT > viewport.scrollTop + view) {
      viewport.scrollTop = Math.max(0, top - view / 2);
    }
  }

  function render() {
    const total = rows.length;
    spacer.style.height = `${total * ROW_HEIGHT}px`;
    const range = visibleRange(viewport.scrollTop, viewport.clientHeight || ROW_HEIGHT, total);
    const count = range.end - range.start;
    windowEl.style.transform = `translateY(${range.start * ROW_HEIGHT}px)`;
    while (pool.length < count) pool.push(createRow());
    while (pool.length > count) {
      const extra = pool.pop();
      extra?.remove();
    }
    for (let index = 0; index < count; index += 1) {
      bindRow(pool[index], rows[range.start + index], range.start + index);
    }
    domRows = count;
    paintSelection();
  }

  function createRow() {
    const row = document.createElement('div');
    row.className = 'layer-row';
    row.setAttribute('role', 'treeitem');
    const twist = document.createElement('button');
    twist.type = 'button';
    twist.className = 'twist';
    twist.dataset.act = 'twist';
    const label = document.createElement('span');
    label.className = 'label';
    const eye = document.createElement('button');
    eye.type = 'button';
    eye.className = 'icon';
    eye.dataset.act = 'visible';
    const lock = document.createElement('button');
    lock.type = 'button';
    lock.className = 'icon';
    lock.dataset.act = 'lock';
    row.append(twist, label, eye, lock);
    windowEl.append(row);
    return row;
  }

  /**
   * @param {HTMLElement} element
   * @param {object} row
   * @param {number} index
   */
  function bindRow(element, row, index) {
    element.dataset.index = String(index);
    element.dataset.id = row.id;
    element.style.paddingLeft = `${6 + row.depth * 14}px`;
    element.classList.toggle('hidden-item', !row.visible || ancestorHidden(row));
    element.classList.toggle('locked', row.locked || row.ancestorLocked);
    const twist = element.querySelector('.twist');
    const label = element.querySelector('.label');
    const eye = element.querySelector('[data-act="visible"]');
    const lock = element.querySelector('[data-act="lock"]');
    const expandable = isContainerType(row.elementType) && tree.byId.get(row.id)?.children?.length;
    if (twist instanceof HTMLButtonElement) {
      twist.hidden = !expandable;
      twist.textContent = collapsed.has(row.id) ? '▸' : '▾';
      twist.setAttribute('aria-label', collapsed.has(row.id) ? '展开' : '折叠');
    }
    if (label) {
      if (renaming && renaming.id === row.id && label.contains(renaming.input)) {
        // keep the active input
      } else {
        label.textContent = row.label;
      }
    }
    if (eye instanceof HTMLButtonElement) {
      eye.textContent = row.visible ? '●' : '○';
      eye.setAttribute('aria-label', row.visible ? '隐藏' : '显示');
      eye.setAttribute('aria-pressed', row.visible ? 'true' : 'false');
    }
    if (lock instanceof HTMLButtonElement) {
      eyeLocked(lock, row);
    }
  }

  /**
   * @param {HTMLButtonElement} button
   * @param {object} row
   */
  function eyeLocked(button, row) {
    const locked = row.locked || row.ancestorLocked;
    button.textContent = locked ? '锁' : '开';
    button.disabled = row.ancestorLocked && !row.locked;
    button.setAttribute('aria-label', row.locked ? '解锁' : '锁定');
    button.setAttribute('aria-pressed', locked ? 'true' : 'false');
  }

  /**
   * 祖先被隐藏时这一行变淡，但自己的眼睛状态不变。
   *
   * @param {object} row
   */
  function ancestorHidden(row) {
    let parent = row.parentKey;
    while (parent) {
      const node = tree.byId.get(parent);
      if (node && node.visible === false) return true;
      parent = tree.parentOf.get(parent);
    }
    return false;
  }

  function paintSelection() {
    for (const element of pool) {
      element.classList.toggle('selected', selected.has(element.dataset.id || ''));
    }
  }

  /**
   * @param {number} clientY
   * @returns {{ rowIndex: number, place: 'before' | 'after' | 'into' }}
   */
  function hitPlace(clientY) {
    const rect = viewport.getBoundingClientRect();
    const y = clientY - rect.top + viewport.scrollTop;
    if (!rows.length || y < 0) return { rowIndex: -1, place: 'before' };
    if (y >= rows.length * ROW_HEIGHT) return { rowIndex: rows.length, place: 'after' };
    const rowIndex = Math.floor(y / ROW_HEIGHT);
    const offset = y - rowIndex * ROW_HEIGHT;
    const row = rows[rowIndex];
    if (offset < 8) return { rowIndex, place: 'before' };
    if (offset > ROW_HEIGHT - 8) return { rowIndex, place: 'after' };
    if (isContainerType(row?.elementType)) return { rowIndex, place: 'into' };
    return { rowIndex, place: offset < ROW_HEIGHT / 2 ? 'before' : 'after' };
  }

  /**
   * @param {number} clientY
   */
  function showDrop(clientY) {
    if (!drag) return;
    const hit = hitPlace(clientY);
    const target = dropIndex(rows, hit.rowIndex, hit.place, drag.id, tree);
    clearDropInto();
    if (!target) {
      indicator.hidden = true;
      return;
    }
    if (hit.place === 'into' && hit.rowIndex >= 0) {
      indicator.hidden = true;
      const local = hit.rowIndex - visibleStart();
      const element = pool[local];
      element?.classList.add('drop-into');
      return;
    }
    const lineIndex = hit.rowIndex < 0 ? 0 : hit.place === 'before' ? hit.rowIndex : hit.rowIndex + 1;
    indicator.hidden = false;
    indicator.style.top = `${lineIndex * ROW_HEIGHT - viewport.scrollTop}px`;
  }

  function visibleStart() {
    return visibleRange(viewport.scrollTop, viewport.clientHeight || ROW_HEIGHT, rows.length).start;
  }

  function clearDropInto() {
    for (const element of pool) element.classList.remove('drop-into');
  }

  return panel;
}

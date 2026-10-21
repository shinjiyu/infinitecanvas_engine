/**
 * 远端光标标签 + 选区轮廓 overlay（pointer-events: none，不进场景 JSON）。
 *
 * 与 WebSocket 协同路径和 yrs awareness 共用 {@link PresenceState} 形状。
 * 不新增快捷键；权限 / 冲突 UI 不在这里。
 */

/**
 * @typedef {{ x: number, y: number }} PresenceCursor
 * @typedef {{ name: string, color: string, cursor?: PresenceCursor, selection: string[] }} PresenceState
 * @typedef {{ clientId: string, state: PresenceState }} RemotePresence
 * @typedef {{ x: number, y: number, width: number, height: number }} ScreenRect
 */

/**
 * 世界坐标 → 屏幕像素（与引擎 Viewport::world_to_screen 一致）。
 *
 * @param {{ x: number, y: number }} world
 * @param {{ centerX: number, centerY: number, zoom: number, canvasWidth: number, canvasHeight: number }} viewport
 * @returns {{ x: number, y: number }}
 */
export function worldToScreen(world, viewport) {
  const ox = (world.x - viewport.centerX) * viewport.zoom;
  const oy = (world.y - viewport.centerY) * viewport.zoom;
  return {
    x: ox + viewport.canvasWidth / 2,
    y: oy + viewport.canvasHeight / 2,
  };
}

/**
 * 世界轴对齐包围盒 → 屏幕矩形。
 *
 * @param {{ x: number, y: number, width: number, height: number }} bounds 世界包围盒（左上 + 宽高）
 * @param {{ centerX: number, centerY: number, zoom: number, canvasWidth: number, canvasHeight: number }} viewport
 * @returns {ScreenRect}
 */
export function worldBoundsToScreenRect(bounds, viewport) {
  const tl = worldToScreen({ x: bounds.x, y: bounds.y }, viewport);
  const br = worldToScreen(
    { x: bounds.x + bounds.width, y: bounds.y + bounds.height },
    viewport
  );
  return {
    x: Math.min(tl.x, br.x),
    y: Math.min(tl.y, br.y),
    width: Math.abs(br.x - tl.x),
    height: Math.abs(br.y - tl.y),
  };
}

/**
 * 渲染远端光标标签。overlay 必须 `pointer-events: none`。
 *
 * @param {HTMLElement} root
 * @param {RemotePresence[]} remotes
 * @param {{ toScreen?: (p: PresenceCursor) => { x: number, y: number } }} [opts]
 *   `toScreen`：若光标已是屏幕像素可省略；世界坐标时传入转换。
 */
export function renderRemoteCursors(root, remotes, opts = {}) {
  const keep = new Set();
  for (const remote of remotes) {
    const { clientId, state } = remote;
    if (!state.cursor) continue;
    keep.add(clientId);
    const screen = opts.toScreen
      ? opts.toScreen(state.cursor)
      : { x: state.cursor.x, y: state.cursor.y };
    let el = root.querySelector(`[data-remote-cursor="${CSS.escape(clientId)}"]`);
    if (!el) {
      el = document.createElement('div');
      el.dataset.remoteCursor = clientId;
      el.className = 'remote-cursor';
      el.style.pointerEvents = 'none';
      el.innerHTML = `
        <svg class="cursor-icon" viewBox="0 0 24 24" fill="${state.color}">
          <path d="M5.5 3.21V20.8c0 .45.54.67.85.35l4.86-4.86h8.28c.45 0 .67-.54.35-.85L5.85 2.86c-.31-.31-.85-.09-.85.35z"/>
        </svg>
        <span class="cursor-label" style="background:${state.color}">${escapeHtml(state.name)}</span>
      `;
      root.appendChild(el);
    } else {
      const label = el.querySelector('.cursor-label');
      if (label) label.textContent = state.name;
      const icon = el.querySelector('.cursor-icon');
      if (icon) icon.setAttribute('fill', state.color);
      if (label) label.style.background = state.color;
    }
    el.style.transform = `translate(${screen.x}px, ${screen.y}px)`;
  }
  for (const el of [...root.querySelectorAll('[data-remote-cursor]')]) {
    if (!keep.has(el.dataset.remoteCursor)) el.remove();
  }
}

/**
 * 渲染远端选区轮廓。不影响 hit-test（pointer-events: none）。
 *
 * @param {HTMLElement} root
 * @param {RemotePresence[]} remotes
 * @param {(elementId: string) => ScreenRect | null} resolveScreenRect
 */
export function renderRemoteSelections(root, remotes, resolveScreenRect) {
  const keep = new Set();
  for (const remote of remotes) {
    const { clientId, state } = remote;
    for (const elementId of state.selection ?? []) {
      const rect = resolveScreenRect(String(elementId));
      if (!rect) continue;
      const key = `${clientId}:${elementId}`;
      keep.add(key);
      let el = root.querySelector(`[data-remote-sel="${CSS.escape(key)}"]`);
      if (!el) {
        el = document.createElement('div');
        el.dataset.remoteSel = key;
        el.className = 'remote-selection-overlay';
        el.style.pointerEvents = 'none';
        el.style.borderColor = state.color;
        root.appendChild(el);
      } else {
        el.style.borderColor = state.color;
      }
      el.style.left = `${rect.x}px`;
      el.style.top = `${rect.y}px`;
      el.style.width = `${rect.width}px`;
      el.style.height = `${rect.height}px`;
    }
  }
  for (const el of [...root.querySelectorAll('[data-remote-sel]')]) {
    if (!keep.has(el.dataset.remoteSel)) el.remove();
  }
}

/**
 * 一次刷新光标 + 选区；离开的 peer 从 remotes 消失后 DOM 会清掉。
 *
 * @param {{ cursors: HTMLElement, selections: HTMLElement }} roots
 * @param {RemotePresence[]} remotes 已排除本地用户
 * @param {{
 *   toScreen?: (p: PresenceCursor) => { x: number, y: number },
 *   resolveScreenRect: (elementId: string) => ScreenRect | null,
 * }} opts
 */
export function renderRemotePresence(roots, remotes, opts) {
  renderRemoteCursors(roots.cursors, remotes, { toScreen: opts.toScreen });
  renderRemoteSelections(roots.selections, remotes, opts.resolveScreenRect);
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

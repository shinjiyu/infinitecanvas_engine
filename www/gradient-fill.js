/**
 * 属性面板的渐变补丁。不登记快捷键。
 *
 * 关掉填充时同时把种类收回纯色，渐变色标不再参与绘制。
 * 切到线性或径向时，没有两个色标就用当前纯色补一对。
 */

/**
 * @param {unknown} color
 * @returns {string}
 */
export function cssColor(color) {
  if (typeof color !== 'string' || !color.startsWith('#')) return '#000000';
  return color.length >= 7 ? color.slice(0, 7) : '#000000';
}

/**
 * 从当前纯色做出两端色标。接近白色时另一端用深色，否则用白色。
 *
 * @param {unknown} fill
 * @returns {{ offset: number, color: string }[]}
 */
export function defaultGradientStops(fill) {
  const start = cssColor(fill);
  const end = start.toLowerCase() === '#ffffff' ? '#1a1a1a' : '#ffffff';
  return [
    { offset: 0, color: start },
    { offset: 1, color: end },
  ];
}

/**
 * @param {unknown} stops
 * @returns {{ offset: number, color: string }[]}
 */
export function normalizeStops(stops) {
  if (!Array.isArray(stops)) return [];
  const cleaned = [];
  for (const stop of stops) {
    if (!stop || typeof stop !== 'object') continue;
    const offset = Number(/** @type {{ offset?: unknown }} */ (stop).offset);
    if (!Number.isFinite(offset)) continue;
    cleaned.push({
      offset: Math.min(1, Math.max(0, offset)),
      color: cssColor(/** @type {{ color?: unknown }} */ (stop).color),
    });
  }
  cleaned.sort((left, right) => left.offset - right.offset);
  return cleaned.slice(0, 8);
}

/**
 * 在最大的空隙里插入一个色标。已经有 8 个时原样返回。
 *
 * @param {unknown} stops
 * @returns {{ offset: number, color: string }[]}
 */
export function insertStop(stops) {
  const sorted = normalizeStops(stops);
  if (sorted.length < 2 || sorted.length >= 8) return sorted;
  let gap = -1;
  let index = 0;
  for (let i = 0; i < sorted.length - 1; i += 1) {
    const width = sorted[i + 1].offset - sorted[i].offset;
    if (width > gap) {
      gap = width;
      index = i;
    }
  }
  const left = sorted[index];
  const right = sorted[index + 1];
  const next = sorted.slice();
  next.splice(index + 1, 0, {
    offset: (left.offset + right.offset) / 2,
    color: left.color,
  });
  return next;
}

/**
 * @param {boolean} enabled
 * @param {unknown} hex
 * @returns {Record<string, unknown>}
 */
export function fillEnabledPatch(enabled, hex) {
  if (!enabled) return { fill: null, fillKind: 'solid' };
  return { fill: cssColor(hex), fillKind: 'solid' };
}

/**
 * 切换填充种类。线性/径向会带上色标，避免切过去之后没有颜色。
 *
 * @param {string} kind
 * @param {Record<string, unknown>} style
 * @returns {Record<string, unknown> | null}
 */
export function fillKindPatch(kind, style) {
  if (kind !== 'solid' && kind !== 'linear' && kind !== 'radial') return null;
  if (kind === 'solid') return { fillKind: 'solid' };
  const existing = normalizeStops(style.fillStops);
  const stops = existing.length >= 2 ? existing : defaultGradientStops(style.fill);
  /** @type {Record<string, unknown>} */
  const patch = { fillKind: kind, fillStops: stops };
  if (kind === 'linear') {
    patch.fillAngle = typeof style.fillAngle === 'number' ? style.fillAngle : 0;
  }
  if (kind === 'radial') {
    const center = style.fillCenter;
    const point = center && typeof center === 'object' ? /** @type {{ x?: unknown, y?: unknown }} */ (center) : {};
    const x = Number(point.x);
    const y = Number(point.y);
    patch.fillCenter = {
      x: Number.isFinite(x) ? x : 0,
      y: Number.isFinite(y) ? y : 0,
    };
    patch.fillRadius = typeof style.fillRadius === 'number' ? style.fillRadius : 0;
  }
  if (style.fillNone) patch.fill = stops[0].color;
  return patch;
}

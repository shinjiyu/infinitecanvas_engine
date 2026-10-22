/**
 * 用 Canvas2D / OffscreenCanvas 把一段文字光栅化成 RGBA 纹理。
 *
 * 世界尺寸 = 纹理像素 / rasterScale。边距按世界单位换算，换 LOD 时文字块大小保持稳定，
 * 只提高像素密度，这样缩放后不会发糊。
 *
 * @param {object} job
 * @param {string} job.content 文本，支持 `\n`
 * @param {number} job.fontSizePx 像素字号
 * @param {number} job.rasterScale 像素 / 世界单位
 * @param {string} job.color CSS 颜色
 * @param {string} [job.align] `left` | `center` | `right`
 * @param {string} [job.fontWeight] `regular` | `bold`（或 `400` / `700`）
 * @param {number} [job.lineHeight] 行高倍数（相对字号），缺省 1.2
 * @param {number} [job.letterSpacing] 字距（世界单位），缺省 0；乘 rasterScale 得像素
 * @param {boolean} [job.underline] 下划线，缺省 false
 * @param {boolean} [job.strikethrough] 删除线，缺省 false
 * @param {string} job.fontFamily CSS font-family，需要包含中文字体
 * @param {number} job.paddingWorld 世界单位边距
 * @returns {{ width: number, height: number, data: Uint8ClampedArray, worldWidth: number, worldHeight: number }}
 *
 * @example
 * ```js
 * const raster = rasterizeText({
 *   content: 'Hello 你好',
 *   fontSizePx: 32,
 *   rasterScale: 1,
 *   color: '#f4f7fb',
 *   align: 'left',
 *   fontWeight: 'regular',
 *   lineHeight: 1.2,
 *   letterSpacing: 0,
 *   underline: false,
 *   strikethrough: false,
 *   fontFamily: '"WenQuanYi Micro Hei", sans-serif',
 *   paddingWorld: 4,
 * });
 * ```
 */
export function rasterizeText(job) {
  const content = String(job.content ?? '');
  const fontSizePx = Math.max(1, Number(job.fontSizePx) || 16);
  const rasterScale = Math.max(0.0001, Number(job.rasterScale) || 1);
  const paddingPx = Math.max(1, Math.ceil((Number(job.paddingWorld) || 0) * rasterScale));
  const strokeWidthPx = Math.max(0, (Number(job.strokeWidthWorld) || 0) * rasterScale);
  const pad = paddingPx + Math.ceil(strokeWidthPx);
  const align = job.align === 'center' || job.align === 'right' ? job.align : 'left';
  const rawWeight = String(job.fontWeight || 'regular').toLowerCase();
  const fontWeight =
    rawWeight === 'bold' || rawWeight === '700' ? '700' : '400';
  const fontFamily = job.fontFamily || 'sans-serif';
  const color = job.color || '#f4f7fb';
  const lines = content.split('\n');
  const font = `${fontWeight} ${fontSizePx}px ${fontFamily}`;
  const rawLineHeight = Number(job.lineHeight);
  const lineHeightMult =
    Number.isFinite(rawLineHeight) && rawLineHeight > 0 ? rawLineHeight : 1.2;
  const lineHeight = fontSizePx * lineHeightMult;
  const rawSpacing = Number(job.letterSpacing);
  const letterSpacingPx =
    Number.isFinite(rawSpacing) ? rawSpacing * rasterScale : 0;
  const underline = Boolean(job.underline);
  const strikethrough = Boolean(job.strikethrough);

  const measure = createSurface(1, 1).getContext('2d');
  if (!measure) {
    throw new Error('Canvas2D is not available');
  }
  measure.font = font;
  applyLetterSpacing(measure, letterSpacingPx);

  let maxWidth = 1;
  const widths = lines.map((line) => {
    const sample = line.length > 0 ? line : ' ';
    const width = measureLineWidth(measure, sample, letterSpacingPx);
    maxWidth = Math.max(maxWidth, width);
    return width;
  });

  const width = Math.max(1, Math.ceil(maxWidth + pad * 2));
  const height = Math.max(1, Math.ceil(lineHeight * Math.max(lines.length, 1) + pad * 2));
  if (width > 8192 || height > 8192) {
    throw new Error(`Text raster ${width}x${height} exceeds the texture limit`);
  }

  const surface = createSurface(width, height);
  const ctx = surface.getContext('2d', { willReadFrequently: true });
  if (!ctx) {
    throw new Error('Canvas2D is not available');
  }
  ctx.clearRect(0, 0, width, height);
  ctx.font = font;
  applyLetterSpacing(ctx, letterSpacingPx);
  ctx.fillStyle = color;
  ctx.textBaseline = 'middle';
  ctx.textAlign = align;
  if (strokeWidthPx > 0) {
    ctx.lineWidth = strokeWidthPx;
    ctx.strokeStyle = job.strokeColor || color;
    ctx.lineJoin = 'round';
  }

  const decoWidth = Math.max(1, Math.round(fontSizePx * 0.06));
  lines.forEach((line, index) => {
    const y = pad + lineHeight * index + lineHeight / 2;
    let x = pad;
    if (align === 'center') {
      x = pad + maxWidth / 2;
    } else if (align === 'right') {
      x = pad + maxWidth;
    }
    drawLine(ctx, line, x, y, strokeWidthPx > 0, letterSpacingPx);
    if ((underline || strikethrough) && line.length > 0) {
      const lineW = widths[index] || 0;
      let left = pad;
      if (align === 'center') left = pad + (maxWidth - lineW) / 2;
      else if (align === 'right') left = pad + maxWidth - lineW;
      drawDecorations(ctx, left, y, lineW, fontSizePx, color, decoWidth, underline, strikethrough);
    }
  });

  const image = ctx.getImageData(0, 0, width, height);
  return {
    width,
    height,
    data: image.data,
    worldWidth: width / rasterScale,
    worldHeight: height / rasterScale,
    lineWidths: widths,
  };
}

/**
 * 统计 alpha 高于阈值的像素，用来确认字形真的画出来了。
 *
 * @param {Uint8ClampedArray | Uint8Array} data RGBA 缓冲
 * @param {number} [threshold]
 * @returns {number}
 */
export function countInk(data, threshold = 16) {
  let count = 0;
  for (let i = 3; i < data.length; i += 4) {
    if (data[i] > threshold) count += 1;
  }
  return count;
}

/**
 * @param {CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D} ctx
 * @param {number} spacingPx
 */
function applyLetterSpacing(ctx, spacingPx) {
  if (typeof ctx.letterSpacing === 'string' || 'letterSpacing' in ctx) {
    try {
      ctx.letterSpacing = `${spacingPx}px`;
    } catch {
      // ignore unsupported assignment
    }
  }
}

/**
 * @param {CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D} ctx
 * @param {string} text
 * @param {number} spacingPx
 * @returns {number}
 */
function measureLineWidth(ctx, text, spacingPx) {
  if (supportsLetterSpacing(ctx) || Math.abs(spacingPx) < 1e-6) {
    return ctx.measureText(text).width;
  }
  let width = 0;
  const chars = Array.from(text);
  chars.forEach((ch, index) => {
    width += ctx.measureText(ch).width;
    if (index < chars.length - 1) width += spacingPx;
  });
  return width;
}

/**
 * @param {CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D} ctx
 * @param {string} line
 * @param {number} x
 * @param {number} y
 * @param {boolean} stroke
 * @param {number} spacingPx
 */
function drawLine(ctx, line, x, y, stroke, spacingPx) {
  if (supportsLetterSpacing(ctx) || Math.abs(spacingPx) < 1e-6 || line.length === 0) {
    if (stroke) ctx.strokeText(line, x, y);
    ctx.fillText(line, x, y);
    return;
  }
  const chars = Array.from(line);
  const widths = chars.map((ch) => ctx.measureText(ch).width);
  const total =
    widths.reduce((sum, w) => sum + w, 0) + spacingPx * Math.max(chars.length - 1, 0);
  let cursor = x;
  if (ctx.textAlign === 'center') cursor = x - total / 2;
  else if (ctx.textAlign === 'right') cursor = x - total;
  const prevAlign = ctx.textAlign;
  ctx.textAlign = 'left';
  chars.forEach((ch, index) => {
    if (stroke) ctx.strokeText(ch, cursor, y);
    ctx.fillText(ch, cursor, y);
    cursor += widths[index] + spacingPx;
  });
  ctx.textAlign = prevAlign;
}

/**
 * @param {CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D} ctx
 * @returns {boolean}
 */
function supportsLetterSpacing(ctx) {
  return typeof ctx.letterSpacing === 'string' || 'letterSpacing' in ctx;
}

/**
 * 在字形中线附近画下划线 / 删除线。颜色跟文字填充，线宽约 6% 字号。
 * textBaseline 为 middle 时：删除线过中线，下划线略靠下。
 *
 * @param {CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D} ctx
 * @param {number} left
 * @param {number} midY
 * @param {number} width
 * @param {number} fontSizePx
 * @param {string} color
 * @param {number} lineWidth
 * @param {boolean} underline
 * @param {boolean} strikethrough
 */
function drawDecorations(ctx, left, midY, width, fontSizePx, color, lineWidth, underline, strikethrough) {
  if (width <= 0) return;
  const prevStroke = ctx.strokeStyle;
  const prevWidth = ctx.lineWidth;
  const prevCap = ctx.lineCap;
  ctx.strokeStyle = color;
  ctx.lineWidth = lineWidth;
  ctx.lineCap = 'butt';
  if (strikethrough) {
    const y = midY;
    ctx.beginPath();
    ctx.moveTo(left, y);
    ctx.lineTo(left + width, y);
    ctx.stroke();
  }
  if (underline) {
    const y = midY + fontSizePx * 0.4;
    ctx.beginPath();
    ctx.moveTo(left, y);
    ctx.lineTo(left + width, y);
    ctx.stroke();
  }
  ctx.strokeStyle = prevStroke;
  ctx.lineWidth = prevWidth;
  ctx.lineCap = prevCap;
}

/**
 * @param {number} width
 * @param {number} height
 * @returns {OffscreenCanvas | HTMLCanvasElement}
 */
function createSurface(width, height) {
  if (typeof OffscreenCanvas !== 'undefined') {
    return new OffscreenCanvas(width, height);
  }
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

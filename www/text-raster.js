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
  const fontFamily = job.fontFamily || 'sans-serif';
  const color = job.color || '#f4f7fb';
  const lines = content.split('\n');
  const font = `${fontSizePx}px ${fontFamily}`;
  const lineHeight = fontSizePx * 1.4;

  const measure = createSurface(1, 1).getContext('2d');
  if (!measure) {
    throw new Error('Canvas2D is not available');
  }
  measure.font = font;

  let maxWidth = 1;
  const widths = lines.map((line) => {
    const sample = line.length > 0 ? line : ' ';
    const width = measure.measureText(sample).width;
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
  ctx.fillStyle = color;
  ctx.textBaseline = 'middle';
  ctx.textAlign = align;
  if (strokeWidthPx > 0) {
    ctx.lineWidth = strokeWidthPx;
    ctx.strokeStyle = job.strokeColor || color;
    ctx.lineJoin = 'round';
  }

  lines.forEach((line, index) => {
    const y = pad + lineHeight * index + lineHeight / 2;
    let x = pad;
    if (align === 'center') {
      x = pad + maxWidth / 2;
    } else if (align === 'right') {
      x = pad + maxWidth;
    }
    if (strokeWidthPx > 0) ctx.strokeText(line, x, y);
    ctx.fillText(line, x, y);
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

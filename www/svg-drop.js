/**
 * 把 SVG 文件和 PNG/JPEG 分开，避免导入 SVG 时走图片解码。
 *
 * 没有 MIME 的文件仍当作光栅图（和原来的拖放一致），除非扩展名是 `.svg`。
 */

/** 工具栏导入时，指针还没进过画布就用这个世界坐标当 viewBox 原点。 */
export const SVG_IMPORT_DEFAULT_ANCHOR = { x: 24, y: 24 };

/**
 * @param {{ name?: string, type?: string }} file
 * @returns {boolean}
 */
export function isSvgFile(file) {
  const type = (file.type || '').toLowerCase();
  const name = (file.name || '').toLowerCase();
  return type === 'image/svg+xml' || name.endsWith('.svg');
}

/**
 * PNG/JPEG，或没有类型、也不是 SVG 的文件。
 *
 * @param {{ name?: string, type?: string }} file
 * @returns {boolean}
 */
export function isRasterImageFile(file) {
  if (isSvgFile(file)) return false;
  const type = (file.type || '').toLowerCase();
  if (!type) return true;
  return type === 'image/png' || type === 'image/jpeg' || type === 'image/jpg';
}

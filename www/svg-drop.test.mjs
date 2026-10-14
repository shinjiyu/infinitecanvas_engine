import assert from 'node:assert/strict';
import test from 'node:test';
import { SVG_IMPORT_DEFAULT_ANCHOR, isRasterImageFile, isSvgFile } from './svg-drop.js';

test('svg files stay out of the raster image path', () => {
  assert.equal(isSvgFile({ name: 'icon.svg', type: '' }), true);
  assert.equal(isSvgFile({ name: 'icon.SVG', type: 'image/svg+xml' }), true);
  assert.equal(isRasterImageFile({ name: 'icon.svg', type: '' }), false);
  assert.equal(isRasterImageFile({ name: 'icon.svg', type: 'image/svg+xml' }), false);
  assert.equal(isSvgFile({ name: 'photo.png', type: 'image/png' }), false);
  assert.equal(isRasterImageFile({ name: 'photo.png', type: 'image/png' }), true);
  assert.equal(isRasterImageFile({ name: 'photo.jpg', type: 'image/jpeg' }), true);
  assert.equal(isRasterImageFile({ name: 'photo.jpeg', type: 'image/jpg' }), true);
  assert.equal(isRasterImageFile({ name: 'clipboard', type: '' }), true);
  assert.deepEqual(SVG_IMPORT_DEFAULT_ANCHOR, { x: 24, y: 24 });
});

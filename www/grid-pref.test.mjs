import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

import { GRID_VISIBLE_KEY, readGridVisible, writeGridVisible } from './grid-pref.js';

/**
 * @param {Record<string, string>} [initial]
 * @returns {{ getItem: (key: string) => string | null, setItem: (key: string, value: string) => void }}
 */
function memoryStorage(initial = {}) {
  /** @type {Record<string, string>} */
  const data = { ...initial };
  return {
    getItem(key) {
      return Object.prototype.hasOwnProperty.call(data, key) ? data[key] : null;
    },
    setItem(key, value) {
      data[key] = String(value);
    },
  };
}

test('a missing preference stays visible', () => {
  assert.equal(GRID_VISIBLE_KEY, 'infinitecanvas.gridVisible');
  assert.equal(readGridVisible(memoryStorage()), true);
  assert.equal(readGridVisible(null), true);
  assert.equal(readGridVisible(undefined), true);
});

test('only the string false hides the grid', () => {
  const storage = memoryStorage();
  writeGridVisible(storage, false);
  assert.equal(storage.getItem(GRID_VISIBLE_KEY), 'false');
  assert.equal(readGridVisible(storage), false);

  writeGridVisible(storage, true);
  assert.equal(storage.getItem(GRID_VISIBLE_KEY), 'true');
  assert.equal(readGridVisible(storage), true);

  storage.setItem(GRID_VISIBLE_KEY, '0');
  assert.equal(readGridVisible(storage), true);
  storage.setItem(GRID_VISIBLE_KEY, '');
  assert.equal(readGridVisible(storage), true);
  storage.setItem(GRID_VISIBLE_KEY, 'FALSE');
  assert.equal(readGridVisible(storage), true);
});

test('a storage failure does not throw and stays visible', () => {
  assert.equal(readGridVisible({
    getItem() {
      throw new Error('denied');
    },
  }), true);
  writeGridVisible({
    setItem() {
      throw new Error('denied');
    },
  }, false);
  writeGridVisible(null, false);
});

test('the demo reads and writes the grid preference', () => {
  const main = fs.readFileSync(new URL('./main.js', import.meta.url), 'utf8');
  const html = fs.readFileSync(new URL('./index.html', import.meta.url), 'utf8');
  assert.match(main, /readGridVisible/);
  assert.match(main, /writeGridVisible/);
  assert.match(main, /set_grid_visible/);
  assert.match(main, /grid_visible/);
  assert.match(main, /toggleGrid/);
  assert.match(html, /id="grid-btn"/);
});

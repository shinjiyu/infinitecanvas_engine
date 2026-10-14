import assert from 'node:assert/strict';
import test from 'node:test';
import {
  canvasPointerReachesEngine,
  clampMenuPosition,
  contextMenuItems,
  resolveContextTarget,
  shortcutHint,
} from './context-menu.js';

function ids(items) {
  return items.filter((item) => item.type !== 'separator').map((item) => item.id);
}

function hintOf(items, id) {
  return items.find((item) => item.id === id)?.hint;
}

test('empty canvas lists paste, select all, and zoom to fit', () => {
  const items = contextMenuItems({
    kind: 'canvas',
    clipboardHasContent: false,
    canGroup: false,
    canUngroup: false,
  }, false);
  assert.deepEqual(ids(items), ['paste', 'select-all', 'zoom-fit']);
});

test('selection menu hides paste until the clipboard has text', () => {
  const bare = contextMenuItems({
    kind: 'selection',
    clipboardHasContent: false,
    canGroup: false,
    canUngroup: false,
  }, false);
  assert.deepEqual(ids(bare), [
    'copy',
    'duplicate',
    'delete',
    'front',
    'back',
    'forward',
    'backward',
  ]);
  const withPaste = contextMenuItems({
    kind: 'selection',
    clipboardHasContent: true,
    canGroup: false,
    canUngroup: false,
  }, false);
  assert.equal(ids(withPaste)[1], 'paste');
});

test('group and ungroup appear only when that action applies', () => {
  const grouped = contextMenuItems({
    kind: 'selection',
    clipboardHasContent: false,
    canGroup: true,
    canUngroup: false,
  }, false);
  assert.ok(ids(grouped).includes('group'));
  assert.ok(!ids(grouped).includes('ungroup'));

  const singleGroup = contextMenuItems({
    kind: 'selection',
    clipboardHasContent: false,
    canGroup: false,
    canUngroup: true,
  }, false);
  assert.ok(ids(singleGroup).includes('ungroup'));
  assert.ok(!ids(singleGroup).includes('group'));

  const neither = contextMenuItems({
    kind: 'selection',
    clipboardHasContent: false,
    canGroup: false,
    canUngroup: false,
  }, false);
  assert.ok(!ids(neither).includes('group'));
  assert.ok(!ids(neither).includes('ungroup'));
});

test('menu hints come from the SHORTCUTS table', () => {
  assert.equal(shortcutHint('copy', false), 'Ctrl+C');
  assert.equal(shortcutHint('zoom-fit', false), 'Shift+1');
  assert.equal(shortcutHint('delete', false), 'Delete / Backspace');
  assert.equal(shortcutHint('group', false), 'Ctrl+G');
  assert.equal(shortcutHint('ungroup', false), 'Ctrl+Shift+G');
  assert.equal(shortcutHint('front', false), 'Ctrl+]');
  assert.equal(shortcutHint('forward', false), ']');
  assert.equal(shortcutHint('select-all', false), 'Ctrl+A');
  assert.equal(shortcutHint('not-a-command', false), undefined);

  const canvas = contextMenuItems({ kind: 'canvas' }, false);
  assert.equal(hintOf(canvas, 'paste'), 'Ctrl+V');
  assert.equal(hintOf(canvas, 'zoom-fit'), 'Shift+1');

  const selection = contextMenuItems({
    kind: 'selection',
    canGroup: true,
    canUngroup: false,
  }, false);
  assert.equal(hintOf(selection, 'duplicate'), 'Ctrl+D');
  assert.equal(hintOf(selection, 'group'), 'Ctrl+G');
  assert.equal(hintOf(selection, 'back'), 'Ctrl+[');
});

test('a hit on the selection or its ancestor keeps the selection', () => {
  const parentOf = (id) => (id === 'child' ? 'group' : null);
  assert.deepEqual(resolveContextTarget(null, ['1'], parentOf), {
    kind: 'canvas',
    selectHit: false,
  });
  assert.deepEqual(resolveContextTarget('1', ['1', '2'], parentOf), {
    kind: 'selection',
    selectHit: false,
  });
  assert.deepEqual(resolveContextTarget('child', ['group'], parentOf), {
    kind: 'selection',
    selectHit: false,
  });
  assert.deepEqual(resolveContextTarget('9', ['1'], parentOf), {
    kind: 'selection',
    selectHit: true,
  });
});

test('an open menu blocks the pointer that would pan or marquee', () => {
  assert.equal(canvasPointerReachesEngine(true), false);
  assert.equal(canvasPointerReachesEngine(false), true);
});

test('the menu stays inside the viewport', () => {
  assert.deepEqual(clampMenuPosition(900, 700, 180, 220, 800, 600), { x: 612, y: 372 });
  assert.deepEqual(clampMenuPosition(4, 2, 180, 40, 800, 600), { x: 8, y: 8 });
});

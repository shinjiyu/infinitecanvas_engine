/**
 * 快捷键表的单元测试。
 *
 * 运行：`node --test shortcuts.test.js`（在 `www/` 目录）。
 */
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';

import {
  SHORTCUTS,
  escapeAction,
  formatChord,
  formatZoomPercent,
  helpEntries,
  isCanvasStageFocus,
  isEditableTarget,
  matchShortcut,
  prefersMacModifier,
  shouldApplyShortcut,
} from './shortcuts.js';

/**
 * @param {Partial<{ key: string, code: string, ctrlKey: boolean, metaKey: boolean, shiftKey: boolean, altKey: boolean, repeat: boolean }>} partial
 */
function keyEvent(partial) {
  return {
    key: '',
    code: '',
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    repeat: false,
    ...partial,
  };
}

/**
 * @param {string} handler
 * @param {Parameters<typeof keyEvent>[0]} partial
 * @param {'down' | 'up'} [phase]
 */
function assertHandler(handler, partial, phase = 'down') {
  const hit = matchShortcut(SHORTCUTS, keyEvent(partial), phase);
  assert.ok(hit, `expected ${handler}`);
  assert.equal(hit.handler, handler);
  return hit;
}

test('shortcut ids are unique and every row has an action, tool, and handler', () => {
  const ids = new Set();
  for (const shortcut of SHORTCUTS) {
    assert.equal(ids.has(shortcut.id), false, shortcut.id);
    ids.add(shortcut.id);
    assert.ok(shortcut.action);
    assert.ok(shortcut.tool);
    assert.ok(shortcut.handler);
    assert.ok(shortcut.label);
  }
});

test('tool keys select the tools that exist', () => {
  assert.equal(assertHandler('tool', { key: 'v', code: 'KeyV' }).tool, 'select');
  assert.equal(assertHandler('tool', { key: 'r', code: 'KeyR' }).tool, 'rectangle');
  assert.equal(assertHandler('tool', { key: 'o', code: 'KeyO' }).tool, 'circle');
  assert.equal(assertHandler('tool', { key: 'l', code: 'KeyL' }).tool, 'line');
  assert.equal(assertHandler('tool', { key: 'p', code: 'KeyP' }).tool, 'pen');
  assert.equal(assertHandler('tool', { key: 'h', code: 'KeyH' }).tool, 'pan');
  assert.equal(assertHandler('tool', { key: 't', code: 'KeyT' }).tool, 'text');
  assert.equal(assertHandler('tool', { key: 'i', code: 'KeyI' }).tool, 'eyedropper');
  assert.equal(assertHandler('toggleMinimap', { key: 'm', code: 'KeyM' }).id, 'minimap');
  assert.equal(assertHandler('tool', { key: 'V' }).tool, 'select');
  assert.equal(assertHandler('tool', { key: 'l', shiftKey: true }).tool, 'line');
});

test('eyedropper is plain I and leaves the other tool keys alone', () => {
  assert.equal(assertHandler('tool', { key: 'I' }).tool, 'eyedropper');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'i', shiftKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'i', ctrlKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'i', metaKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'i', altKey: true }), 'down'), null);
  assert.equal(assertHandler('tool', { key: 'v' }).tool, 'select');
  assert.equal(assertHandler('tool', { key: 'r' }).tool, 'rectangle');
  assert.equal(assertHandler('tool', { key: 'h' }).tool, 'pan');
  const row = helpEntries(SHORTCUTS, false).find((entry) => entry.id === 'tool-eyedropper');
  assert.ok(row);
  assert.equal(row.toolLabel, '吸管');
  assert.equal(row.action, '吸管：单击吸取填充和描边');
  assert.deepEqual(row.chords, ['I']);
});

test('modifier letters do not switch tools', () => {
  const ctrlV = matchShortcut(SHORTCUTS, keyEvent({ key: 'v', ctrlKey: true }), 'down');
  assert.equal(ctrlV?.handler, 'paste');
  assert.notEqual(ctrlV?.handler, 'tool');
  const metaV = matchShortcut(SHORTCUTS, keyEvent({ key: 'v', metaKey: true }), 'down');
  assert.equal(metaV?.handler, 'paste');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', altKey: true }), 'down'), null);
  const lock = matchShortcut(SHORTCUTS, keyEvent({ key: 'l', ctrlKey: true }), 'down');
  assert.equal(lock?.handler, 'toggleLock');
  assert.notEqual(lock?.handler, 'tool');
});

test('undo, redo, save, and open keep their previous chords', () => {
  assert.equal(assertHandler('undo', { key: 'z', ctrlKey: true }).id, 'undo');
  assert.equal(assertHandler('undo', { key: 'z', metaKey: true }).id, 'undo');
  assert.equal(assertHandler('redo', { key: 'y', ctrlKey: true }).id, 'redo-y');
  assert.equal(assertHandler('redo', { key: 'z', ctrlKey: true, shiftKey: true }).id, 'redo-shift-z');
  assert.equal(assertHandler('redo', { key: 'Z', metaKey: true, shiftKey: true }).id, 'redo-shift-z');
  assert.equal(assertHandler('save', { key: 's', metaKey: true }).id, 'save');
  assert.equal(assertHandler('open', { key: 'o', ctrlKey: true }).id, 'open');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'z', ctrlKey: true, shiftKey: true }), 'down')?.handler, 'redo');
  assert.notEqual(
    matchShortcut(SHORTCUTS, keyEvent({ key: 'o', ctrlKey: true }), 'down')?.tool,
    'circle',
  );
});

test('delete, select all, escape, zoom, and help', () => {
  assert.equal(assertHandler('delete', { key: 'Delete' }).id, 'delete');
  assert.equal(assertHandler('delete', { key: 'Backspace' }).id, 'backspace');
  assert.equal(assertHandler('selectAll', { key: 'a', ctrlKey: true }).id, 'select-all');
  assert.equal(assertHandler('selectAll', { key: 'a', metaKey: true }).id, 'select-all');
  assert.equal(assertHandler('group', { key: 'g', ctrlKey: true }).id, 'group');
  assert.equal(assertHandler('group', { key: 'G', metaKey: true }).id, 'group');
  assert.equal(assertHandler('ungroup', { key: 'g', ctrlKey: true, shiftKey: true }).id, 'ungroup');
  assert.equal(assertHandler('ungroup', { key: 'G', metaKey: true, shiftKey: true }).id, 'ungroup');
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: 'g', ctrlKey: true, shiftKey: true }), 'down')?.handler,
    'ungroup',
  );
  const rulers = assertHandler('toggleRulers', { key: 'g' });
  assert.equal(rulers.id, 'rulers');
  assert.equal(rulers.allowRepeat, false);
  assert.equal(assertHandler('toggleRulers', { key: 'G' }).id, 'rulers');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'g', ctrlKey: true, altKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'g', shiftKey: true }), 'down'), null);
  const grid = assertHandler('toggleGrid', { key: "'", code: 'Quote', ctrlKey: true });
  assert.equal(grid.id, 'toggle-grid');
  assert.equal(grid.allowRepeat, false);
  assert.equal(assertHandler('toggleGrid', { key: "'", code: 'Quote', metaKey: true }).id, 'toggle-grid');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: "'", code: 'Quote' }), 'down'), null);
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: '"', code: 'Quote', ctrlKey: true, shiftKey: true }), 'down'),
    null,
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: "'", code: 'Quote', ctrlKey: true, altKey: true }), 'down'),
    null,
  );
  assert.notEqual(matchShortcut(SHORTCUTS, keyEvent({ key: 'g' }), 'down')?.handler, 'toggleGrid');
  assert.notEqual(
    matchShortcut(SHORTCUTS, keyEvent({ key: "'", code: 'Quote', ctrlKey: true }), 'down')?.handler,
    'toggleRulers',
  );
  assert.equal(assertHandler('escape', { key: 'Escape' }).id, 'escape');
  assert.equal(assertHandler('zoomFit', { key: '!', code: 'Digit1', shiftKey: true }).id, 'zoom-fit');
  assert.equal(assertHandler('zoomFit', { key: '1', code: 'Digit1', shiftKey: true }).id, 'zoom-fit');
  assert.equal(assertHandler('zoomSelection', { key: '@', code: 'Digit2', shiftKey: true }).id, 'zoom-selection');
  assert.equal(assertHandler('zoomSelection', { key: '2', code: 'Digit2', shiftKey: true }).id, 'zoom-selection');
  assert.equal(assertHandler('zoom100', { key: ')', code: 'Digit0', shiftKey: true }).id, 'zoom-100');
  assert.equal(assertHandler('zoom100', { code: 'Digit0', shiftKey: true }).id, 'zoom-100');
  assert.equal(assertHandler('zoomIn', { key: '=', code: 'Equal', ctrlKey: true }).handler, 'zoomIn');
  assert.equal(assertHandler('zoomIn', { key: '+', code: 'Equal', ctrlKey: true, shiftKey: true }).id, 'zoom-in');
  assert.equal(assertHandler('zoomIn', { key: '+', code: 'NumpadAdd', metaKey: true }).id, 'zoom-in');
  assert.equal(assertHandler('zoomOut', { key: '-', code: 'Minus', ctrlKey: true }).id, 'zoom-out');
  assert.equal(assertHandler('zoomOut', { key: '-', code: 'NumpadSubtract', metaKey: true }).id, 'zoom-out');
  assert.equal(assertHandler('zoom100', { key: '0', code: 'Digit0', ctrlKey: true }).id, 'zoom-reset');
  assert.equal(assertHandler('zoom100', { key: '0', code: 'Numpad0', metaKey: true }).id, 'zoom-reset');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '=', code: 'Equal' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '-', code: 'Minus' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '0', code: 'Digit0' }), 'down'), null);
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: '_', code: 'Minus', ctrlKey: true, shiftKey: true }), 'down'),
    null,
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: '=', code: 'Equal', ctrlKey: true, altKey: true }), 'down'),
    null,
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: ')', code: 'Digit0', ctrlKey: true, shiftKey: true }), 'down'),
    null,
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: '@', code: 'Digit2', shiftKey: true }), 'down')?.handler,
    'zoomSelection',
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: '!', code: 'Digit1', shiftKey: true }), 'down')?.handler,
    'zoomFit',
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: ')', code: 'Digit0', shiftKey: true }), 'down')?.handler,
    'zoom100',
  );
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '2', code: 'Digit2' }), 'down'), null);
  assert.equal(assertHandler('toggleHelp', { key: '?', code: 'Slash', shiftKey: true }).id, 'help');
  assert.equal(assertHandler('toggleHelp', { key: '/', code: 'Slash', shiftKey: true }).id, 'help');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '1', code: 'Digit1' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '/', code: 'Slash' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '?', code: 'Slash' }), 'down'), null);
});

test('copy, paste, and duplicate use Ctrl or Cmd without Shift', () => {
  assert.equal(assertHandler('copy', { key: 'c', ctrlKey: true }).id, 'copy');
  assert.equal(assertHandler('copy', { key: 'C', metaKey: true }).id, 'copy');
  assert.equal(assertHandler('paste', { key: 'v', ctrlKey: true }).id, 'paste');
  assert.equal(assertHandler('duplicate', { key: 'd', metaKey: true }).id, 'duplicate');
  assert.equal(assertHandler('duplicate', { key: 'D', ctrlKey: true }).id, 'duplicate');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'c' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'd' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'c', ctrlKey: true, shiftKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', ctrlKey: true, shiftKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'd', metaKey: true, altKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', ctrlKey: true }), 'down')?.tool, 'canvas');
});

test('arrow keys nudge, and Shift is the 10x step', () => {
  assert.equal(assertHandler('nudge', { key: 'ArrowLeft' }).direction, 'left');
  assert.equal(assertHandler('nudge', { key: 'ArrowRight' }).direction, 'right');
  assert.equal(assertHandler('nudge', { key: 'ArrowUp' }).direction, 'up');
  assert.equal(assertHandler('nudge', { key: 'ArrowDown' }).direction, 'down');
  assert.equal(assertHandler('nudge', { key: 'ArrowLeft' }).shift, false);
  const fast = assertHandler('nudge', { key: 'ArrowUp', shiftKey: true });
  assert.equal(fast.direction, 'up');
  assert.equal(fast.shift, true);
  assert.equal(fast.id, 'nudge-up-fast');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'ArrowRight', ctrlKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'ArrowLeft', metaKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'ArrowDown', altKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'ArrowRight', ctrlKey: true, shiftKey: true }), 'down'), null);
});

test('shift H, V, and R flip or rotate; plain H, V, and R stay tools', () => {
  const horizontal = assertHandler('flip', { key: 'h', shiftKey: true });
  assert.equal(horizontal.axis, 'horizontal');
  assert.equal(horizontal.id, 'flip-horizontal');
  assert.equal(horizontal.allowRepeat, false);
  const vertical = assertHandler('flip', { key: 'V', shiftKey: true });
  assert.equal(vertical.axis, 'vertical');
  assert.equal(vertical.id, 'flip-vertical');
  const cw = assertHandler('rotate', { key: 'r', shiftKey: true });
  assert.equal(cw.dir, 'cw');
  assert.equal(cw.id, 'rotate-cw');
  assert.equal(cw.allowRepeat, false);
  const ccw = assertHandler('rotate', { key: 'R', shiftKey: true, altKey: true });
  assert.equal(ccw.dir, 'ccw');
  assert.equal(ccw.id, 'rotate-ccw');
  assert.equal(ccw.allowRepeat, false);
  assert.equal(assertHandler('tool', { key: 'h' }).tool, 'pan');
  assert.equal(assertHandler('tool', { key: 'v' }).tool, 'select');
  assert.equal(assertHandler('tool', { key: 'r' }).tool, 'rectangle');
  const hide = matchShortcut(SHORTCUTS, keyEvent({ key: 'h', shiftKey: true, ctrlKey: true }), 'down');
  assert.equal(hide?.handler, 'toggleHide');
  assert.equal(hide?.id, 'toggle-hide');
  assert.notEqual(hide?.handler, 'flip');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', shiftKey: true, metaKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'r', shiftKey: true, ctrlKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'r', shiftKey: true, metaKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'r', altKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'h', shiftKey: true, altKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', ctrlKey: true }), 'down')?.handler, 'paste');
});

test('ctrl or cmd L locks, and ctrl or cmd shift H hides', () => {
  const lock = assertHandler('toggleLock', { key: 'l', ctrlKey: true });
  assert.equal(lock.id, 'toggle-lock');
  assert.equal(lock.allowRepeat, false);
  assert.equal(assertHandler('toggleLock', { key: 'L', metaKey: true }).id, 'toggle-lock');
  assert.equal(assertHandler('tool', { key: 'l' }).tool, 'line');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'l', ctrlKey: true, shiftKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'l', altKey: true }), 'down'), null);
  const hide = assertHandler('toggleHide', { key: 'H', metaKey: true, shiftKey: true });
  assert.equal(hide.id, 'toggle-hide');
  assert.equal(hide.allowRepeat, false);
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: 'h', ctrlKey: true, shiftKey: true, altKey: true }), 'down'),
    null,
  );
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'h', ctrlKey: true }), 'down'), null);
});

test('bracket keys reorder the selection, and Shift or Alt do not', () => {
  assert.equal(assertHandler('reorder', { key: ']', code: 'BracketRight' }).order, 'forward');
  assert.equal(assertHandler('reorder', { key: '[', code: 'BracketLeft' }).order, 'backward');
  assert.equal(assertHandler('reorder', { key: 'ü', code: 'BracketLeft' }).order, 'backward');
  const front = assertHandler('reorder', { key: ']', code: 'BracketRight', ctrlKey: true });
  assert.equal(front.order, 'front');
  assert.equal(front.id, 'stack-front');
  const back = assertHandler('reorder', { key: '[', code: 'BracketLeft', metaKey: true });
  assert.equal(back.order, 'back');
  assert.equal(back.id, 'stack-back');
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: '}', code: 'BracketRight', shiftKey: true }), 'down'),
    null,
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: ']', code: 'BracketRight', altKey: true }), 'down'),
    null,
  );
  assert.equal(
    matchShortcut(
      SHORTCUTS,
      keyEvent({ key: ']', code: 'BracketRight', ctrlKey: true, shiftKey: true }),
      'down',
    ),
    null,
  );
});

test('space pushes the hand tool on keydown and pops it on keyup', () => {
  assert.equal(assertHandler('pushHand', { key: ' ', code: 'Space' }, 'down').id, 'temp-pan');
  assert.equal(assertHandler('popHand', { key: ' ', code: 'Space' }, 'up').id, 'temp-pan-up');
  assert.equal(assertHandler('pushHand', { key: ' ', code: 'Space', ctrlKey: true }).id, 'temp-pan');
  assert.equal(assertHandler('pushHand', { key: ' ', code: 'Space', altKey: true }).id, 'temp-pan');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: ' ', code: 'Space' }), 'up')?.handler, 'popHand');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: ' ', code: 'Space' }), 'down')?.handler, 'pushHand');
});

test('help rows are generated from the table and show both chords', () => {
  const mac = helpEntries(SHORTCUTS, true);
  const other = helpEntries(SHORTCUTS, false);
  assert.equal(mac.length, other.length);
  assert.deepEqual(mac.map((row) => row.id), other.map((row) => row.id));

  const redo = mac.find((row) => row.id === 'redo');
  assert.deepEqual(redo?.chords, ['⌘Y', '⌘⇧Z']);
  const redoOther = other.find((row) => row.id === 'redo');
  assert.deepEqual(redoOther?.chords, ['Ctrl+Y', 'Ctrl+Shift+Z']);

  const deleted = other.find((row) => row.id === 'delete');
  assert.deepEqual(deleted?.chords, ['Delete', 'Backspace']);

  assert.equal(other.find((row) => row.id === 'zoom-fit')?.chords[0], 'Shift+1');
  assert.equal(other.find((row) => row.id === 'zoom-selection')?.chords[0], 'Shift+2');
  assert.equal(mac.find((row) => row.id === 'zoom-selection')?.chords[0], '⇧2');
  assert.equal(other.find((row) => row.id === 'zoom-selection')?.action, '缩放到选中');
  assert.equal(mac.find((row) => row.id === 'zoom-100')?.chords[0], '⇧0');
  assert.deepEqual(other.find((row) => row.id === 'zoom-in')?.chords, ['Ctrl++', 'Ctrl+=']);
  assert.deepEqual(mac.find((row) => row.id === 'zoom-in')?.chords, ['⌘+', '⌘=']);
  assert.equal(other.find((row) => row.id === 'zoom-out')?.chords[0], 'Ctrl+-');
  assert.equal(mac.find((row) => row.id === 'zoom-out')?.chords[0], '⌘-');
  assert.deepEqual(other.find((row) => row.id === 'zoom-100')?.chords, ['Shift+0', 'Ctrl+0']);
  assert.deepEqual(mac.find((row) => row.id === 'zoom-100')?.chords, ['⇧0', '⌘0']);
  assert.equal(other.find((row) => row.id === 'undo')?.chords[0], 'Ctrl+Z');
  assert.equal(mac.find((row) => row.id === 'select-all')?.chords[0], '⌘A');
  assert.equal(other.find((row) => row.id === 'rulers')?.chords[0], 'G');
  assert.equal(other.find((row) => row.id === 'rulers')?.action, '显示或隐藏标尺和参考线');
  assert.equal(other.find((row) => row.id === 'rulers')?.toolLabel, '画布');
  assert.equal(other.find((row) => row.id === 'toggle-grid')?.chords[0], "Ctrl+'");
  assert.equal(mac.find((row) => row.id === 'toggle-grid')?.chords[0], "⌘'");
  assert.equal(other.find((row) => row.id === 'toggle-grid')?.action, '显示或隐藏背景网格');
  assert.equal(mac.find((row) => row.id === 'toggle-grid')?.toolLabel, '画布');
  assert.equal(other.find((row) => row.id === 'group')?.chords[0], 'Ctrl+G');
  assert.equal(mac.find((row) => row.id === 'group')?.chords[0], '⌘G');
  assert.equal(other.find((row) => row.id === 'ungroup')?.chords[0], 'Ctrl+Shift+G');
  assert.equal(mac.find((row) => row.id === 'ungroup')?.chords[0], '⌘⇧G');
  assert.equal(other.find((row) => row.id === 'copy')?.chords[0], 'Ctrl+C');
  assert.equal(mac.find((row) => row.id === 'paste')?.chords[0], '⌘V');
  assert.equal(other.find((row) => row.id === 'duplicate')?.chords[0], 'Ctrl+D');
  assert.deepEqual(other.find((row) => row.id === 'nudge')?.chords, ['←', '→', '↑', '↓']);
  assert.deepEqual(mac.find((row) => row.id === 'nudge-fast')?.chords, ['⇧←', '⇧→', '⇧↑', '⇧↓']);
  assert.deepEqual(other.find((row) => row.id === 'nudge-fast')?.chords, [
    'Shift+←',
    'Shift+→',
    'Shift+↑',
    'Shift+↓',
  ]);
  assert.equal(other.find((row) => row.id === 'stack-forward')?.chords[0], ']');
  assert.equal(other.find((row) => row.id === 'stack-backward')?.chords[0], '[');
  assert.equal(other.find((row) => row.id === 'stack-front')?.chords[0], 'Ctrl+]');
  assert.equal(mac.find((row) => row.id === 'stack-front')?.chords[0], '⌘]');
  assert.equal(other.find((row) => row.id === 'stack-back')?.chords[0], 'Ctrl+[');
  assert.equal(mac.find((row) => row.id === 'stack-back')?.chords[0], '⌘[');
  assert.equal(other.find((row) => row.id === 'flip-horizontal')?.chords[0], 'Shift+H');
  assert.equal(mac.find((row) => row.id === 'flip-vertical')?.chords[0], '⇧V');
  assert.equal(other.find((row) => row.id === 'rotate-cw')?.chords[0], 'Shift+R');
  assert.equal(mac.find((row) => row.id === 'rotate-cw')?.chords[0], '⇧R');
  assert.equal(other.find((row) => row.id === 'rotate-ccw')?.chords[0], 'Shift+Alt+R');
  assert.equal(mac.find((row) => row.id === 'rotate-ccw')?.chords[0], '⇧⌥R');
  assert.equal(other.find((row) => row.id === 'toggle-lock')?.chords[0], 'Ctrl+L');
  assert.equal(mac.find((row) => row.id === 'toggle-lock')?.chords[0], '⌘L');
  assert.equal(other.find((row) => row.id === 'toggle-hide')?.chords[0], 'Ctrl+Shift+H');
  assert.equal(mac.find((row) => row.id === 'toggle-hide')?.chords[0], '⌘⇧H');
  assert.equal(other.find((row) => row.id === 'toggle-lock')?.action, '锁定或解锁选中元素');
  assert.equal(mac.find((row) => row.id === 'toggle-hide')?.toolLabel, '画布');
  assert.equal(other.find((row) => row.id === 'help')?.chords[0], 'Shift+/');
  assert.equal(mac.find((row) => row.id === 'help')?.chords[0], '⇧/');

  const macText = mac.flatMap((row) => row.chords).join(' ');
  const otherText = other.flatMap((row) => row.chords).join(' ');
  assert.equal(macText.includes('Ctrl'), false);
  assert.equal(otherText.includes('⌘'), false);
  assert.equal(macText.includes('⇧'), true);
  assert.equal(otherText.includes('Shift'), true);

  assert.equal(mac.find((row) => row.id === 'tool-select')?.toolLabel, '选择');
  assert.equal(mac.find((row) => row.id === 'tool-line')?.tool, 'line');
  assert.equal(mac.find((row) => row.id === 'undo')?.toolLabel, '画布');
  assert.equal(mac.some((row) => row.id === 'temp-pan-up'), false);
});

test('formatChord and platform detection', () => {
  const undo = SHORTCUTS.find((shortcut) => shortcut.id === 'undo');
  assert.ok(undo);
  assert.equal(formatChord(undo, true), '⌘Z');
  assert.equal(formatChord(undo, false), 'Ctrl+Z');
  assert.equal(prefersMacModifier('MacIntel'), true);
  assert.equal(prefersMacModifier('iPhone'), true);
  assert.equal(prefersMacModifier('Win32'), false);
  assert.equal(prefersMacModifier('Linux x86_64'), false);
});

test('escape closes help before clearing the selection', () => {
  assert.equal(escapeAction(true), 'closeHelp');
  assert.equal(escapeAction(false), 'clearSelection');
});

test('keyboard zoom percent matches the camera scale, and only the stage steals browser zoom', () => {
  assert.equal(formatZoomPercent(1), '100%');
  assert.equal(formatZoomPercent(1.1), '110%');
  assert.equal(formatZoomPercent(0.1), '10%');
  assert.equal(formatZoomPercent(10), '1000%');
  assert.equal(formatZoomPercent(Number.NaN), '100%');

  const zoomIn = SHORTCUTS.find((shortcut) => shortcut.id === 'zoom-in');
  const zoomReset = SHORTCUTS.find((shortcut) => shortcut.id === 'zoom-reset');
  const shift100 = SHORTCUTS.find((shortcut) => shortcut.id === 'zoom-100');
  const undo = SHORTCUTS.find((shortcut) => shortcut.id === 'undo');
  assert.ok(zoomIn && zoomReset && shift100 && undo);
  assert.equal(zoomIn.stealsBrowserZoom, true);
  assert.equal(zoomReset.stealsBrowserZoom, true);
  assert.equal(shift100.stealsBrowserZoom, undefined);

  const body = { tagName: 'BODY' };
  const canvas = { tagName: 'CANVAS', id: 'canvas' };
  const stage = { tagName: 'DIV', id: 'stage' };
  const button = { tagName: 'BUTTON', id: 'undo-btn' };
  const input = { tagName: 'INPUT', id: 'prop-font-size' };
  const text = { tagName: 'TEXTAREA', id: 'text-editor' };
  const layers = { tagName: 'DIV', id: 'layer-list' };

  assert.equal(isCanvasStageFocus(body), true);
  assert.equal(isCanvasStageFocus(canvas), true);
  assert.equal(isCanvasStageFocus(stage), true);
  assert.equal(isCanvasStageFocus(button), false);
  assert.equal(isCanvasStageFocus(layers), false);
  assert.equal(shouldApplyShortcut(zoomIn, body), true);
  assert.equal(shouldApplyShortcut(zoomIn, canvas), true);
  assert.equal(shouldApplyShortcut(zoomReset, stage), true);
  assert.equal(shouldApplyShortcut(zoomIn, button), false);
  assert.equal(shouldApplyShortcut(zoomIn, input), false);
  assert.equal(shouldApplyShortcut(zoomIn, text), false);
  assert.equal(shouldApplyShortcut(zoomIn, layers), false);
  assert.equal(shouldApplyShortcut(undo, button), true);
  assert.equal(shouldApplyShortcut(undo, input), false);
  assert.equal(shouldApplyShortcut(shift100, button), true);
  assert.equal(shouldApplyShortcut(null, body), false);
});

test('editable focus covers input, textarea, and contenteditable', () => {
  assert.equal(isEditableTarget({ tagName: 'INPUT', parentElement: null }), true);
  assert.equal(isEditableTarget({ tagName: 'SELECT', parentElement: null }), true);
  assert.equal(isEditableTarget({ tagName: 'select', parentElement: null }), true);
  assert.equal(isEditableTarget({ tagName: 'input', parentElement: null }), true);
  assert.equal(isEditableTarget({ tagName: 'TEXTAREA', parentElement: null }), true);
  assert.equal(
    isEditableTarget({
      tagName: 'SPAN',
      parentElement: { tagName: 'TEXTAREA', parentElement: null },
    }),
    true,
  );
  assert.equal(
    isEditableTarget({
      tagName: 'DIV',
      isContentEditable: true,
      parentElement: null,
    }),
    true,
  );
  assert.equal(
    isEditableTarget({
      tagName: 'DIV',
      getAttribute: (name) => (name === 'contenteditable' ? '' : null),
      parentElement: null,
    }),
    true,
  );
  assert.equal(
    isEditableTarget({
      tagName: 'SPAN',
      getAttribute: (name) => (name === 'contenteditable' ? 'false' : null),
      parentElement: {
        tagName: 'DIV',
        isContentEditable: true,
        parentElement: null,
      },
    }),
    true,
  );
  assert.equal(
    isEditableTarget({
      tagName: 'DIV',
      getAttribute: (name) => (name === 'contenteditable' ? 'false' : null),
      parentElement: null,
    }),
    false,
  );
  assert.equal(isEditableTarget({ tagName: 'BUTTON', parentElement: null }), false);
  assert.equal(isEditableTarget({ tagName: 'CANVAS', parentElement: null }), false);
  assert.equal(isEditableTarget(null), false);
});

test('text editing shortcuts stay inside the text editor', () => {
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: 'Enter' }), 'down', 'text-editor')?.handler,
    'commitText',
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: 'Enter', shiftKey: true }), 'down', 'text-editor'),
    null,
  );
  assert.equal(
    matchShortcut(SHORTCUTS, keyEvent({ key: 'Escape' }), 'down', 'text-editor')?.handler,
    'cancelText',
  );
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'Enter' }), 'down') , null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'Escape' }), 'down')?.handler, 'escape');
  assert.equal(helpEntries(SHORTCUTS, false).some((row) => row.id === 'tool-text'), true);
  assert.equal(helpEntries(SHORTCUTS, false).find((row) => row.id === 'tool-polygon')?.toolLabel, '多边形');
  assert.equal(helpEntries(SHORTCUTS, false).find((row) => row.id === 'tool-star')?.chords[0], 'K');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'n' }), 'down')?.tool, 'polygon');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'k' }), 'down')?.tool, 'star');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'n', ctrlKey: true }), 'down'), null);
  assert.equal(helpEntries(SHORTCUTS, false).find((row) => row.id === 'text-commit')?.toolLabel, '文本');
});

test('the page dispatches every handler declared by the table', () => {
  const main = fs.readFileSync(new URL('./main.js', import.meta.url), 'utf8');
  assert.match(main, /from '\.\/shortcuts\.js'/);
  const handlers = new Set(SHORTCUTS.map((shortcut) => shortcut.handler));
  for (const handler of handlers) {
    assert.match(main, new RegExp(`['"]${handler}['"]`), handler);
  }
  assert.match(main, /zoom_by\(engine\.zoom_step\(\)\)/);
  assert.match(main, /zoom_by\(1 \/ engine\.zoom_step\(\)\)/);
  assert.match(main, /const step = engine\.zoom_step\(\)/);
  assert.match(main, /engine\.zoom_at\(point\.x, point\.y, factor\)/);
  assert.match(main, /formatZoomPercent\(engine\.get_zoom\(\)\)/);
  assert.match(main, /shouldApplyShortcut\(shortcut, event\.target\)/);
  assert.doesNotMatch(main, /deltaY < 0 \? 1\.1/);
});

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
  helpEntries,
  isEditableTarget,
  matchShortcut,
  prefersMacModifier,
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
  assert.equal(assertHandler('toggleMinimap', { key: 'm', code: 'KeyM' }).id, 'minimap');
  assert.equal(assertHandler('tool', { key: 'V' }).tool, 'select');
  assert.equal(assertHandler('tool', { key: 'v', shiftKey: true }).tool, 'select');
});

test('modifier letters do not switch tools', () => {
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', ctrlKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', metaKey: true }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'v', altKey: true }), 'down'), null);
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
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: 'g' }), 'down'), null);
  assert.equal(assertHandler('escape', { key: 'Escape' }).id, 'escape');
  assert.equal(assertHandler('zoomFit', { key: '!', code: 'Digit1', shiftKey: true }).id, 'zoom-fit');
  assert.equal(assertHandler('zoomFit', { key: '1', code: 'Digit1', shiftKey: true }).id, 'zoom-fit');
  assert.equal(assertHandler('zoom100', { key: ')', code: 'Digit0', shiftKey: true }).id, 'zoom-100');
  assert.equal(assertHandler('zoom100', { code: 'Digit0', shiftKey: true }).id, 'zoom-100');
  assert.equal(assertHandler('toggleHelp', { key: '?', code: 'Slash', shiftKey: true }).id, 'help');
  assert.equal(assertHandler('toggleHelp', { key: '/', code: 'Slash', shiftKey: true }).id, 'help');
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '1', code: 'Digit1' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '/', code: 'Slash' }), 'down'), null);
  assert.equal(matchShortcut(SHORTCUTS, keyEvent({ key: '?', code: 'Slash' }), 'down'), null);
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
  assert.equal(mac.find((row) => row.id === 'zoom-100')?.chords[0], '⇧0');
  assert.equal(other.find((row) => row.id === 'undo')?.chords[0], 'Ctrl+Z');
  assert.equal(mac.find((row) => row.id === 'select-all')?.chords[0], '⌘A');
  assert.equal(other.find((row) => row.id === 'group')?.chords[0], 'Ctrl+G');
  assert.equal(mac.find((row) => row.id === 'group')?.chords[0], '⌘G');
  assert.equal(other.find((row) => row.id === 'ungroup')?.chords[0], 'Ctrl+Shift+G');
  assert.equal(mac.find((row) => row.id === 'ungroup')?.chords[0], '⌘⇧G');
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

test('editable focus covers input, textarea, and contenteditable', () => {
  assert.equal(isEditableTarget({ tagName: 'INPUT', parentElement: null }), true);
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
  assert.equal(helpEntries(SHORTCUTS, false).find((row) => row.id === 'text-commit')?.toolLabel, '文本');
});

test('the page dispatches every handler declared by the table', () => {
  const main = fs.readFileSync(new URL('./main.js', import.meta.url), 'utf8');
  assert.match(main, /from '\.\/shortcuts\.js'/);
  const handlers = new Set(SHORTCUTS.map((shortcut) => shortcut.handler));
  for (const handler of handlers) {
    assert.match(main, new RegExp(`['"]${handler}['"]`), handler);
  }
});

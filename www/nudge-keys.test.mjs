import assert from 'node:assert/strict';
import test from 'node:test';
import { arrowNudge, isTypingTarget } from './nudge-keys.js';

test('arrow keys nudge by direction, and Shift is the fast step', () => {
  assert.deepEqual(arrowNudge({ key: 'ArrowRight' }), { direction: 'right', shift: false });
  assert.deepEqual(arrowNudge({ key: 'ArrowLeft', shiftKey: false }), {
    direction: 'left',
    shift: false,
  });
  assert.deepEqual(arrowNudge({ key: 'ArrowUp', shiftKey: true }), { direction: 'up', shift: true });
  assert.deepEqual(arrowNudge({ key: 'ArrowDown', shiftKey: true }), {
    direction: 'down',
    shift: true,
  });
});

test('arrows inside text inputs do not nudge the canvas', () => {
  for (const tagName of ['INPUT', 'input', 'TEXTAREA', 'SELECT']) {
    assert.equal(arrowNudge({ key: 'ArrowRight', shiftKey: true, target: { tagName } }), null);
  }
  assert.equal(
    arrowNudge({ key: 'ArrowDown', target: { tagName: 'DIV', isContentEditable: true } }),
    null,
  );
  assert.equal(isTypingTarget({ tagName: 'BODY' }), false);
  assert.deepEqual(arrowNudge({ key: 'ArrowLeft', target: { tagName: 'BODY' } }), {
    direction: 'left',
    shift: false,
  });
});

test('arrow nudge does not take undo, delete, space, or tool shortcuts', () => {
  assert.equal(arrowNudge({ key: 'z', ctrlKey: true }), null);
  assert.equal(arrowNudge({ key: 'c', ctrlKey: true }), null);
  assert.equal(arrowNudge({ key: 'v', ctrlKey: true }), null);
  assert.equal(arrowNudge({ key: 'd', metaKey: true }), null);
  assert.equal(arrowNudge({ key: 'z', metaKey: true, shiftKey: true }), null);
  assert.equal(arrowNudge({ key: 'ArrowRight', ctrlKey: true }), null);
  assert.equal(arrowNudge({ key: 'ArrowLeft', metaKey: true }), null);
  assert.equal(arrowNudge({ key: 'ArrowDown', altKey: true }), null);
  assert.equal(arrowNudge({ key: ' ' }), null);
  assert.equal(arrowNudge({ key: 'v' }), null);
  assert.equal(arrowNudge({ key: 'V', shiftKey: true }), null);
  assert.equal(arrowNudge({ key: 'Delete' }), null);
  assert.equal(arrowNudge({ key: 'Backspace' }), null);
  assert.equal(arrowNudge(null), null);
});

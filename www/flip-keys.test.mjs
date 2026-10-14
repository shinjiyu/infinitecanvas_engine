import assert from 'node:assert/strict';
import test from 'node:test';
import { orientAction } from './flip-keys.js';

test('shift combinations flip or rotate the selection', () => {
  assert.deepEqual(orientAction({ key: 'h', shiftKey: true }), {
    kind: 'flip',
    axis: 'horizontal',
  });
  assert.deepEqual(orientAction({ key: 'V', shiftKey: true }), {
    kind: 'flip',
    axis: 'vertical',
  });
  assert.deepEqual(orientAction({ key: 'R', shiftKey: true }), { kind: 'rotate', dir: 'cw' });
  assert.deepEqual(orientAction({ key: 'r', shiftKey: true, altKey: true }), {
    kind: 'rotate',
    dir: 'ccw',
  });
});

test('tool keys, typing, and modified chords do not flip', () => {
  assert.equal(orientAction({ key: 'h' }), null);
  assert.equal(orientAction({ key: 'v' }), null);
  assert.equal(orientAction({ key: 'r' }), null);
  assert.equal(orientAction({ key: 'h', shiftKey: true, ctrlKey: true }), null);
  assert.equal(orientAction({ key: 'r', shiftKey: true, metaKey: true }), null);
  assert.equal(orientAction({ key: 'r', altKey: true }), null);
  assert.equal(orientAction({ key: 'ArrowRight', shiftKey: true }), null);
  assert.equal(orientAction({ key: 'z', ctrlKey: true }), null);
  assert.equal(orientAction({ key: 'h', shiftKey: true, repeat: true }), null);
  assert.equal(orientAction({ key: 'H', shiftKey: true, target: { tagName: 'INPUT' } }), null);
  assert.equal(
    orientAction({ key: 'v', shiftKey: true, target: { tagName: 'DIV', isContentEditable: true } }),
    null,
  );
  assert.equal(orientAction(null), null);
});

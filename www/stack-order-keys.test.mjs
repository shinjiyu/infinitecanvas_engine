import assert from 'node:assert/strict';
import test from 'node:test';
import { stackOrderAction } from './stack-order-keys.js';

test('bracket keys map to forward, backward, front, and back', () => {
  assert.equal(stackOrderAction({ key: ']', code: 'BracketRight' }), 'forward');
  assert.equal(stackOrderAction({ key: '[', code: 'BracketLeft' }), 'backward');
  assert.equal(stackOrderAction({ key: ']', code: 'BracketRight', ctrlKey: true }), 'front');
  assert.equal(stackOrderAction({ key: '[', code: 'BracketLeft', metaKey: true }), 'back');
  assert.equal(stackOrderAction({ key: 'ü', code: 'BracketLeft' }), 'backward');
  assert.equal(stackOrderAction({ key: ']', code: 'BracketRight', ctrlKey: true, metaKey: true }), 'front');
});

test('stack order keys ignore typing, shift, alt, and unrelated shortcuts', () => {
  assert.equal(stackOrderAction({ key: ']', code: 'BracketRight', target: { tagName: 'INPUT' } }), null);
  assert.equal(
    stackOrderAction({ key: '[', code: 'BracketLeft', target: { tagName: 'DIV', isContentEditable: true } }),
    null,
  );
  assert.equal(stackOrderAction({ key: '}', code: 'BracketRight', shiftKey: true }), null);
  assert.equal(stackOrderAction({ key: ']', code: 'BracketRight', altKey: true }), null);
  assert.equal(stackOrderAction({ key: ']', code: 'BracketRight', ctrlKey: true, shiftKey: true }), null);
  assert.equal(stackOrderAction({ key: 'z', ctrlKey: true }), null);
  assert.equal(stackOrderAction({ key: 'c', metaKey: true }), null);
  assert.equal(stackOrderAction({ key: 'v' }), null);
  assert.equal(stackOrderAction({ key: 'ArrowRight' }), null);
  assert.equal(stackOrderAction({ key: ' ', code: 'Space' }), null);
  assert.equal(stackOrderAction(null), null);
});

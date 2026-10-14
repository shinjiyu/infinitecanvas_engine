import assert from 'node:assert/strict';
import test from 'node:test';
import { rulerKeyAction } from './ruler-keys.js';

test('G toggles rulers and Delete or Backspace removes a guide', () => {
  assert.equal(rulerKeyAction({ key: 'g' }), 'toggle-rulers');
  assert.equal(rulerKeyAction({ key: 'G' }), 'toggle-rulers');
  assert.equal(rulerKeyAction({ key: 'Delete' }), 'delete-guide');
  assert.equal(rulerKeyAction({ key: 'Backspace' }), 'delete-guide');
});

test('modified keys, typing, and other shortcuts do not touch rulers', () => {
  assert.equal(rulerKeyAction({ key: 'g', ctrlKey: true }), null);
  assert.equal(rulerKeyAction({ key: 'g', metaKey: true }), null);
  assert.equal(rulerKeyAction({ key: 'g', shiftKey: true }), null);
  assert.equal(rulerKeyAction({ key: 'g', altKey: true }), null);
  assert.equal(rulerKeyAction({ key: 'g', repeat: true }), null);
  assert.equal(rulerKeyAction({ key: 'm' }), null);
  assert.equal(rulerKeyAction({ key: 'Delete', target: { tagName: 'INPUT' } }), null);
  assert.equal(rulerKeyAction({ key: 'g', target: { tagName: 'TEXTAREA' } }), null);
  assert.equal(rulerKeyAction(null), null);
});

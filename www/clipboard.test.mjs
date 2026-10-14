import assert from 'node:assert/strict';
import test from 'node:test';
import {
  PASTE_STEP,
  createPasteSession,
  pasteOffset,
  readTextClipboard,
  screenToWorld,
  writeTextClipboard,
} from './clipboard.js';

test('paste lands on the pointer first, then steps away', () => {
  assert.deepEqual(pasteOffset(true, 0), { dx: 0, dy: 0 });
  assert.deepEqual(pasteOffset(true, 1), { dx: PASTE_STEP, dy: PASTE_STEP });
  assert.deepEqual(pasteOffset(true, 2), { dx: 20, dy: 20 });
});

test('paste away from the canvas starts at +10 and keeps stepping', () => {
  assert.deepEqual(pasteOffset(false, 0), { dx: 10, dy: 10 });
  assert.deepEqual(pasteOffset(false, 1), { dx: 20, dy: 20 });
});

test('a new copy resets successive paste offsets', () => {
  const session = createPasteSession();
  session.arm('a');
  assert.deepEqual(session.nextPlacement('a', false), { dx: 10, dy: 10 });
  assert.deepEqual(session.nextPlacement('a', false), { dx: 20, dy: 20 });
  session.arm('a');
  assert.deepEqual(session.nextPlacement('a', true), { dx: 0, dy: 0 });
  assert.deepEqual(session.nextPlacement('b', true), { dx: 0, dy: 0 });
  assert.deepEqual(session.nextPlacement('b', true), { dx: 10, dy: 10 });
});

test('screen center maps to the viewport center', () => {
  const world = screenToWorld(
    { x: 400, y: 300 },
    { width: 800, height: 600 },
    { x: 12, y: -4 },
    2,
  );
  assert.equal(world.x, 12);
  assert.equal(world.y, -4);
  const corner = screenToWorld(
    { x: 800, y: 600 },
    { width: 800, height: 600 },
    { x: 0, y: 0 },
    1,
  );
  assert.equal(corner.x, 400);
  assert.equal(corner.y, 300);
});

test('clipboard permission failure falls back to memory without throwing', async () => {
  const denied = {
    writeText: async () => {
      throw new Error('denied');
    },
    readText: async () => {
      throw new Error('denied');
    },
  };
  await writeTextClipboard('{"version":1,"elements":[]}', denied);
  assert.equal(await readTextClipboard(denied), '{"version":1,"elements":[]}');
});

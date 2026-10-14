import assert from 'node:assert/strict';
import test from 'node:test';
import { dropIndex, flattenTree, idKey, indexTree, visibleRange } from './layer-panel.js';

function sampleTree() {
  return [
    { id: 1, elementType: 'Rectangle', visible: true, locked: false, children: [] },
    { id: 2, elementType: 'Rectangle', visible: true, locked: false, children: [] },
    {
      id: 3,
      elementType: 'Group',
      visible: true,
      locked: false,
      children: [
        { id: 4, elementType: 'Rectangle', visible: true, locked: false, children: [] },
        { id: 5, elementType: 'Rectangle', visible: true, locked: false, children: [] },
      ],
    },
  ];
}

test('flatten lists the front-most root first and nests children', () => {
  const roots = sampleTree();
  const rows = flattenTree(roots, new Set());
  assert.deepEqual(rows.map((row) => row.id), ['3', '5', '4', '2', '1']);
  assert.equal(rows[0].storageIndex, 2);
  assert.equal(rows[1].depth, 1);
  assert.equal(rows[1].parentKey, '3');
});

test('collapsed groups hide their children', () => {
  const rows = flattenTree(sampleTree(), new Set(['3']));
  assert.deepEqual(rows.map((row) => row.id), ['3', '2', '1']);
});

test('drop before the front row brings a back root forward', () => {
  const roots = sampleTree();
  const rows = flattenTree(roots, new Set());
  const tree = indexTree(roots);
  const target = dropIndex(rows, 0, 'before', '1', tree);
  assert.deepEqual(target, { parentId: 0, index: 2 });
});

test('drop into a group appends the element at the front of that group', () => {
  const roots = sampleTree();
  const rows = flattenTree(roots, new Set());
  const tree = indexTree(roots);
  const target = dropIndex(rows, 0, 'into', '1', tree);
  assert.equal(idKey(target.parentId), '3');
  assert.equal(target.index, 2);
});

test('drop after the last row sends an element to the back of its parent', () => {
  const roots = sampleTree();
  const rows = flattenTree(roots, new Set());
  const tree = indexTree(roots);
  const target = dropIndex(rows, rows.length, 'after', '5', tree);
  assert.equal(target.parentId, 0);
  assert.equal(target.index, 0);
});

test('drop rejects a locked ancestor and dropping a group into itself', () => {
  const lockedRoots = sampleTree();
  lockedRoots[2].locked = true;
  const lockedRows = flattenTree(lockedRoots, new Set());
  assert.equal(dropIndex(lockedRows, 3, 'before', '5', indexTree(lockedRoots)), null);

  const roots = sampleTree();
  const rows = flattenTree(roots, new Set());
  assert.equal(dropIndex(rows, 0, 'into', '3', indexTree(roots)), null);
});

test('virtual window stays small for ten thousand rows', () => {
  const top = visibleRange(0, 280, 10_000);
  assert.ok(top.end - top.start < 40);
  const mid = visibleRange(5000, 280, 10_000);
  assert.ok(mid.end - mid.start < 40);
  assert.ok(mid.start > 100);
  assert.ok(mid.end < 10_000);
});

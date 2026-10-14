import assert from 'node:assert/strict';
import test from 'node:test';
import {
  defaultGradientStops,
  fillEnabledPatch,
  fillKindPatch,
  insertStop,
  normalizeStops,
} from './gradient-fill.js';

test('default stops contrast with the current solid fill', () => {
  assert.deepEqual(defaultGradientStops('#336699ff'), [
    { offset: 0, color: '#336699' },
    { offset: 1, color: '#ffffff' },
  ]);
  assert.equal(defaultGradientStops('#ffffff')[1].color, '#1a1a1a');
  assert.equal(defaultGradientStops(null)[0].color, '#000000');
});

test('normalize sorts, clamps, and caps the stop list', () => {
  const stops = normalizeStops([
    { offset: 2, color: '#0000ff' },
    { offset: Number.NaN, color: '#ff0000' },
    { offset: -1, color: '#00ff00aa' },
  ]);
  assert.deepEqual(stops, [
    { offset: 0, color: '#00ff00' },
    { offset: 1, color: '#0000ff' },
  ]);
});

test('insert puts a stop in the widest gap and stops at eight', () => {
  const next = insertStop([
    { offset: 0, color: '#ff0000' },
    { offset: 0.2, color: '#00ff00' },
    { offset: 1, color: '#0000ff' },
  ]);
  assert.equal(next.length, 4);
  assert.equal(next[2].offset, 0.6);
  assert.equal(next[2].color, '#00ff00');
  const full = Array.from({ length: 8 }, (_, index) => ({
    offset: index / 7,
    color: '#000000',
  }));
  assert.equal(insertStop(full).length, 8);
});

test('disabling fill clears the gradient kind and enabling fill is solid', () => {
  assert.deepEqual(fillEnabledPatch(false, '#ff0000'), { fill: null, fillKind: 'solid' });
  assert.deepEqual(fillEnabledPatch(true, '#112233'), { fill: '#112233', fillKind: 'solid' });
});

test('kind patch reuses stops and seeds a gradient when the fill was off', () => {
  assert.deepEqual(fillKindPatch('solid', {}), { fillKind: 'solid' });
  const linear = fillKindPatch('linear', {
    fill: '#ff0000',
    fillNone: true,
    fillAngle: 90,
  });
  assert.equal(linear.fillKind, 'linear');
  assert.equal(linear.fillAngle, 90);
  assert.equal(linear.fill, '#ff0000');
  assert.equal(linear.fillStops.length, 2);
  const radial = fillKindPatch('radial', {
    fillStops: [
      { offset: 0, color: '#ff0000' },
      { offset: 1, color: '#0000ff' },
    ],
    fillCenter: { x: 2, y: -3 },
    fillRadius: 12,
  });
  assert.equal(radial.fillKind, 'radial');
  assert.deepEqual(radial.fillCenter, { x: 2, y: -3 });
  assert.equal(radial.fillRadius, 12);
  assert.equal(radial.fill, undefined);
  assert.equal(fillKindPatch('nope', {}), null);
});

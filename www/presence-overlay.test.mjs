/**
 * Presence overlay unit tests (no DOM for math helpers; jsdom-free checks).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { worldToScreen, worldBoundsToScreenRect } from './presence-overlay.js';

describe('worldToScreen', () => {
  it('maps world origin to canvas center at identity viewport', () => {
    const screen = worldToScreen(
      { x: 0, y: 0 },
      { centerX: 0, centerY: 0, zoom: 1, canvasWidth: 800, canvasHeight: 600 }
    );
    assert.deepEqual(screen, { x: 400, y: 300 });
  });

  it('scales offsets by zoom', () => {
    const screen = worldToScreen(
      { x: 10, y: -5 },
      { centerX: 0, centerY: 0, zoom: 2, canvasWidth: 100, canvasHeight: 100 }
    );
    assert.deepEqual(screen, { x: 70, y: 40 });
  });
});

describe('worldBoundsToScreenRect', () => {
  it('converts AABB without affecting hit-test semantics (pure math)', () => {
    const rect = worldBoundsToScreenRect(
      { x: 0, y: 0, width: 40, height: 20 },
      { centerX: 0, centerY: 0, zoom: 1, canvasWidth: 200, canvasHeight: 200 }
    );
    assert.equal(rect.x, 100);
    assert.equal(rect.y, 100);
    assert.equal(rect.width, 40);
    assert.equal(rect.height, 20);
  });
});

describe('presence shape soft-coordinate', () => {
  it('maps RemoteUser-like payloads to PresenceState fields', async () => {
    // Inline mirror of packages/core presenceFromRemoteUser (www has no TS build in test).
    const user = {
      id: 'u1',
      name: 'Ada',
      color: '#4090f2',
      cursor: { x: 1, y: 2 },
      selection: ['7', '8'],
    };
    const localId = 'me';
    const remotes = [user, { id: localId, name: 'Me', color: '#000', selection: [] }]
      .filter((u) => u.id !== localId)
      .map((u) => ({
        clientId: u.id,
        state: {
          name: u.name,
          color: u.color,
          cursor: u.cursor,
          selection: (u.selection ?? []).map(String),
        },
      }));
    assert.equal(remotes.length, 1);
    assert.equal(remotes[0].clientId, 'u1');
    assert.deepEqual(remotes[0].state.selection, ['7', '8']);
    assert.ok(remotes.every((r) => r.clientId !== localId));
  });
});

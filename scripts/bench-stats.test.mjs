import assert from 'node:assert/strict';
import test from 'node:test';
import { exitCode } from './bench.mjs';
import { average, percentile, summarizeFrames, sustains60Fps } from './bench-stats.mjs';

test('percentile uses the nearest-rank index on an unsorted sample', () => {
    assert.equal(percentile([10, 30, 20, 40], 0.5), 20);
    assert.equal(percentile([10, 30, 20, 40], 0.95), 40);
    assert.equal(percentile([5], 0.95), 5);
    assert.equal(percentile([], 0.95), null);
});

test('average ignores nothing and rejects empty input with null', () => {
    assert.equal(average([2, 4, 6]), 4);
    assert.equal(average([]), null);
});

test('60 FPS requires both average FPS and a tight P95', () => {
    assert.equal(sustains60Fps(1000 / 60, 16.7), true);
    assert.equal(sustains60Fps(1000 / 58, 18), true);
    assert.equal(sustains60Fps(1000 / 50, 16), false);
    assert.equal(sustains60Fps(1000 / 60, 33), false);
    assert.equal(sustains60Fps(null, 16), false);
});

test('100k may fail without failing a 1k and 10k run', () => {
    const report = { scales: [{ elements: 1000 }, { elements: 10000 }] };
    assert.equal(exitCode(report, [1000, 10000, 100000]), 0);
    assert.equal(exitCode({ scales: [{ elements: 1000 }] }, [1000, 10000, 100000]), 1);
});

test('a run that only asks for 100k still requires that scale', () => {
    assert.equal(exitCode({ scales: [] }, [100000]), 1);
    assert.equal(exitCode({ scales: [{ elements: 100000 }] }, [100000]), 0);
});

test('summarizeFrames reports submit time separately from the rAF interval', () => {
    const summary = summarizeFrames([16, 17, 16, 18], [1, 1, 2, 1]);
    assert.equal(summary.samples, 4);
    assert.equal(summary.sustained60, true);
    assert.ok(summary.avgSubmitMs != null && summary.avgSubmitMs < 2);
    assert.ok(summary.avgFps != null && summary.avgFps > 55);
});

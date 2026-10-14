/**
 * 帧时间汇总。压测页和 Node 脚本共用同一套百分位定义，避免 README 数字和脚本对不上。
 */

/**
 * 最近秩百分位。`samples` 不必预先排序。
 *
 * p = 0.95 时取排序后下标 `ceil(0.95 * n) - 1`。
 * 空数组返回 `null`。
 *
 * @param {number[]} samples
 * @param {number} p 0 到 1
 * @returns {number | null}
 *
 * @example
 * percentile([10, 30, 20], 0.95) // 30
 */
export function percentile(samples, p) {
    if (!Array.isArray(samples) || samples.length === 0) {
        return null;
    }
    if (p < 0 || p > 1) {
        throw new Error(`percentile p must be between 0 and 1, got ${p}`);
    }
    const sorted = [...samples].sort((a, b) => a - b);
    const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1));
    return sorted[index];
}

/**
 * 算术平均。空数组返回 `null`。
 *
 * @param {number[]} samples
 * @returns {number | null}
 */
export function average(samples) {
    if (!Array.isArray(samples) || samples.length === 0) {
        return null;
    }
    let sum = 0;
    for (const value of samples) {
        sum += value;
    }
    return sum / samples.length;
}

/**
 * 是否接近 60 FPS。
 *
 * 判据（同时满足）：
 * - 平均帧间隔对应的 FPS ≥ 55
 * - P95 帧间隔 ≤ 20ms
 *
 * 20ms 给软件渲染或合成器偶发长帧留一点余量，但仍要求大多数帧留在 60Hz 预算附近。
 * 不把「平均很快、少数帧极慢」记成达标。
 *
 * @param {number | null} avgFrameMs rAF 帧间隔平均
 * @param {number | null} p95FrameMs rAF 帧间隔 P95
 * @returns {boolean}
 */
export function sustains60Fps(avgFrameMs, p95FrameMs) {
    if (avgFrameMs == null || p95FrameMs == null || avgFrameMs <= 0) {
        return false;
    }
    const avgFps = 1000 / avgFrameMs;
    return avgFps >= 55 && p95FrameMs <= 20;
}

/**
 * @param {number | null} value
 * @param {number} digits
 * @returns {string}
 */
export function formatMs(value, digits = 2) {
    if (value == null || Number.isNaN(value)) {
        return 'n/a';
    }
    return value.toFixed(digits);
}

/**
 * 把一串帧间隔收成 README 用的摘要。
 *
 * @param {number[]} frameIntervalsMs rAF 间隔，含浏览器等待，单位毫秒
 * @param {number[]} submitMs `render()` 调用耗时，单位毫秒
 * @returns {{
 *   samples: number,
 *   avgFrameMs: number | null,
 *   p95FrameMs: number | null,
 *   avgFps: number | null,
 *   avgSubmitMs: number | null,
 *   p95SubmitMs: number | null,
 *   sustained60: boolean,
 * }}
 */
export function summarizeFrames(frameIntervalsMs, submitMs) {
    const avgFrameMs = average(frameIntervalsMs);
    const p95FrameMs = percentile(frameIntervalsMs, 0.95);
    const avgSubmitMs = average(submitMs);
    const p95SubmitMs = percentile(submitMs, 0.95);
    return {
        samples: frameIntervalsMs.length,
        avgFrameMs,
        p95FrameMs,
        avgFps: avgFrameMs && avgFrameMs > 0 ? 1000 / avgFrameMs : null,
        avgSubmitMs,
        p95SubmitMs,
        sustained60: sustains60Fps(avgFrameMs, p95FrameMs),
    };
}

/**
 * 一键帧时间压测。
 *
 * 起一个静态服务，用本机 Chrome（headless + WebGPU）打开 `www/bench.html`，
 * 把 1k / 10k / 100k 矩形场景的平均与 P95 帧时间打到 stdout。
 *
 * ```bash
 * node scripts/bench.mjs            # 1k、10k、100k
 * node scripts/bench.mjs --smoke    # 只跑 1k，不要求 60 FPS
 * node scripts/bench.mjs --counts 1000,10000 --out docs/perf/run.json
 * ```
 *
 * 前置：`cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm`
 * Chrome 需要能创建 WebGPU 适配器。无 GPU 时脚本会加上 SwiftShader 相关参数。
 * 退出码：页面崩溃、WebGPU 初始化失败，或稳态/平移后仍在重生成顶点时为 1。
 * 某一档（例如 100k）加载失败会记在结果里，不单独导致失败；1k 和 10k 都没有成功采样则失败。
 */
import { execSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { connectPage, waitForDebugger } from './cdp.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const wasmPath = path.join(repoRoot, 'packages/core/wasm/infinitecanvas_wasm_bg.wasm');

const MIME = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.mjs': 'text/javascript; charset=utf-8',
    '.wasm': 'application/wasm',
    '.json': 'application/json',
    '.css': 'text/css; charset=utf-8',
};

/**
 * @param {string[]} argv
 */
function parseArgs(argv) {
    const options = {
        smoke: false,
        counts: null,
        frames: null,
        warmup: null,
        out: null,
        markdown: null,
        tight: false,
        readback: false,
        screenshot: null,
    };
    for (let index = 0; index < argv.length; index += 1) {
        const arg = argv[index];
        if (arg === '--smoke') {
            options.smoke = true;
        } else if (arg === '--counts') {
            options.counts = argv[++index];
        } else if (arg === '--frames') {
            options.frames = argv[++index];
        } else if (arg === '--warmup') {
            options.warmup = argv[++index];
        } else if (arg === '--out') {
            options.out = argv[++index];
        } else if (arg === '--markdown') {
            options.markdown = argv[++index];
        } else if (arg === '--tight') {
            options.tight = true;
        } else if (arg === '--readback') {
            options.readback = true;
        } else if (arg === '--screenshot') {
            options.screenshot = argv[++index];
        } else {
            throw new Error(`未知参数 ${arg}`);
        }
    }
    return options;
}

/**
 * @returns {Promise<number>}
 */
function freePort() {
    return new Promise((resolve, reject) => {
        const server = net.createServer();
        server.listen(0, '127.0.0.1', () => {
            const address = server.address();
            const port = typeof address === 'object' && address ? address.port : 0;
            server.close((error) => (error ? reject(error) : resolve(port)));
        });
    });
}

/**
 * 仓库根目录的静态文件服务。阻止 `..` 逃出仓库。
 *
 * @param {number} port
 */
function startStaticServer(port) {
    const server = createServer(async (request, response) => {
        try {
            const url = new URL(request.url || '/', `http://127.0.0.1:${port}`);
            const relative = decodeURIComponent(url.pathname);
            const filePath = path.resolve(repoRoot, `.${relative}`);
            if (filePath !== repoRoot && !filePath.startsWith(`${repoRoot}${path.sep}`)) {
                response.writeHead(403);
                response.end('forbidden');
                return;
            }
            const data = await readFile(filePath);
            const type = MIME[path.extname(filePath)] || 'application/octet-stream';
            response.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' });
            response.end(data);
        } catch {
            response.writeHead(404);
            response.end('not found');
        }
    });
    return new Promise((resolve) => {
        server.listen(port, '127.0.0.1', () => resolve(server));
    });
}

/**
 * @returns {string}
 */
function findChrome() {
    const candidates = [
        process.env.CHROME_PATH,
        'google-chrome',
        'google-chrome-stable',
        '/usr/bin/google-chrome',
        '/usr/bin/google-chrome-stable',
        '/usr/local/bin/google-chrome',
    ].filter(Boolean);
    for (const candidate of candidates) {
        try {
            execSync(`command -v ${candidate}`, { stdio: 'ignore' });
            return candidate;
        } catch {
            if (candidate.startsWith('/') && existsSync(candidate)) {
                return candidate;
            }
        }
    }
    throw new Error('找不到 Chrome。设置 CHROME_PATH，或安装 google-chrome。');
}

/**
 * @param {string} command
 * @returns {string}
 */
function capture(command) {
    try {
        return execSync(command, { cwd: repoRoot, encoding: 'utf8' }).trim();
    } catch {
        return '';
    }
}

/**
 * @returns {string}
 */
function cpuModel() {
    try {
        const text = execSync('cat /proc/cpuinfo', { encoding: 'utf8' });
        const line = text.split('\n').find((row) => row.startsWith('model name'));
        return line ? line.split(':').slice(1).join(':').trim() : os.arch();
    } catch {
        return os.arch();
    }
}

/**
 * @param {import('node:child_process').ChildProcess} child
 * @param {number} debugPort
 * @param {string} url
 */
async function launchChrome(childPath, debugPort, url) {
    const userData = path.join(os.tmpdir(), `infinitecanvas-bench-${debugPort}`);
    const args = [
        '--headless=new',
        '--no-sandbox',
        '--disable-dev-shm-usage',
        '--disable-gpu-sandbox',
        '--enable-unsafe-webgpu',
        '--use-angle=swiftshader',
        '--ignore-gpu-blocklist',
        '--enable-webgpu-developer-features',
        '--enable-precise-memory-info',
        '--disable-background-timer-throttling',
        '--disable-backgrounding-occluded-windows',
        '--disable-renderer-backgrounding',
        '--no-first-run',
        '--no-default-browser-check',
        '--window-size=1280,800',
        `--user-data-dir=${userData}`,
        `--remote-debugging-port=${debugPort}`,
        url,
    ];
    const child = spawn(childPath, args, { stdio: ['ignore', 'pipe', 'pipe'] });
    let stderr = '';
    child.stderr.on('data', (chunk) => {
        stderr = (stderr + chunk.toString()).slice(-4000);
    });
    return { child, args, stderr: () => stderr };
}

/**
 * @param {import('./cdp.mjs').CdpSession} session
 * @param {number} timeoutMs
 */
async function waitForBench(session, timeoutMs) {
    const started = Date.now();
    let lastStatus = '';
    while (Date.now() - started < timeoutMs) {
        const status = await session.send('Runtime.evaluate', {
            expression: 'window.__BENCH_STATUS__ || ""',
            returnByValue: true,
        });
        const statusText = status.result?.value || '';
        if (statusText && statusText !== lastStatus) {
            console.error(`[bench] ${statusText}`);
            lastStatus = statusText;
        }
        const error = await session.send('Runtime.evaluate', {
            expression: 'window.__BENCH_ERROR__ || null',
            returnByValue: true,
        });
        const result = await session.send('Runtime.evaluate', {
            expression: 'window.__BENCH_RESULT__ || null',
            returnByValue: true,
        });
        if (result.result?.value) {
            return result.result.value;
        }
        if (error.result?.value && !result.result?.value) {
            const recent = session.exceptions.slice(-3).join('\n');
            throw new Error(`${error.result.value}${recent ? `\n${recent}` : ''}`);
        }
        if (session.exceptions.length > 0 && Date.now() - started > 5000) {
            const text = session.exceptions[session.exceptions.length - 1];
            if (/Initialization failed|WebGPU|wasm/i.test(text)) {
                throw new Error(text);
            }
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
    }
    const logs = session.consoleMessages.slice(-20).join('\n');
    throw new Error(`压测超时（${timeoutMs}ms）\n${logs}`);
}

/**
 * @param {any} report
 * @returns {string}
 */
function toMarkdown(report) {
    const lines = [
        `# 性能记录 ${report.date}`,
        '',
        `- commit: \`${report.commit || 'unknown'}\``,
        `- 系统: ${report.os}`,
        `- CPU: ${report.cpu}`,
        `- 浏览器: ${report.chrome}`,
        `- GPU / WebGPU: ${report.adapter.description || 'n/a'}（architecture ${report.adapter.architecture || 'n/a'}，fallback ${report.adapter.isFallbackAdapter}）`,
        `- 画布: ${report.canvas.width}×${report.canvas.height}`,
        `- 发布版 WASM: ${report.wasmBytes} bytes（\`infinitecanvas_wasm_bg.wasm\`）`,
        `- rustc: ${report.rustc || 'n/a'}`,
        `- wasm-pack: ${report.wasmPack || 'n/a'}`,
        '',
        '场景是 `load_stress_scene` 的填充矩形（400 列，间距 24，尺寸 16×12）。',
        '稳态帧在顶点缓存建好之后采样。帧间隔是相邻两次 `render()` 返回的时间，含等待下一帧；第一段间隔丢掉。',
        '提交时间只包住 `render()`。它在 `queue.submit` 和 `present` 之后返回，没有 GPU 时间戳。',
        '这次 headless SwiftShader 上，`present` 没有把光栅时间算进 `render()`。',
        '约 60 FPS：平均 FPS ≥ 55，且 P95 帧间隔 ≤ 20ms。',
        '绘制路径提交顶点缓存里的全部矩形。没有关闭脏标记。稳态和平移后 `geometryUnchanged` 都应为 true。',
        '',
        '| 元素 | 平均帧间隔 ms | P95 帧间隔 ms | 平均 FPS | ~60 FPS | 平均提交 ms | P95 提交 ms | 冷启动帧 ms | WASM 堆 | JS 堆 used |',
        '| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |',
    ];
    for (const scale of report.scales) {
        if (scale.error) {
            lines.push(`| ${scale.elements} | 失败 |  |  |  |  |  |  |  |  |`);
            lines.push('');
            lines.push(`\`${scale.elements}\` 失败原因：${scale.error}`);
            continue;
        }
        lines.push(
            `| ${scale.elements} | ${num(scale.avgFrameMs)} | ${num(scale.p95FrameMs)} | ${num(scale.avgFps, 1)} | ${scale.sustained60 ? 'yes' : 'no'} | ${num(scale.avgSubmitMs)} | ${num(scale.p95SubmitMs)} | ${num(scale.coldFrameMs)} | ${scale.memory.wasmHeapBytes} | ${scale.memory.jsHeapUsedBytes ?? 'n/a'} |`,
        );
    }
    if (report.failures.length > 0) {
        lines.push('');
        lines.push('## 未完成的规模');
        for (const failure of report.failures) {
            lines.push(`- ${failure.elements}: ${failure.error}`);
        }
    }
    lines.push('');
    return `${lines.join('\n')}\n`;
}

/**
 * @param {number | null | undefined} value
 * @param {number} digits
 * @returns {string}
 */
function num(value, digits = 2) {
    if (value == null || Number.isNaN(value)) {
        return 'n/a';
    }
    return Number(value).toFixed(digits);
}

/**
 * 页面一次跑完所有规模。某一档抛错时页面会整段失败。
 * 为了让 100k 的失败不影响已经得到的 1k/10k，这里按规模分开打开页面。
 *
 * @param {object} options
 * @param {number[]} options.counts
 * @param {boolean} options.smoke
 * @param {string | null} options.frames
 * @param {string | null} options.warmup
 */
async function runScales(options) {
    const chromePath = findChrome();
    const chromeVersion = capture(`${chromePath} --version`);
    const httpPort = await freePort();
    const server = await startStaticServer(httpPort);
    const scales = [];
    const failures = [];
    /** @type {any} */
    let environment = null;
    try {
        for (const count of options.counts) {
            const debugPort = await freePort();
            const query = new URLSearchParams({
                autorun: '1',
                counts: String(count),
            });
            if (options.frames) {
                query.set('frames', options.frames);
            } else if (options.smoke) {
                query.set('frames', '20');
            }
            if (options.warmup) {
                query.set('warmup', options.warmup);
            } else if (options.smoke) {
                query.set('warmup', '5');
            }
            if (options.tight) {
                query.set('tight', '1');
            }
            if (options.readback) {
                query.set('readback', '1');
            }
            const url = `http://127.0.0.1:${httpPort}/www/bench.html?${query}`;
            const launched = await launchChrome(chromePath, debugPort, url);
            try {
                await waitForDebugger(debugPort, 20000);
                const session = await connectPage(debugPort);
                const timeoutMs = options.smoke ? 90_000 : count >= 100_000 ? 15 * 60_000 : 5 * 60_000;
                const pageResult = await waitForBench(session, timeoutMs);
                if (options.screenshot && count === 10000) {
                    const shot = await session.send('Page.captureScreenshot', { format: 'png' });
                    const destination = path.resolve(repoRoot, options.screenshot);
                    await mkdir(path.dirname(destination), { recursive: true });
                    await writeFile(destination, Buffer.from(shot.data, 'base64'));
                    console.error(`[bench] screenshot ${destination}`);
                }
                session.close();
                if (!pageResult.ok) {
                    throw new Error(pageResult.error || 'bench result not ok');
                }
                const scale = pageResult.scales[0];
                scales.push(scale);
                environment = environment || {
                    canvas: pageResult.canvas,
                    userAgent: pageResult.userAgent,
                    adapter: pageResult.adapter,
                    warmup: pageResult.warmup,
                    frames: pageResult.frames,
                };
                console.error(
                    `[bench] ${count} avg ${num(scale.avgFrameMs)} ms  p95 ${num(scale.p95FrameMs)} ms  fps ${num(scale.avgFps, 1)}  60fps ${scale.sustained60}`,
                );
            } catch (error) {
                const message = error instanceof Error ? error.message : String(error);
                console.error(`[bench] ${count} 失败: ${message}`);
                failures.push({ elements: count, error: message });
            } finally {
                launched.child.kill('SIGKILL');
            }
        }
    } finally {
        server.close();
    }

    const wasmStat = await stat(wasmPath);
    return {
        date: new Date().toISOString().slice(0, 10),
        commit: capture('git rev-parse HEAD'),
        os: `${os.type()} ${os.release()} ${os.arch()}`,
        cpu: cpuModel(),
        logicalCpus: os.cpus().length,
        memoryBytes: os.totalmem(),
        chrome: chromeVersion,
        chromePath,
        rustc: capture('rustc --version'),
        wasmPack: capture('wasm-pack --version'),
        wasmBytes: wasmStat.size,
        smoke: options.smoke,
        canvas: environment?.canvas || { width: 1280, height: 720 },
        userAgent: environment?.userAgent || '',
        adapter: environment?.adapter || {},
        warmup: environment?.warmup ?? null,
        frames: environment?.frames ?? null,
        tight: options.tight,
        readback: options.readback,
        method: {
            scene: 'load_stress_scene filled rectangles, 400 columns, spacing 24px, size 16x12',
            frameMs: 'interval between render() returns on animation frames; the first interval is dropped',
            submitMs: 'performance.now() around render() only',
            sustained60: 'average FPS >= 55 and P95 frame interval <= 20ms',
            coldFrame: 'first render() after load_stress_scene, includes vertex-cache build and full GPU upload',
            culling: 'the draw path submits the whole vertex cache; this run does not disable culling or dirty flags',
            dirtyFlags: 'a steady sample and a pan(24, 16) must both report geometryUnchanged',
            gpuTime: 'not measured. render() returns after queue submit and present, without a GPU timestamp query. On this headless SwiftShader host, present did not add raster time to render().',
        },
        scales,
        failures,
    };
}

/**
 * 退出码。100k 失败只记在结果里，不让整次 1k/10k/100k 失败。
 * 只请求 100k 时，这一档本身必须成功。
 *
 * @param {{ scales: { elements: number }[] }} report
 * @param {number[]} requested
 * @returns {number}
 */
export function exitCode(report, requested) {
    const okCounts = new Set(report.scales.map((scale) => scale.elements));
    const required = requested.filter((count) => count !== 100000);
    const mustPass = required.length > 0 ? required : requested;
    return mustPass.every((count) => okCounts.has(count)) ? 0 : 1;
}

/**
 * 直接执行 `node scripts/bench.mjs` 时跑压测。被测试 import 时不启动 Chrome。
 *
 * @returns {boolean}
 */
function isDirectRun() {
    const entry = process.argv[1];
    if (!entry) {
        return false;
    }
    return pathToFileURL(path.resolve(entry)).href === import.meta.url;
}

/**
 * @returns {Promise<number>}
 */
async function main() {
    const options = parseArgs(process.argv.slice(2));
    if (!existsSync(wasmPath)) {
        console.error(`缺少 ${wasmPath}`);
        console.error('先运行: cd crates/wasm && wasm-pack build --target web --out-dir ../../packages/core/wasm');
        return 1;
    }

    const counts = options.counts
        ? options.counts.split(',').map((part) => Number(part.trim()))
        : options.smoke
          ? [1000]
          : [1000, 10000, 100000];

    const report = await runScales({ ...options, counts });
    const json = JSON.stringify(report, null, 2);
    console.log(json);
    if (options.out) {
        const destination = path.resolve(repoRoot, options.out);
        await mkdir(path.dirname(destination), { recursive: true });
        await writeFile(destination, `${json}\n`);
    }
    if (options.markdown) {
        const destination = path.resolve(repoRoot, options.markdown);
        await mkdir(path.dirname(destination), { recursive: true });
        await writeFile(destination, toMarkdown(report));
    }
    return exitCode(report, counts);
}

if (isDirectRun()) {
    main()
        .then((code) => {
            process.exit(code);
        })
        .catch((error) => {
            console.error(error);
            process.exit(1);
        });
}

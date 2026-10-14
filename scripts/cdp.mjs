/**
 * 最小 Chrome DevTools Protocol 客户端。
 *
 * 压测脚本用它驱动本机 Chrome，不额外依赖 Puppeteer。
 * 只覆盖导航、执行表达式、读取控制台这三件事。
 *
 * Node 22 起才有全局 `WebSocket`。CI 和本地文档用的是 Node 20，
 * 直接 `new WebSocket(url)` 会抛 `ReferenceError: WebSocket is not defined`。
 * 没有全局对象时改用 undici 6 的客户端。两边都有 `addEventListener`、`send`、`close`，
 * 文本帧的 `message` 事件把载荷放在 `data` 上。
 */
import { WebSocket as UndiciWebSocket } from 'undici';

/**
 * 选出 CDP 用的 WebSocket 构造器。
 *
 * @param {{ WebSocket?: typeof globalThis.WebSocket }} [scope] 默认 `globalThis`。测试可传入没有 `WebSocket` 的对象，模拟 Node 20。
 * @returns {typeof globalThis.WebSocket}
 *
 * @example
 * ```js
 * const WS = resolveCdpWebSocket();
 * const ws = new WS('ws://127.0.0.1:9222/devtools/page/1');
 * ```
 */
export function resolveCdpWebSocket(scope = globalThis) {
    return scope.WebSocket ?? UndiciWebSocket;
}

/**
 * 当前进程实际使用的构造器。Node 22+ 是全局 `WebSocket`，Node 20 是 undici。
 *
 * @type {typeof globalThis.WebSocket}
 */
export const cdpWebSocket = resolveCdpWebSocket();

/**
 * @param {string} url
 * @param {typeof globalThis.WebSocket} [WebSocketImpl] 默认 {@link cdpWebSocket}。测试注入 undici，用来在 Node 22 上覆盖 Node 20 的路径。
 * @returns {Promise<WebSocket>}
 */
export function openWebSocket(url, WebSocketImpl = cdpWebSocket) {
    return new Promise((resolve, reject) => {
        const ws = new WebSocketImpl(url);
        const timer = setTimeout(() => {
            ws.close();
            reject(new Error(`WebSocket 连接超时: ${url}`));
        }, 15000);
        ws.addEventListener('open', () => {
            clearTimeout(timer);
            resolve(ws);
        });
        ws.addEventListener('error', () => {
            clearTimeout(timer);
            reject(new Error(`WebSocket 连接失败: ${url}`));
        });
    });
}

/**
 * 对一个页面目标发送 CDP 命令。
 */
export class CdpSession {
    /**
     * @param {WebSocket} ws
     */
    constructor(ws) {
        this.ws = ws;
        this.nextId = 1;
        /** @type {Map<number, {resolve: Function, reject: Function}>} */
        this.pending = new Map();
        /** @type {string[]} */
        this.consoleMessages = [];
        /** @type {string[]} */
        this.exceptions = [];
        ws.addEventListener('message', (event) => {
            const message = JSON.parse(event.data);
            if (message.id && this.pending.has(message.id)) {
                const waiter = this.pending.get(message.id);
                this.pending.delete(message.id);
                if (message.error) {
                    waiter.reject(new Error(`${message.error.message || 'CDP error'}`));
                } else {
                    waiter.resolve(message.result);
                }
                return;
            }
            this.captureEvent(message);
        });
    }

    /**
     * @param {any} message
     */
    captureEvent(message) {
        if (message.method === 'Runtime.consoleAPICalled') {
            const args = (message.params.args || [])
                .map((arg) => arg.value ?? arg.description ?? arg.type)
                .join(' ');
            this.consoleMessages.push(args);
        }
        if (message.method === 'Runtime.exceptionThrown') {
            const details = message.params.exceptionDetails;
            const text = details.exception?.description || details.text || 'exception';
            this.exceptions.push(text);
        }
    }

    /**
     * @param {string} method
     * @param {Record<string, unknown>} [params]
     * @returns {Promise<any>}
     */
    send(method, params = {}) {
        const id = this.nextId++;
        return new Promise((resolve, reject) => {
            this.pending.set(id, { resolve, reject });
            this.ws.send(JSON.stringify({ id, method, params }));
        });
    }

    close() {
        this.ws.close();
    }
}

/**
 * 等待调试端口开始响应。
 *
 * @param {number} port
 * @param {number} timeoutMs
 * @returns {Promise<void>}
 */
export async function waitForDebugger(port, timeoutMs = 20000) {
    const started = Date.now();
    let lastError = '调试端口未响应';
    while (Date.now() - started < timeoutMs) {
        try {
            const response = await fetch(`http://127.0.0.1:${port}/json/version`);
            if (response.ok) {
                return;
            }
            lastError = `HTTP ${response.status}`;
        } catch (error) {
            lastError = error instanceof Error ? error.message : String(error);
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error(`Chrome 调试端口 ${port} 未就绪: ${lastError}`);
}

/**
 * 连接到第一个 page 目标。
 *
 * @param {number} port
 * @returns {Promise<CdpSession>}
 */
export async function connectPage(port) {
    const started = Date.now();
    while (Date.now() - started < 20000) {
        const response = await fetch(`http://127.0.0.1:${port}/json/list`);
        const targets = await response.json();
        const page = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
        if (page) {
            const ws = await openWebSocket(page.webSocketDebuggerUrl);
            const session = new CdpSession(ws);
            await session.send('Runtime.enable');
            await session.send('Page.enable');
            return session;
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error('没有找到 Chrome page 目标');
}

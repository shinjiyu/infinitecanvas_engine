/**
 * 最小 Chrome DevTools Protocol 客户端。
 *
 * 压测脚本用它驱动本机 Chrome，不额外依赖 Puppeteer。
 * 只覆盖导航、执行表达式、读取控制台这三件事。
 */

/**
 * @param {string} url
 * @returns {Promise<WebSocket>}
 */
export function openWebSocket(url) {
    return new Promise((resolve, reject) => {
        const ws = new WebSocket(url);
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

import assert from 'node:assert/strict';
import test from 'node:test';
import { WebSocket as UndiciWebSocket } from 'undici';
import { WebSocketServer } from 'ws';
import { CdpSession, openWebSocket, resolveCdpWebSocket } from './cdp.mjs';

test('Node 20 without a global WebSocket uses undici', () => {
    assert.equal(typeof UndiciWebSocket, 'function');
    assert.equal(resolveCdpWebSocket({}), UndiciWebSocket);
});

test('Node 22+ keeps the built-in WebSocket', () => {
    function GlobalSocket() {}
    assert.equal(resolveCdpWebSocket({ WebSocket: GlobalSocket }), GlobalSocket);
});

test('this runtime can construct the CDP WebSocket', () => {
    const WS = resolveCdpWebSocket();
    assert.equal(typeof WS, 'function');
});

/**
 * 用指定构造器走一遍 CdpSession：addEventListener 收到的 data 必须能 JSON.parse。
 *
 * @param {typeof WebSocket} WebSocketImpl
 */
async function roundTrip(WebSocketImpl) {
    const server = new WebSocketServer({ port: 0, host: '127.0.0.1' });
    await new Promise((resolve) => server.once('listening', resolve));
    const address = server.address();
    const port = typeof address === 'object' && address ? address.port : 0;
    /** @type {string} */
    let received = '';
    server.on('connection', (socket) => {
        socket.on('message', (data) => {
            received = data.toString();
            const request = JSON.parse(received);
            socket.send(JSON.stringify({ id: request.id, result: { value: 1 } }));
        });
    });
    try {
        const ws = await openWebSocket(`ws://127.0.0.1:${port}`, WebSocketImpl);
        const session = new CdpSession(ws);
        const result = await session.send('Runtime.evaluate', { expression: '1' });
        assert.deepEqual(result, { value: 1 });
        assert.match(received, /Runtime\.evaluate/);
        session.close();
    } finally {
        await new Promise((resolve, reject) => {
            server.close((error) => (error ? reject(error) : resolve(undefined)));
        });
    }
}

test('undici WebSocket matches the CdpSession client API', async () => {
    await roundTrip(UndiciWebSocket);
});

test('the resolved WebSocket matches the CdpSession client API', async () => {
    await roundTrip(resolveCdpWebSocket());
});

test('a closed debugger port rejects instead of hanging', async () => {
    await assert.rejects(
        () => openWebSocket('ws://127.0.0.1:1', UndiciWebSocket),
        /WebSocket 连接失败/,
    );
});

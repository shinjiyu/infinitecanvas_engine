/**
 * InfiniteCanvas 协同服务器
 *
 * M4.1: 服务端基础
 * - WebSocket 服务器
 * - 房间管理
 * - 用户会话管理
 */

import { WebSocketServer, WebSocket } from 'ws';
import { v4 as uuidv4 } from 'uuid';
import { RoomManager } from './room.js';
import { UserSession } from './user.js';
import type { ClientMessage } from './types.js';

const PORT = parseInt(process.env.PORT || '8080', 10);

// ============ 服务器实例 ============

const wss = new WebSocketServer({ port: PORT });
const roomManager = new RoomManager();
const sessions = new Map<WebSocket, UserSession>();

// ============ 连接处理 ============

wss.on('connection', (ws: WebSocket) => {
  console.log('[Server] New connection');

  ws.on('message', (data: Buffer) => {
    try {
      const message = JSON.parse(data.toString()) as ClientMessage;
      handleMessage(ws, message);
    } catch (error) {
      console.error('[Server] Failed to parse message:', error);
    }
  });

  ws.on('close', () => {
    handleDisconnect(ws);
  });

  ws.on('error', (error) => {
    console.error('[Server] WebSocket error:', error);
    handleDisconnect(ws);
  });
});

// ============ 消息处理 ============

function handleMessage(ws: WebSocket, message: ClientMessage): void {
  switch (message.type) {
    case 'join':
      handleJoin(ws, message.roomId, message.userName);
      break;

    case 'cursor_move':
      handleCursorMove(ws, message.position);
      break;

    case 'selection_change':
      handleSelectionChange(ws, message.selection);
      break;

    case 'operation':
      handleOperation(ws, message.operationId, message.operation);
      break;

    default:
      console.warn('[Server] Unknown message type:', (message as { type: string }).type);
  }
}

/**
 * 处理加入房间
 */
function handleJoin(ws: WebSocket, roomId: string, userName: string): void {
  // 如果已经在会话中，先离开
  const existingSession = sessions.get(ws);
  if (existingSession && existingSession.roomId) {
    const room = roomManager.getRoom(existingSession.roomId);
    if (room) {
      room.leave(existingSession.id);
      roomManager.cleanupEmptyRoom(existingSession.roomId);
    }
  }

  // 创建新会话
  const userId = uuidv4();
  const session = new UserSession(userId, userName, ws);
  sessions.set(ws, session);

  // 加入房间
  const room = roomManager.getOrCreateRoom(roomId);
  room.join(session);

  console.log(`[Server] User ${userName} (${userId}) joined room ${roomId}`);
}

/**
 * 处理光标移动
 */
function handleCursorMove(ws: WebSocket, position: { x: number; y: number }): void {
  const session = sessions.get(ws);
  if (!session || !session.roomId) return;

  session.updateCursor(position);

  const room = roomManager.getRoom(session.roomId);
  if (room) {
    room.broadcastCursor(session.id, position);
  }
}

/**
 * 处理选择变更
 */
function handleSelectionChange(ws: WebSocket, selection: string[]): void {
  const session = sessions.get(ws);
  if (!session || !session.roomId) return;

  session.updateSelection(selection);

  const room = roomManager.getRoom(session.roomId);
  if (room) {
    room.broadcastSelection(session.id, selection);
  }
}

/**
 * 处理画布操作
 */
function handleOperation(
  ws: WebSocket,
  operationId: string,
  operation: ClientMessage extends { type: 'operation' } ? ClientMessage['operation'] : never
): void {
  const session = sessions.get(ws);
  if (!session || !session.roomId) return;

  const room = roomManager.getRoom(session.roomId);
  if (room) {
    // 广播操作给其他用户
    room.broadcastOperation(session.id, operationId, operation);

    // 发送确认给发送者
    session.send({
      type: 'operation_ack',
      operationId,
      success: true,
    });
  }
}

/**
 * 处理断开连接
 */
function handleDisconnect(ws: WebSocket): void {
  const session = sessions.get(ws);
  if (!session) return;

  console.log(`[Server] User ${session.name} (${session.id}) disconnected`);

  // 从房间中移除
  if (session.roomId) {
    const room = roomManager.getRoom(session.roomId);
    if (room) {
      room.leave(session.id);
      roomManager.cleanupEmptyRoom(session.roomId);
    }
  }

  // 清理会话
  sessions.delete(ws);
}

// ============ 服务器启动 ============

console.log(`
╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║   🎨 InfiniteCanvas 协同服务器                             ║
║                                                           ║
║   端口: ${PORT}                                            ║
║   WebSocket: ws://localhost:${PORT}                        ║
║                                                           ║
║   准备接收连接...                                          ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝
`);

// 定期打印状态
setInterval(() => {
  const stats = roomManager.getRoomStats();
  if (stats.length > 0) {
    console.log('[Server] Room stats:', stats);
  }
}, 60000); // 每分钟

// 优雅关闭
process.on('SIGINT', () => {
  console.log('\n[Server] Shutting down...');
  wss.close(() => {
    console.log('[Server] Closed');
    process.exit(0);
  });
});



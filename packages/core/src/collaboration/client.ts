/**
 * 协同客户端连接管理器
 *
 * M4.2: 客户端 WebSocket
 * - 连接管理
 * - 自动重连
 * - 消息收发
 */

import type {
  CollaborationEvents,
  ConnectionState,
  ClientMessage,
  ServerMessage,
  RemoteUser,
  Vec2,
  CanvasOperation,
} from './types';
import {
  remotePresencesFromUsers,
  type RemotePresence,
} from './presence';

export interface CollaborationClientOptions {
  /** 服务器地址，如 ws://localhost:8080 */
  serverUrl: string;
  /** 自动重连 */
  autoReconnect?: boolean;
  /** 重连延迟（毫秒） */
  reconnectDelay?: number;
  /** 最大重连次数 */
  maxReconnectAttempts?: number;
  /** 光标同步节流时间（毫秒） */
  cursorThrottleMs?: number;
}

const DEFAULT_OPTIONS: Required<Omit<CollaborationClientOptions, 'serverUrl'>> = {
  autoReconnect: true,
  reconnectDelay: 1000,
  maxReconnectAttempts: 10,
  cursorThrottleMs: 50,
};

/**
 * 协同客户端
 */
export class CollaborationClient {
  private options: Required<CollaborationClientOptions>;
  private ws: WebSocket | null = null;
  private events: CollaborationEvents = {};
  private state: ConnectionState = 'disconnected';
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  // 房间状态
  private roomId: string | null = null;
  private myUserId: string | null = null;
  private remoteUsers: Map<string, RemoteUser> = new Map();

  // 光标节流
  private lastCursorSend = 0;
  private pendingCursor: Vec2 | null = null;
  private cursorTimer: ReturnType<typeof setTimeout> | null = null;

  // 操作回调
  private operationCallbacks: Map<string, (success: boolean) => void> = new Map();
  private operationCounter = 0;

  constructor(options: CollaborationClientOptions) {
    this.options = { ...DEFAULT_OPTIONS, ...options };
  }

  // ============ 公共 API ============

  /**
   * 设置事件回调
   */
  on<K extends keyof CollaborationEvents>(
    event: K,
    callback: CollaborationEvents[K]
  ): this {
    this.events[event] = callback;
    return this;
  }

  /**
   * 获取当前连接状态
   */
  getState(): ConnectionState {
    return this.state;
  }

  /**
   * 获取当前房间 ID
   */
  getRoomId(): string | null {
    return this.roomId;
  }

  /**
   * 获取我的用户 ID
   */
  getMyUserId(): string | null {
    return this.myUserId;
  }

  /**
   * 获取远程用户列表
   */
  getRemoteUsers(): RemoteUser[] {
    return Array.from(this.remoteUsers.values());
  }

  /**
   * 远端在场快照（awareness 形状），已排除本地用户。
   *
   * 与 yrs `CollabSession::remote_presences` 同形，供光标/选区 overlay 使用。
   * 不读写场景 JSON。
   */
  getRemotePresences(): RemotePresence[] {
    return remotePresencesFromUsers(this.getRemoteUsers(), this.myUserId);
  }

  /**
   * 连接到服务器并加入房间
   */
  connect(roomId: string, userName: string): void {
    this.roomId = roomId;
    this.doConnect(roomId, userName);
  }

  /**
   * 断开连接
   */
  disconnect(): void {
    this.options.autoReconnect = false;
    this.cleanup();
    this.setState('disconnected');
  }

  /**
   * 发送光标位置（节流）
   */
  sendCursorMove(position: Vec2): void {
    const now = Date.now();
    const elapsed = now - this.lastCursorSend;

    if (elapsed >= this.options.cursorThrottleMs) {
      this.doSendCursor(position);
    } else {
      // 节流：延迟发送
      this.pendingCursor = position;
      if (!this.cursorTimer) {
        this.cursorTimer = setTimeout(() => {
          if (this.pendingCursor) {
            this.doSendCursor(this.pendingCursor);
            this.pendingCursor = null;
          }
          this.cursorTimer = null;
        }, this.options.cursorThrottleMs - elapsed);
      }
    }
  }

  /**
   * 发送选择变更
   */
  sendSelectionChange(selection: string[]): void {
    this.send({
      type: 'selection_change',
      selection,
    });
  }

  /**
   * 发送画布操作
   */
  sendOperation(operation: CanvasOperation): Promise<boolean> {
    return new Promise((resolve) => {
      const operationId = `op_${++this.operationCounter}_${Date.now()}`;

      this.operationCallbacks.set(operationId, (success) => {
        this.operationCallbacks.delete(operationId);
        resolve(success);
      });

      this.send({
        type: 'operation',
        operationId,
        operation,
      });

      // 超时处理
      setTimeout(() => {
        if (this.operationCallbacks.has(operationId)) {
          this.operationCallbacks.delete(operationId);
          resolve(false);
        }
      }, 5000);
    });
  }

  // ============ 内部方法 ============

  private doConnect(roomId: string, userName: string): void {
    this.setState('connecting');
    this.cleanup();

    try {
      this.ws = new WebSocket(this.options.serverUrl);

      this.ws.onopen = () => {
        console.log('[Collab] Connected to server');
        this.reconnectAttempts = 0;
        this.setState('connected');

        // 发送加入房间消息
        this.send({
          type: 'join',
          roomId,
          userName,
        });
      };

      this.ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data as string) as ServerMessage;
          this.handleMessage(message);
        } catch (error) {
          console.error('[Collab] Failed to parse message:', error);
        }
      };

      this.ws.onclose = () => {
        console.log('[Collab] Connection closed');
        this.handleDisconnect(roomId, userName);
      };

      this.ws.onerror = (error) => {
        console.error('[Collab] WebSocket error:', error);
        this.events.onError?.(new Error('WebSocket connection error'));
      };
    } catch (error) {
      console.error('[Collab] Failed to connect:', error);
      this.handleDisconnect(roomId, userName);
    }
  }

  private handleDisconnect(roomId: string, userName: string): void {
    this.ws = null;
    this.remoteUsers.clear();

    if (this.options.autoReconnect && this.reconnectAttempts < this.options.maxReconnectAttempts) {
      this.setState('reconnecting');
      this.reconnectAttempts++;

      const delay = this.options.reconnectDelay * Math.pow(1.5, this.reconnectAttempts - 1);
      console.log(`[Collab] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

      this.reconnectTimer = setTimeout(() => {
        this.doConnect(roomId, userName);
      }, delay);
    } else {
      this.setState('disconnected');
    }
  }

  private handleMessage(message: ServerMessage): void {
    switch (message.type) {
      case 'room_state':
        this.myUserId = message.yourUserId;
        this.remoteUsers.clear();
        for (const user of message.users) {
          if (user.id !== this.myUserId) {
            this.remoteUsers.set(user.id, user);
          }
        }
        this.events.onRoomJoined?.(message.roomId, message.users, message.yourUserId);
        break;

      case 'user_joined':
        this.remoteUsers.set(message.user.id, message.user);
        this.events.onUserJoined?.(message.user);
        break;

      case 'user_left':
        this.remoteUsers.delete(message.userId);
        this.events.onUserLeft?.(message.userId);
        break;

      case 'cursor_update': {
        const user = this.remoteUsers.get(message.userId);
        if (user) {
          user.cursor = message.position;
        }
        this.events.onCursorUpdate?.(message.userId, message.position);
        break;
      }

      case 'selection_update': {
        const user = this.remoteUsers.get(message.userId);
        if (user) {
          user.selection = message.selection;
        }
        this.events.onSelectionUpdate?.(message.userId, message.selection);
        break;
      }

      case 'operation':
        this.events.onRemoteOperation?.(message.operation);
        break;

      case 'operation_ack': {
        const callback = this.operationCallbacks.get(message.operationId);
        if (callback) {
          callback(message.success);
        }
        this.events.onOperationAck?.(message.operationId, message.success);
        break;
      }
    }
  }

  private send(message: ClientMessage): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }

  private doSendCursor(position: Vec2): void {
    this.lastCursorSend = Date.now();
    this.send({
      type: 'cursor_move',
      position,
    });
  }

  private setState(state: ConnectionState): void {
    if (this.state !== state) {
      this.state = state;
      this.events.onConnectionStateChange?.(state);
    }
  }

  private cleanup(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.cursorTimer) {
      clearTimeout(this.cursorTimer);
      this.cursorTimer = null;
    }
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      if (this.ws.readyState === WebSocket.OPEN) {
        this.ws.close();
      }
      this.ws = null;
    }
  }
}



/**
 * 房间管理
 */

import type { UserSession } from './user.js';
import type {
  CanvasOperation,
  CursorUpdateMessage,
  OperationMessage,
  RoomStateMessage,
  SelectionUpdateMessage,
  ServerMessage,
  UserJoinedMessage,
  UserLeftMessage,
} from './types.js';

/**
 * 协同房间
 */
export class Room {
  public readonly id: string;
  public readonly createdAt: Date;
  private users: Map<string, UserSession> = new Map();
  private operationHistory: CanvasOperation[] = [];

  constructor(id: string) {
    this.id = id;
    this.createdAt = new Date();
  }

  /**
   * 获取房间内用户数量
   */
  get userCount(): number {
    return this.users.size;
  }

  /**
   * 用户加入房间
   */
  join(user: UserSession): void {
    // 通知现有用户
    const joinedMessage: UserJoinedMessage = {
      type: 'user_joined',
      user: user.toUser(),
    };
    this.broadcast(joinedMessage, user.id);

    // 添加用户到房间
    this.users.set(user.id, user);
    user.roomId = this.id;

    // 发送房间状态给新用户
    const roomState: RoomStateMessage = {
      type: 'room_state',
      roomId: this.id,
      users: Array.from(this.users.values()).map((u) => u.toUser()),
      yourUserId: user.id,
    };
    user.send(roomState);

    console.log(`[Room ${this.id}] User ${user.name} (${user.id}) joined. Total: ${this.userCount}`);
  }

  /**
   * 用户离开房间
   */
  leave(userId: string): UserSession | null {
    const user = this.users.get(userId);
    if (!user) return null;

    this.users.delete(userId);
    user.roomId = null;

    // 通知其他用户
    const leftMessage: UserLeftMessage = {
      type: 'user_left',
      userId,
    };
    this.broadcast(leftMessage);

    console.log(`[Room ${this.id}] User ${user.name} (${userId}) left. Total: ${this.userCount}`);
    return user;
  }

  /**
   * 广播光标更新
   */
  broadcastCursor(userId: string, position: { x: number; y: number }): void {
    const message: CursorUpdateMessage = {
      type: 'cursor_update',
      userId,
      position,
    };
    this.broadcast(message, userId);
  }

  /**
   * 广播选择更新
   */
  broadcastSelection(userId: string, selection: string[]): void {
    const message: SelectionUpdateMessage = {
      type: 'selection_update',
      userId,
      selection,
    };
    this.broadcast(message, userId);
  }

  /**
   * 广播操作
   */
  broadcastOperation(userId: string, operationId: string, operation: CanvasOperation): void {
    // 保存到历史
    this.operationHistory.push(operation);

    // 广播给其他用户
    const message: OperationMessage = {
      type: 'operation',
      operationId,
      operation,
    };
    this.broadcast(message, userId);
  }

  /**
   * 广播消息给房间内所有用户
   */
  broadcast(message: ServerMessage, excludeUserId?: string): void {
    for (const [userId, user] of this.users) {
      if (userId !== excludeUserId) {
        user.send(message);
      }
    }
  }

  /**
   * 获取用户
   */
  getUser(userId: string): UserSession | undefined {
    return this.users.get(userId);
  }

  /**
   * 检查房间是否为空
   */
  isEmpty(): boolean {
    return this.users.size === 0;
  }
}

/**
 * 房间管理器
 */
export class RoomManager {
  private rooms: Map<string, Room> = new Map();

  /**
   * 获取或创建房间
   */
  getOrCreateRoom(roomId: string): Room {
    let room = this.rooms.get(roomId);
    if (!room) {
      room = new Room(roomId);
      this.rooms.set(roomId, room);
      console.log(`[RoomManager] Created room: ${roomId}`);
    }
    return room;
  }

  /**
   * 获取房间
   */
  getRoom(roomId: string): Room | undefined {
    return this.rooms.get(roomId);
  }

  /**
   * 删除空房间
   */
  cleanupEmptyRoom(roomId: string): boolean {
    const room = this.rooms.get(roomId);
    if (room && room.isEmpty()) {
      this.rooms.delete(roomId);
      console.log(`[RoomManager] Cleaned up empty room: ${roomId}`);
      return true;
    }
    return false;
  }

  /**
   * 获取所有房间信息
   */
  getRoomStats(): { roomId: string; userCount: number; createdAt: Date }[] {
    return Array.from(this.rooms.values()).map((room) => ({
      roomId: room.id,
      userCount: room.userCount,
      createdAt: room.createdAt,
    }));
  }
}


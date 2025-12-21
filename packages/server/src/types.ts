/**
 * 消息类型定义
 */

// ============ 基础类型 ============

export interface Vec2 {
  x: number;
  y: number;
}

export interface Color {
  r: number;
  g: number;
  b: number;
  a: number;
}

// ============ 用户相关 ============

export interface User {
  id: string;
  name: string;
  color: string; // 用户标识颜色
  cursor?: Vec2;
  selection: string[]; // 选中的元素 ID 列表
}

// ============ 消息类型 ============

export type MessageType =
  // 连接管理
  | 'join'
  | 'leave'
  | 'user_joined'
  | 'user_left'
  | 'room_state'
  // 光标同步
  | 'cursor_move'
  | 'cursor_update'
  // 选择同步
  | 'selection_change'
  | 'selection_update'
  // 操作同步
  | 'operation'
  | 'operation_ack';

// ============ 客户端 -> 服务端消息 ============

export interface JoinMessage {
  type: 'join';
  roomId: string;
  userName: string;
}

export interface CursorMoveMessage {
  type: 'cursor_move';
  position: Vec2;
}

export interface SelectionChangeMessage {
  type: 'selection_change';
  selection: string[];
}

export interface OperationMessage {
  type: 'operation';
  operationId: string;
  operation: CanvasOperation;
}

// ============ 服务端 -> 客户端消息 ============

export interface RoomStateMessage {
  type: 'room_state';
  roomId: string;
  users: User[];
  yourUserId: string;
}

export interface UserJoinedMessage {
  type: 'user_joined';
  user: User;
}

export interface UserLeftMessage {
  type: 'user_left';
  userId: string;
}

export interface CursorUpdateMessage {
  type: 'cursor_update';
  userId: string;
  position: Vec2;
}

export interface SelectionUpdateMessage {
  type: 'selection_update';
  userId: string;
  selection: string[];
}

export interface OperationAckMessage {
  type: 'operation_ack';
  operationId: string;
  success: boolean;
}

// ============ 画布操作类型 ============

export type CanvasOperation =
  | AddElementOperation
  | RemoveElementOperation
  | MoveElementOperation
  | UpdateElementOperation;

export interface AddElementOperation {
  op: 'add';
  elementType: 'rectangle' | 'circle' | 'image';
  elementId: string;
  properties: Record<string, unknown>;
}

export interface RemoveElementOperation {
  op: 'remove';
  elementId: string;
}

export interface MoveElementOperation {
  op: 'move';
  elementId: string;
  position: Vec2;
}

export interface UpdateElementOperation {
  op: 'update';
  elementId: string;
  properties: Record<string, unknown>;
}

// ============ 联合消息类型 ============

export type ClientMessage =
  | JoinMessage
  | CursorMoveMessage
  | SelectionChangeMessage
  | OperationMessage;

export type ServerMessage =
  | RoomStateMessage
  | UserJoinedMessage
  | UserLeftMessage
  | CursorUpdateMessage
  | SelectionUpdateMessage
  | OperationAckMessage
  | OperationMessage; // 广播操作给其他用户



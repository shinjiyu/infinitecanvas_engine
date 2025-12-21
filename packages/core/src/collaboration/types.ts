/**
 * 协同消息类型定义（客户端）
 */

// ============ 基础类型 ============

export interface Vec2 {
  x: number;
  y: number;
}

// ============ 用户相关 ============

export interface RemoteUser {
  id: string;
  name: string;
  color: string;
  cursor?: Vec2;
  selection: string[];
}

// ============ 连接状态 ============

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

// ============ 消息类型 ============

// 客户端 -> 服务端
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

// 服务端 -> 客户端
export interface RoomStateMessage {
  type: 'room_state';
  roomId: string;
  users: RemoteUser[];
  yourUserId: string;
}

export interface UserJoinedMessage {
  type: 'user_joined';
  user: RemoteUser;
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
  | OperationMessage;

// ============ 事件回调 ============

export interface CollaborationEvents {
  onConnectionStateChange?: (state: ConnectionState) => void;
  onRoomJoined?: (roomId: string, users: RemoteUser[], myUserId: string) => void;
  onUserJoined?: (user: RemoteUser) => void;
  onUserLeft?: (userId: string) => void;
  onCursorUpdate?: (userId: string, position: Vec2) => void;
  onSelectionUpdate?: (userId: string, selection: string[]) => void;
  onRemoteOperation?: (operation: CanvasOperation) => void;
  onOperationAck?: (operationId: string, success: boolean) => void;
  onError?: (error: Error) => void;
}


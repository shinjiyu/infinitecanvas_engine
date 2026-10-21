/**
 * Yjs awareness 在场状态（与 Rust `PresenceState` 同形）。
 *
 * 光标用世界坐标；选区是元素 id 的十进制字符串列表。
 * 不进场景 JSON。现有 WebSocket `RemoteUser` 可通过
 * {@link presenceFromRemoteUser} 转成同一形状，软协调两条路径。
 *
 * 权限 / 冲突 UI 不在本模块范围。
 */

import type { RemoteUser, Vec2 } from './types';

/** 世界坐标光标 */
export interface PresenceCursor {
  x: number;
  y: number;
}

/**
 * 一个对等端的在场快照。
 *
 * 字段名与 `crates/collab` 的 `PresenceState` JSON 对齐。
 */
export interface PresenceState {
  name: string;
  color: string;
  cursor?: PresenceCursor;
  /** 选中元素 id（十进制字符串，与场景 id 一致） */
  selection: string[];
}

/** 远端在场：稳定 client 键 + 状态 */
export interface RemotePresence {
  /** WebSocket userId 或 yrs client id 的字符串 */
  clientId: string;
  state: PresenceState;
}

/**
 * 从现有 WebSocket `RemoteUser` 构造 awareness 形状的在场状态。
 *
 * 坐标语义：房间服务目前广播屏幕像素；接入 yrs awareness 后应改为世界坐标。
 * 本转换保持数值不变，只统一字段名，方便 overlay 共用。
 */
export function presenceFromRemoteUser(user: RemoteUser): PresenceState {
  return {
    name: user.name,
    color: user.color,
    cursor: user.cursor ? { x: user.cursor.x, y: user.cursor.y } : undefined,
    selection: (user.selection ?? []).map(String),
  };
}

/**
 * 把远程用户列表转成远端在场列表，并去掉本地用户（不画自己的远端副本）。
 */
export function remotePresencesFromUsers(
  users: RemoteUser[],
  localUserId: string | null
): RemotePresence[] {
  return users
    .filter((u) => u.id !== localUserId)
    .map((u) => ({
      clientId: u.id,
      state: presenceFromRemoteUser(u),
    }));
}

/**
 * 构造要广播的本地在场状态。
 */
export function makeLocalPresence(
  name: string,
  color: string,
  cursor: Vec2 | null | undefined,
  selection: Array<string | number>
): PresenceState {
  return {
    name,
    color,
    cursor: cursor ? { x: cursor.x, y: cursor.y } : undefined,
    selection: selection.map(String),
  };
}

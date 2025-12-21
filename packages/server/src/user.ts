/**
 * 用户会话管理
 */

import type { WebSocket } from 'ws';
import type { User, Vec2 } from './types.js';

// 预定义的用户颜色
const USER_COLORS = [
  '#4090f2', // 蓝色
  '#36d399', // 绿色
  '#f24040', // 红色
  '#f2c94c', // 黄色
  '#9b59b6', // 紫色
  '#e91e63', // 粉色
  '#00bcd4', // 青色
  '#ff9800', // 橙色
];

let colorIndex = 0;

function getNextColor(): string {
  const color = USER_COLORS[colorIndex % USER_COLORS.length];
  colorIndex++;
  return color;
}

/**
 * 用户会话
 */
export class UserSession {
  public readonly id: string;
  public readonly name: string;
  public readonly color: string;
  public readonly socket: WebSocket;
  public roomId: string | null = null;
  public cursor: Vec2 | null = null;
  public selection: string[] = [];

  constructor(id: string, name: string, socket: WebSocket) {
    this.id = id;
    this.name = name;
    this.color = getNextColor();
    this.socket = socket;
  }

  /**
   * 转换为用户信息对象
   */
  toUser(): User {
    return {
      id: this.id,
      name: this.name,
      color: this.color,
      cursor: this.cursor ?? undefined,
      selection: this.selection,
    };
  }

  /**
   * 发送消息
   */
  send(message: object): void {
    if (this.socket.readyState === this.socket.OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }

  /**
   * 更新光标位置
   */
  updateCursor(position: Vec2): void {
    this.cursor = position;
  }

  /**
   * 更新选择状态
   */
  updateSelection(selection: string[]): void {
    this.selection = selection;
  }
}



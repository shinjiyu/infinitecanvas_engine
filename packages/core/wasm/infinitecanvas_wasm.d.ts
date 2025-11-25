/**
 * Canvas 2D Engine TypeScript 类型定义
 */

export class InfiniteCanvas {
  constructor();
  init(canvas: HTMLCanvasElement): void;
  render(): void;
  addElement(element: any): number;
  getStats(): {
    elements: number;
    zoom: number;
    frameCount: number;
  };
}

export default function init(): Promise<void>;


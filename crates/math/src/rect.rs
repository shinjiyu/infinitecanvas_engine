//! 矩形
//!
//! 用于边界框计算和碰撞检测
use glam::Vec2;
use serde::{Deserialize, Serialize};

/// 矩形（Axis-Aligned Bounding Box）
///
/// 使用左上角坐标 + 宽高表示
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// 创建矩形
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 从两个点创建矩形
    pub fn from_corners(min: Vec2, max: Vec2) -> Self {
        Self {
            x: min.x,
            y: min.y,
            width: max.x - min.x,
            height: max.y - min.y,
        }
    }

    /// 从中心点和尺寸创建
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        Self {
            x: center.x - size.x / 2.0,
            y: center.y - size.y / 2.0,
            width: size.x,
            height: size.y,
        }
    }

    /// 获取左边界
    pub fn left(&self) -> f32 {
        self.x
    }

    /// 获取右边界
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// 获取上边界
    pub fn top(&self) -> f32 {
        self.y
    }

    /// 获取下边界
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// 获取中心点
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// 获取左上角
    pub fn min(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// 获取右下角
    pub fn max(&self) -> Vec2 {
        Vec2::new(self.right(), self.bottom())
    }

    /// 获取尺寸
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    /// 获取面积
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// 判断点是否在矩形内
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.left()
            && point.x <= self.right()
            && point.y >= self.top()
            && point.y <= self.bottom()
    }

    /// 判断是否与另一个矩形相交
    pub fn intersects(&self, other: &Rect) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    /// 判断是否完全包含另一个矩形
    pub fn contains_rect(&self, other: &Rect) -> bool {
        self.left() <= other.left()
            && self.right() >= other.right()
            && self.top() <= other.top()
            && self.bottom() >= other.bottom()
    }

    /// 计算与另一个矩形的交集
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }

        let left = self.left().max(other.left());
        let right = self.right().min(other.right());
        let top = self.top().max(other.top());
        let bottom = self.bottom().min(other.bottom());

        Some(Rect::new(left, top, right - left, bottom - top))
    }

    /// 计算包含两个矩形的最小矩形
    pub fn union(&self, other: &Rect) -> Rect {
        let left = self.left().min(other.left());
        let right = self.right().max(other.right());
        let top = self.top().min(other.top());
        let bottom = self.bottom().max(other.bottom());

        Rect::new(left, top, right - left, bottom - top)
    }

    /// 扩展矩形
    pub fn inflate(&self, amount: f32) -> Rect {
        Rect::new(
            self.x - amount,
            self.y - amount,
            self.width + amount * 2.0,
            self.height + amount * 2.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(rect.contains(Vec2::new(50.0, 40.0)));
        assert!(rect.contains(Vec2::new(10.0, 20.0)));
        assert!(!rect.contains(Vec2::new(5.0, 40.0)));
    }

    #[test]
    fn test_intersects() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let c = Rect::new(200.0, 200.0, 100.0, 100.0);

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_intersection() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);

        let result = a.intersection(&b).unwrap();
        assert_eq!(result, Rect::new(50.0, 50.0, 50.0, 50.0));
    }
}

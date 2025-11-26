///! 场景容器
///!
///! 管理无限画布上的所有元素

use crate::element::{Circle, Element, ElementId, Image, Rectangle};
use std::collections::HashMap;

/// 元素枚举（支持不同类型元素）
#[derive(Debug, Clone)]
pub enum AnyElement {
    Rectangle(Rectangle),
    Circle(Circle),
    Image(Image),
}

impl AnyElement {
    /// 获取元素 ID
    pub fn id(&self) -> ElementId {
        match self {
            AnyElement::Rectangle(r) => r.props().id,
            AnyElement::Circle(c) => c.props().id,
            AnyElement::Image(i) => i.props().id,
        }
    }

    /// 作为 Element trait 对象
    pub fn as_element(&self) -> &dyn Element {
        match self {
            AnyElement::Rectangle(r) => r,
            AnyElement::Circle(c) => c,
            AnyElement::Image(i) => i,
        }
    }
}

/// 场景容器
///
/// # 职责
/// - 存储和管理所有元素
/// - 元素增删改查
/// - 元素查找和过滤
/// - Z-index 排序
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{Scene, Rectangle, Circle};
/// use infinitecanvas_engine::math::Vec2;
///
/// let mut scene = Scene::new();
/// let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0));
/// scene.add_rect(rect);
/// assert_eq!(scene.count(), 1);
/// ```
pub struct Scene {
    elements: HashMap<ElementId, AnyElement>,
    next_id: ElementId,
}

impl Scene {
    /// 创建空场景
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            next_id: 1,
        }
    }

    /// 生成新的元素 ID
    pub fn next_id(&mut self) -> ElementId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 添加矩形元素
    pub fn add_rect(&mut self, rect: Rectangle) -> ElementId {
        let id = rect.props().id;
        self.elements.insert(id, AnyElement::Rectangle(rect));
        id
    }

    /// 添加圆形元素
    pub fn add_circle(&mut self, circle: Circle) -> ElementId {
        let id = circle.props().id;
        self.elements.insert(id, AnyElement::Circle(circle));
        id
    }

    /// 添加图片元素
    pub fn add_image(&mut self, image: Image) -> ElementId {
        let id = image.props().id;
        self.elements.insert(id, AnyElement::Image(image));
        id
    }

    /// 移除元素
    pub fn remove(&mut self, id: ElementId) -> Option<AnyElement> {
        self.elements.remove(&id)
    }

    /// 获取元素
    pub fn get(&self, id: ElementId) -> Option<&AnyElement> {
        self.elements.get(&id)
    }

    /// 获取可变元素
    pub fn get_mut(&mut self, id: ElementId) -> Option<&mut AnyElement> {
        self.elements.get_mut(&id)
    }

    /// 清空场景
    pub fn clear(&mut self) {
        self.elements.clear();
    }

    /// 获取元素数量
    pub fn count(&self) -> usize {
        self.elements.len()
    }

    /// 判断场景是否为空
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// 获取所有元素（按 z-index 排序）
    pub fn elements_sorted(&self) -> Vec<&AnyElement> {
        let mut elements: Vec<&AnyElement> = self.elements.values().collect();
        elements.sort_by_key(|e| e.as_element().props().z_index);
        elements
    }

    /// 迭代所有元素
    pub fn iter(&self) -> impl Iterator<Item = &AnyElement> {
        self.elements.values()
    }

    /// 查找指定位置的元素（从上到下）
    pub fn find_at(&self, point: infinitecanvas_math::Vec2) -> Option<ElementId> {
        let mut elements = self.elements_sorted();
        elements.reverse(); // 从顶层开始查找

        for element in elements {
            if element.as_element().contains_point(point) {
                return Some(element.id());
            }
        }

        None
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitecanvas_math::Vec2;

    #[test]
    fn test_add_remove() {
        let mut scene = Scene::new();

        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0));
        scene.add_rect(rect);
        assert_eq!(scene.count(), 1);

        scene.remove(1);
        assert_eq!(scene.count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut scene = Scene::new();

        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::ONE));
        scene.add_circle(Circle::new(2, Vec2::ZERO, 10.0));
        assert_eq!(scene.count(), 2);

        scene.clear();
        assert_eq!(scene.count(), 0);
    }

    #[test]
    fn test_find_at() {
        let mut scene = Scene::new();

        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 30.0));
        scene.add_rect(rect);

        let circle = Circle::new(2, Vec2::new(200.0, 200.0), 50.0);
        scene.add_circle(circle);

        // 在矩形内
        assert_eq!(scene.find_at(Vec2::new(100.0, 100.0)), Some(1));

        // 在圆形内
        assert_eq!(scene.find_at(Vec2::new(200.0, 200.0)), Some(2));

        // 不在任何元素内
        assert_eq!(scene.find_at(Vec2::new(500.0, 500.0)), None);
    }

    #[test]
    fn test_z_index_sorting() {
        let mut scene = Scene::new();

        let mut rect1 = Rectangle::new(1, Vec2::ZERO, Vec2::ONE);
        rect1.props_mut().z_index = 10;
        scene.add_rect(rect1);

        let mut rect2 = Rectangle::new(2, Vec2::ZERO, Vec2::ONE);
        rect2.props_mut().z_index = 5;
        scene.add_rect(rect2);

        let elements = scene.elements_sorted();
        assert_eq!(elements[0].id(), 2); // z_index=5
        assert_eq!(elements[1].id(), 1); // z_index=10
    }

    #[test]
    fn test_next_id() {
        let mut scene = Scene::new();
        assert_eq!(scene.next_id(), 1);
        assert_eq!(scene.next_id(), 2);
        assert_eq!(scene.next_id(), 3);
    }
}


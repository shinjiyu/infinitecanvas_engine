//! QuadTree 空间索引
//!
//! 用于加速空间查询（命中测试、范围查询等）

use crate::ElementId;
use infinitecanvas_math::Rect;

/// QuadTree 节点容量
const MAX_OBJECTS: usize = 10;
/// QuadTree 最大深度
const MAX_DEPTH: usize = 8;

/// QuadTree 中存储的对象
#[derive(Debug, Clone)]
struct QuadObject {
    id: ElementId,
    bounds: Rect,
}

/// QuadTree 节点
#[derive(Debug)]
pub struct QuadTree {
    /// 节点边界
    bounds: Rect,
    /// 当前深度
    depth: usize,
    /// 存储的对象
    objects: Vec<QuadObject>,
    /// 子节点 [NW, NE, SW, SE]
    children: Option<Box<[QuadTree; 4]>>,
}

impl QuadTree {
    /// 创建 QuadTree
    pub fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            depth: 0,
            objects: Vec::new(),
            children: None,
        }
    }

    /// 创建指定深度的 QuadTree
    fn with_depth(bounds: Rect, depth: usize) -> Self {
        Self {
            bounds,
            depth,
            objects: Vec::new(),
            children: None,
        }
    }

    /// 插入对象
    pub fn insert(&mut self, id: ElementId, bounds: Rect) {
        // 如果边界不相交，跳过
        if !self.bounds.intersects(&bounds) {
            return;
        }

        // 如果有子节点，尝试插入到子节点
        if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                child.insert(id, bounds);
            }
            return;
        }

        // 添加到当前节点
        self.objects.push(QuadObject { id, bounds });

        // 如果超过容量且未达到最大深度，分裂
        if self.objects.len() > MAX_OBJECTS && self.depth < MAX_DEPTH {
            self.subdivide();
        }
    }

    /// 分裂节点
    fn subdivide(&mut self) {
        let x = self.bounds.x;
        let y = self.bounds.y;
        let hw = self.bounds.width / 2.0;
        let hh = self.bounds.height / 2.0;

        let nw = Rect::new(x, y, hw, hh);
        let ne = Rect::new(x + hw, y, hw, hh);
        let sw = Rect::new(x, y + hh, hw, hh);
        let se = Rect::new(x + hw, y + hh, hw, hh);

        let depth = self.depth + 1;
        self.children = Some(Box::new([
            QuadTree::with_depth(nw, depth),
            QuadTree::with_depth(ne, depth),
            QuadTree::with_depth(sw, depth),
            QuadTree::with_depth(se, depth),
        ]));

        // 将对象重新插入到子节点
        let objects = std::mem::take(&mut self.objects);
        for obj in objects {
            if let Some(ref mut children) = self.children {
                for child in children.iter_mut() {
                    child.insert(obj.id, obj.bounds);
                }
            }
        }
    }

    /// 查询与点相交的所有对象
    pub fn query_point(&self, point: infinitecanvas_math::Vec2) -> Vec<ElementId> {
        let mut results = Vec::new();
        self.query_point_internal(point, &mut results);
        results
    }

    fn query_point_internal(&self, point: infinitecanvas_math::Vec2, results: &mut Vec<ElementId>) {
        // 如果点不在边界内，跳过
        if !self.bounds.contains(point) {
            return;
        }

        // 检查当前节点的对象
        for obj in &self.objects {
            if obj.bounds.contains(point) && !results.contains(&obj.id) {
                results.push(obj.id);
            }
        }

        // 递归检查子节点
        if let Some(ref children) = self.children {
            for child in children.iter() {
                child.query_point_internal(point, results);
            }
        }
    }

    /// 查询与矩形相交的所有对象
    pub fn query_rect(&self, rect: &Rect) -> Vec<ElementId> {
        let mut results = Vec::new();
        self.query_rect_internal(rect, &mut results);
        results
    }

    fn query_rect_internal(&self, rect: &Rect, results: &mut Vec<ElementId>) {
        // 如果矩形不相交，跳过
        if !self.bounds.intersects(rect) {
            return;
        }

        // 检查当前节点的对象
        for obj in &self.objects {
            if obj.bounds.intersects(rect) && !results.contains(&obj.id) {
                results.push(obj.id);
            }
        }

        // 递归检查子节点
        if let Some(ref children) = self.children {
            for child in children.iter() {
                child.query_rect_internal(rect, results);
            }
        }
    }

    /// 移除对象
    pub fn remove(&mut self, id: ElementId) -> bool {
        // 从当前节点移除
        let before_len = self.objects.len();
        self.objects.retain(|obj| obj.id != id);
        let removed = self.objects.len() < before_len;

        // 从子节点移除
        if let Some(ref mut children) = self.children {
            for child in children.iter_mut() {
                child.remove(id);
            }
        }

        removed
    }

    /// 清空树
    pub fn clear(&mut self) {
        self.objects.clear();
        self.children = None;
    }

    /// 获取对象数量
    pub fn count(&self) -> usize {
        let mut count = self.objects.len();
        if let Some(ref children) = self.children {
            for child in children.iter() {
                count += child.count();
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitecanvas_math::Vec2;

    #[test]
    fn test_insert_and_query_point() {
        let mut tree = QuadTree::new(Rect::new(0.0, 0.0, 1000.0, 1000.0));

        tree.insert(1, Rect::new(100.0, 100.0, 50.0, 50.0));
        tree.insert(2, Rect::new(200.0, 200.0, 50.0, 50.0));
        tree.insert(3, Rect::new(500.0, 500.0, 50.0, 50.0));

        // 查询点在第一个矩形内
        let results = tree.query_point(Vec2::new(125.0, 125.0));
        assert_eq!(results, vec![1]);

        // 查询点不在任何矩形内
        let results = tree.query_point(Vec2::new(0.0, 0.0));
        assert!(results.is_empty());
    }

    #[test]
    fn test_query_rect() {
        let mut tree = QuadTree::new(Rect::new(0.0, 0.0, 1000.0, 1000.0));

        tree.insert(1, Rect::new(100.0, 100.0, 50.0, 50.0));
        tree.insert(2, Rect::new(200.0, 200.0, 50.0, 50.0));
        tree.insert(3, Rect::new(500.0, 500.0, 50.0, 50.0));

        // 查询矩形与前两个相交
        let query = Rect::new(50.0, 50.0, 200.0, 200.0);
        let mut results = tree.query_rect(&query);
        results.sort();
        assert_eq!(results, vec![1, 2]);
    }

    #[test]
    fn test_subdivide() {
        let mut tree = QuadTree::new(Rect::new(0.0, 0.0, 1000.0, 1000.0));

        // 插入足够多的对象触发分裂
        for i in 0..15 {
            tree.insert(
                i as u64,
                Rect::new(i as f32 * 50.0, i as f32 * 50.0, 40.0, 40.0),
            );
        }

        assert!(tree.children.is_some());
    }

    #[test]
    fn test_remove() {
        let mut tree = QuadTree::new(Rect::new(0.0, 0.0, 1000.0, 1000.0));

        tree.insert(1, Rect::new(100.0, 100.0, 50.0, 50.0));
        tree.insert(2, Rect::new(200.0, 200.0, 50.0, 50.0));

        tree.remove(1);

        let results = tree.query_point(Vec2::new(125.0, 125.0));
        assert!(results.is_empty());
    }
}

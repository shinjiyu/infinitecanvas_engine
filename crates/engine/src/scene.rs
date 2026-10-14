//! 场景容器
//!
//! 管理无限画布上的所有元素
use crate::element::{Circle, Element, ElementId, Image, Line, Path, Rectangle, Text};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// 元素枚举（支持不同类型元素）
///
/// JSON 使用 `{"type":"line","data":{...}}` 这种相邻标签。
/// `type` 为小写的 `rectangle`、`circle`、`image`、`line`、`path`、`text`。
/// 反序列化时未知字段会被忽略；缺省的可选字段使用各结构体的 serde 默认值。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
pub enum AnyElement {
    Rectangle(Rectangle),
    Circle(Circle),
    Image(Image),
    Line(Line),
    Path(Path),
    Text(Text),
}

impl AnyElement {
    /// 获取元素 ID
    pub fn id(&self) -> ElementId {
        match self {
            AnyElement::Rectangle(r) => r.props().id,
            AnyElement::Circle(c) => c.props().id,
            AnyElement::Image(i) => i.props().id,
            AnyElement::Line(line) => line.props().id,
            AnyElement::Path(path) => path.props().id,
            AnyElement::Text(t) => t.props().id,
        }
    }

    /// 作为 Element trait 对象
    pub fn as_element(&self) -> &dyn Element {
        match self {
            AnyElement::Rectangle(r) => r,
            AnyElement::Circle(c) => c,
            AnyElement::Image(i) => i,
            AnyElement::Line(line) => line,
            AnyElement::Path(path) => path,
            AnyElement::Text(t) => t,
        }
    }

    /// 作为可变 Element trait 对象
    pub fn as_element_mut(&mut self) -> &mut dyn Element {
        match self {
            AnyElement::Rectangle(r) => r,
            AnyElement::Circle(c) => c,
            AnyElement::Image(i) => i,
            AnyElement::Line(line) => line,
            AnyElement::Path(path) => path,
            AnyElement::Text(t) => t,
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
/// - 版本化 JSON 导入导出（[`Scene::to_json`] / [`Scene::from_json`]）
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
#[derive(Debug)]
pub struct Scene {
    elements: HashMap<ElementId, AnyElement>,
    next_id: ElementId,
    /// 增删元素时递增。顶点缓存据此整表重排，而不是每帧排序。
    ///
    /// 不参与 [`PartialEq`]，也不写入 JSON。
    structure_epoch: u64,
    /// 几何、样式或变换变了、需要重生成顶点的元素。
    ///
    /// 用集合去重，避免多选拖动时按列表扫描。不参与 [`PartialEq`]，也不写入 JSON。
    dirty_ids: HashSet<ElementId>,
    /// 选中集合或选中元素几何变化时递增，用来跳过选择框顶点重建。
    ///
    /// 不参与 [`PartialEq`]，也不写入 JSON。
    selection_epoch: u64,
}

impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.next_id == other.next_id && self.elements == other.elements
    }
}

impl Scene {
    /// 创建空场景
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            next_id: 1,
            structure_epoch: 0,
            dirty_ids: HashSet::new(),
            selection_epoch: 0,
        }
    }

    /// 插入元素并标记结构变化。
    ///
    /// 新元素保持 `dirty = true`。下一次顶点同步会因为 epoch 变化整表重建，
    /// 因此这里不把 id 放进脏列表。
    fn insert_element(&mut self, element: AnyElement) -> ElementId {
        let id = element.id();
        self.elements.insert(id, element);
        if let Some(stored) = self.elements.get_mut(&id) {
            stored.as_element_mut().mark_dirty();
        }
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
        id
    }

    /// 场景结构版本。增删元素后变化。
    pub fn structure_epoch(&self) -> u64 {
        self.structure_epoch
    }

    /// 选择高亮版本。选择集变化，或被选中元素的几何变化时递增。
    pub fn selection_epoch(&self) -> u64 {
        self.selection_epoch
    }

    /// 选择集发生变化（不涉及顶点几何）。
    pub fn bump_selection_epoch(&mut self) {
        self.selection_epoch = self.selection_epoch.wrapping_add(1);
    }

    /// 是否有元素在等待重生成顶点
    pub fn has_dirty_elements(&self) -> bool {
        !self.dirty_ids.is_empty()
    }

    /// 取出并清空待重生成的元素 id
    pub fn take_dirty_ids(&mut self) -> Vec<ElementId> {
        self.dirty_ids.drain().collect()
    }

    /// 结构被整体替换（例如 JSON 导入）后调用。
    ///
    /// 结构版本前进，并把当前每个元素放进脏队列，让下一帧顶点缓存整表重建。
    pub fn invalidate_geometry(&mut self) {
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
        self.dirty_ids.clear();
        let ids: Vec<ElementId> = self.elements.keys().copied().collect();
        for id in ids {
            self.mark_dirty(id);
        }
    }

    /// 记录一个元素的几何、样式或变换已经变化。
    ///
    /// 平移/缩放视口不要调用。同一元素在被缓存消化前重复调用只会入队一次。
    /// 被选中的元素同时推进 [`Self::selection_epoch`]，让高亮框跟着动。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Element, Rectangle, Scene};
    /// use infinitecanvas_engine::math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
    /// scene
    ///     .get_mut(1)
    ///     .unwrap()
    ///     .as_element_mut()
    ///     .props_mut()
    ///     .transform
    ///     .rotate(0.5);
    /// scene.mark_dirty(1);
    /// assert!(scene.has_dirty_elements());
    /// ```
    pub fn mark_dirty(&mut self, id: ElementId) {
        // 已经入队的元素不用重复记。新建元素的 dirty 为 true，但还没入队，
        // 这种情况下仍然要推进去，否则第一次同步前的修改会漏掉选择框刷新。
        if self.dirty_ids.contains(&id) {
            return;
        }
        let selected = {
            let Some(element) = self.elements.get_mut(&id) else {
                return;
            };
            let props = element.as_element_mut().props_mut();
            props.dirty = true;
            props.selected
        };
        self.dirty_ids.insert(id);
        if selected {
            self.selection_epoch = self.selection_epoch.wrapping_add(1);
        }
    }

    /// 用已经校验过的元素表恢复场景
    ///
    /// `next_id` 是下一次 [`Scene::next_id`] 的返回值。
    pub(crate) fn from_parts(elements: HashMap<ElementId, AnyElement>, next_id: ElementId) -> Self {
        let mut scene = Self {
            elements,
            next_id,
            structure_epoch: 0,
            dirty_ids: HashSet::new(),
            selection_epoch: 0,
        };
        scene.invalidate_geometry();
        scene
    }

    /// 下一次 [`Scene::next_id`] 会返回的 ID，不消耗它
    pub fn peek_next_id(&self) -> ElementId {
        self.next_id
    }

    /// 生成新的元素 ID
    pub fn next_id(&mut self) -> ElementId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 添加矩形元素
    pub fn add_rect(&mut self, rect: Rectangle) -> ElementId {
        self.insert_element(AnyElement::Rectangle(rect))
    }

    /// 添加圆形元素
    pub fn add_circle(&mut self, circle: Circle) -> ElementId {
        self.insert_element(AnyElement::Circle(circle))
    }

    /// 添加图片元素
    pub fn add_image(&mut self, image: Image) -> ElementId {
        self.insert_element(AnyElement::Image(image))
    }

    /// 添加直线元素
    pub fn add_line(&mut self, line: Line) -> ElementId {
        self.insert_element(AnyElement::Line(line))
    }

    /// 添加自由画笔路径
    pub fn add_path(&mut self, path: Path) -> ElementId {
        self.insert_element(AnyElement::Path(path))
    }

    /// 添加文本元素
    ///
    /// # 参数
    /// - `text`: 已分配 ID 的文本元素
    ///
    /// # 返回
    /// 元素 ID
    pub fn add_text(&mut self, text: Text) -> ElementId {
        let id = self.insert_element(AnyElement::Text(text));
        self.mark_dirty(id);
        id
    }

    /// 添加任意类型元素
    pub fn add(&mut self, element: AnyElement) -> ElementId {
        self.insert_element(element)
    }

    /// 移除元素
    pub fn remove(&mut self, id: ElementId) -> Option<AnyElement> {
        let removed = self.elements.remove(&id);
        if removed.is_some() {
            self.structure_epoch = self.structure_epoch.wrapping_add(1);
        }
        removed
    }

    /// 获取元素
    pub fn get(&self, id: ElementId) -> Option<&AnyElement> {
        self.elements.get(&id)
    }

    /// 获取可变元素
    pub fn get_mut(&mut self, id: ElementId) -> Option<&mut AnyElement> {
        self.elements.get_mut(&id)
    }

    /// 判断元素是否存在
    pub fn contains(&self, id: ElementId) -> bool {
        self.elements.contains_key(&id)
    }

    /// 清空场景
    pub fn clear(&mut self) {
        if !self.elements.is_empty() {
            self.structure_epoch = self.structure_epoch.wrapping_add(1);
        }
        self.elements.clear();
        self.dirty_ids.clear();
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

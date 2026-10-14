//! 场景容器
//!
//! 管理无限画布上的所有元素、组的父子关系，以及这些元素共享的图片资源。
//!
//! ```text
//! Scene
//!   ├─ roots（parent = None，按 z_index 排序）
//!   ├─ AssetTable（同一张图只存一份像素）
//!   └─ Group
//!        ├─ leaf（世界变换，含矩形、文本和图片）
//!        └─ Group
//!             └─ leaf
//! ```
//!
//! 叶子和组的 `transform` 都是世界变换。移动、旋转或缩放组时，增量写到每个后代上
//! （文本、矩形和图片同一条路径），并把子树标脏，顶点缓存重写纹理四边形。
//! 场景 JSON 用 `{"type":"group","data":{...}}` 和 `{"type":"text","data":{...}}`。
//! `parent` / `locked` 在 `props` 里，缺省为根节点和未锁定。文本的光栅字段不进 JSON。
//! 图片像素在 `assets` 里，元素只存 `asset_id`。
//! 手动参考线在 `guides` 里，和元素一起进出 JSON，不参与绘制。
//! 复制、粘贴和克隆在 [`self::clipboard`]：新 ID、子树重映射，图片引用一起带走，并走命令栈。
use crate::asset::{AssetError, AssetTable, ImageAsset, MAX_IMAGE_EDGE};
use crate::element::{
    Circle, Element, ElementId, Frame, Group, Image, Line, Path, Polygon, Rectangle, Text,
};
use crate::guides::{GuideSnapshot, GuideStore, ManualGuide};
use crate::scene_tree::{SceneTree, SceneTreeNode};
use crate::snap::GuideAxis;
use infinitecanvas_math::{Rect, Transform, Vec2};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

mod clipboard;

/// 成组、解散、进组、出组失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupError {
    /// 至少需要两个元素
    NeedAtLeastTwo,
    /// 场景里没有这个元素
    UnknownElement(ElementId),
    /// 期望一个组
    NotAGroup(ElementId),
    /// 元素不在任何组里
    NotInGroup(ElementId),
    /// 选中的元素不在同一个父节点下
    DifferentParents,
    /// 会把祖先放进自己的子孙里
    WouldCycle,
    /// 元素或它的祖先被锁定
    Locked(ElementId),
    /// 给出的顺序不是这一层兄弟的排列
    InvalidSiblingOrder,
}

impl std::fmt::Display for GroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedAtLeastTwo => write!(f, "成组至少需要两个元素"),
            Self::UnknownElement(id) => write!(f, "元素 {id} 不存在"),
            Self::NotAGroup(id) => write!(f, "元素 {id} 不是组"),
            Self::NotInGroup(id) => write!(f, "元素 {id} 不在组内"),
            Self::DifferentParents => write!(f, "元素不在同一个父节点下"),
            Self::WouldCycle => write!(f, "操作会形成环"),
            Self::Locked(id) => write!(f, "元素 {id} 已锁定"),
            Self::InvalidSiblingOrder => write!(f, "兄弟顺序不是当前列表的排列"),
        }
    }
}

impl std::error::Error for GroupError {}

/// 元素枚举（支持不同类型元素）
///
/// JSON 使用 `{"type":"text","data":{...}}` 这种相邻标签。
/// `type` 为小写的 `rectangle`、`circle`、`polygon`、`image`、`line`、`path`、`text`、`group`、`frame`。
/// 正多边形和星形都是 `polygon`，用 `data.kind`（`polygon` / `star`）区分。
/// 反序列化时未知字段会被忽略；缺省的可选字段使用各结构体的 serde 默认值。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
pub enum AnyElement {
    Rectangle(Rectangle),
    Circle(Circle),
    /// 正多边形或星形。JSON 类型名是 `polygon`，`kind` 区分星形。
    Polygon(Polygon),
    Image(Image),
    Line(Line),
    Path(Path),
    Text(Text),
    Group(Group),
    /// 命名画板。子元素与组相同，尺寸是自己的矩形。
    Frame(Frame),
}

impl AnyElement {
    /// 获取元素 ID
    pub fn id(&self) -> ElementId {
        match self {
            AnyElement::Rectangle(r) => r.props().id,
            AnyElement::Circle(c) => c.props().id,
            AnyElement::Polygon(polygon) => polygon.props().id,
            AnyElement::Image(i) => i.props().id,
            AnyElement::Line(line) => line.props().id,
            AnyElement::Path(path) => path.props().id,
            AnyElement::Text(text) => text.props().id,
            AnyElement::Group(group) => group.props().id,
            AnyElement::Frame(frame) => frame.props().id,
        }
    }

    /// 组或画板的直接子元素。叶子返回 `None`。
    pub fn container_children(&self) -> Option<&[ElementId]> {
        match self {
            AnyElement::Group(group) => Some(group.child_ids()),
            AnyElement::Frame(frame) => Some(frame.child_ids()),
            _ => None,
        }
    }

    /// 组或画板的可变子元素列表。
    pub fn container_children_mut(&mut self) -> Option<&mut Vec<ElementId>> {
        match self {
            AnyElement::Group(group) => Some(&mut group.children),
            AnyElement::Frame(frame) => Some(&mut frame.children),
            _ => None,
        }
    }

    /// 能否作为父节点。组和画板可以。
    pub fn is_container(&self) -> bool {
        self.container_children().is_some()
    }

    /// 如果这是一个组，返回组数据。
    pub fn as_group(&self) -> Option<&Group> {
        match self {
            AnyElement::Group(group) => Some(group),
            _ => None,
        }
    }

    /// 如果这是一个组，返回可变组数据。
    pub fn as_group_mut(&mut self) -> Option<&mut Group> {
        match self {
            AnyElement::Group(group) => Some(group),
            _ => None,
        }
    }

    /// 作为 Element trait 对象
    pub fn as_element(&self) -> &dyn Element {
        match self {
            AnyElement::Rectangle(r) => r,
            AnyElement::Circle(c) => c,
            AnyElement::Polygon(polygon) => polygon,
            AnyElement::Image(i) => i,
            AnyElement::Line(line) => line,
            AnyElement::Path(path) => path,
            AnyElement::Text(text) => text,
            AnyElement::Group(group) => group,
            AnyElement::Frame(frame) => frame,
        }
    }

    /// 作为可变 Element trait 对象
    pub fn as_element_mut(&mut self) -> &mut dyn Element {
        match self {
            AnyElement::Rectangle(r) => r,
            AnyElement::Circle(c) => c,
            AnyElement::Polygon(polygon) => polygon,
            AnyElement::Image(i) => i,
            AnyElement::Line(line) => line,
            AnyElement::Path(path) => path,
            AnyElement::Text(text) => text,
            AnyElement::Group(group) => group,
            AnyElement::Frame(frame) => frame,
        }
    }
}

/// 从场景里摘下的一棵子树。
///
/// [`Scene::take_subtree`] 和 [`Scene::snapshot_subtree`] 生成它，
/// [`Scene::restore_subtree`] 按原来的父节点和兄弟下标放回。
/// 节点按前序排列，第一个是根；`parent` 和组的 `children` 保持摘除前的值。
#[derive(Debug, Clone)]
pub struct RemovedSubtree {
    /// 根在父组 `children` 中的下标。根在顶层时为 `None`。
    index_in_parent: Option<usize>,
    /// 前序节点。第一个是根。
    nodes: Vec<AnyElement>,
}

impl RemovedSubtree {
    /// 前序节点。第一个是根。
    ///
    /// 删除命令用它找出需要暂存到撤销栈的图片。
    pub(crate) fn nodes(&self) -> &[AnyElement] {
        &self.nodes
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
    /// 增删元素或大纲重排（绘制顺序变化）时递增。顶点缓存据此整表重排，而不是每帧排序。
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
    /// 增删元素或几何变化时递增。视口平移/缩放不改变它。
    ///
    /// 小地图用它判断缩略图要不要重画。不参与 [`PartialEq`]，也不写入 JSON。
    geometry_epoch: u64,
    /// 元素自己的 `visible` 或 `locked` 发生变化时递增。
    ///
    /// 图层面板用它重读眼睛和锁。不推进 [`Self::structure_epoch`]，顶点缓存不会因此整表重排。
    /// 不参与 [`PartialEq`]，也不写入 JSON。
    flags_epoch: u64,
    /// 图片像素。同一张图多次使用只存一份。
    assets: AssetTable,
    /// 手动参考线。参与 [`PartialEq`] 和 JSON，不参与顶点缓存。
    guides: GuideStore,
    /// [`Self::remove`] 把引用降到 0 的资源暂存在这里，等撤销栈取走。
    ///
    /// 不参与 [`PartialEq`]，也不写入 JSON。
    detached_assets: Vec<ImageAsset>,
}

/// 重排前的父子关系、组内顺序和 `z_index`。
///
/// [`crate::ReorderOutlineCommand`] 和 [`crate::ReorderSelectionCommand`] 用它撤销。
/// 世界变换不在这份快照里。
#[derive(Debug, Clone)]
pub struct OutlineSnapshot {
    nodes: Vec<OutlineNode>,
}

#[derive(Debug, Clone)]
struct OutlineNode {
    id: ElementId,
    parent: Option<ElementId>,
    z_index: i32,
    children: Option<Vec<ElementId>>,
}

struct OutlinePlan {
    id: ElementId,
    new_parent: Option<ElementId>,
    source_children: Option<(ElementId, Vec<ElementId>)>,
    dest_children: Option<(ElementId, Vec<ElementId>)>,
    root_order: Vec<ElementId>,
    refresh: Vec<ElementId>,
}

impl PartialEq for Scene {
    fn eq(&self, other: &Self) -> bool {
        self.next_id == other.next_id
            && self.elements == other.elements
            && self.assets == other.assets
            && self.guides == other.guides
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
            geometry_epoch: 0,
            flags_epoch: 0,
            assets: AssetTable::new(),
            guides: GuideStore::default(),
            detached_assets: Vec::new(),
        }
    }

    /// 几何版本前进一格。视口变化不要调用。
    fn bump_geometry_epoch(&mut self) {
        self.geometry_epoch = self.geometry_epoch.wrapping_add(1);
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
        self.bump_geometry_epoch();
        id
    }

    /// 场景结构版本。增删元素后变化。
    pub fn structure_epoch(&self) -> u64 {
        self.structure_epoch
    }

    /// 几何版本。增删元素，或 [`Self::mark_dirty`] 时递增。
    ///
    /// 平移/缩放视口不要调用 [`Self::mark_dirty`]，这个版本也不会变。
    /// 小地图据此跳过缩略图重算，只在相机变化时重画视口框。
    pub fn geometry_epoch(&self) -> u64 {
        self.geometry_epoch
    }

    /// 选择高亮版本。选择集变化，或被选中元素的几何变化时递增。
    pub fn selection_epoch(&self) -> u64 {
        self.selection_epoch
    }

    /// 选择集发生变化（不涉及顶点几何）。
    pub fn bump_selection_epoch(&mut self) {
        self.selection_epoch = self.selection_epoch.wrapping_add(1);
    }

    /// 可见性或锁定的版本。只有标志真正变化时递增。
    pub fn flags_epoch(&self) -> u64 {
        self.flags_epoch
    }

    fn bump_flags_epoch(&mut self) {
        self.flags_epoch = self.flags_epoch.wrapping_add(1);
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
    /// 每次成功调用仍推进 [`Self::geometry_epoch`]，小地图才知道外框变了。
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
        // 几何版本照样前进：同一帧里的第二次修改也要让小地图缓存失效。
        if self.dirty_ids.contains(&id) {
            self.bump_geometry_epoch();
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
        self.bump_geometry_epoch();
        if selected {
            self.selection_epoch = self.selection_epoch.wrapping_add(1);
        }
    }

    /// 用已经校验过的元素表和资源表恢复场景
    ///
    /// `next_id` 是下一次 [`Scene::next_id`] 的返回值。
    pub(crate) fn from_parts(
        elements: HashMap<ElementId, AnyElement>,
        next_id: ElementId,
        assets: AssetTable,
    ) -> Self {
        let mut scene = Self {
            elements,
            next_id,
            structure_epoch: 0,
            dirty_ids: HashSet::new(),
            selection_epoch: 0,
            geometry_epoch: 0,
            flags_epoch: 0,
            assets,
            guides: GuideStore::default(),
            detached_assets: Vec::new(),
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

    /// 添加正多边形或星形，并 [`Scene::mark_dirty`]。
    ///
    /// 新建元素已经标了脏，这里再入队，顶点缓存和选择框都会看到这次创建。
    pub fn add_polygon(&mut self, polygon: Polygon) -> ElementId {
        let id = self.insert_element(AnyElement::Polygon(polygon));
        self.mark_dirty(id);
        id
    }

    /// 添加画板，并 [`Scene::mark_dirty`]。
    ///
    /// 画板按自己的 `size` 绘制，不根据子元素改尺寸。
    pub fn add_frame(&mut self, frame: Frame) -> ElementId {
        let id = self.insert_element(AnyElement::Frame(frame));
        self.mark_dirty(id);
        id
    }

    /// 添加图片元素
    ///
    /// 若 `asset_id` 在资源表里，引用计数加一。
    pub fn add_image(&mut self, image: Image) -> ElementId {
        self.assets.retain(&image.asset_id);
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
    ///
    /// 图片元素会增加对应资源的引用计数。
    pub fn add(&mut self, element: AnyElement) -> ElementId {
        if let AnyElement::Image(image) = &element {
            self.assets.retain(&image.asset_id);
        }
        self.insert_element(element)
    }

    /// 移除元素。如果它是组，连同整棵子树一起删除。
    ///
    /// 只想去掉组、留下子元素时用 [`Scene::ungroup`]。
    /// 返回被删除的根元素；子孙从场景中丢弃。
    /// 每个被删掉的图片都会减少资源引用。降到 0 时资源离开表，
    /// 暂存在 [`Self::take_detached_assets`]。
    /// 要撤销这次删除，用 [`Scene::take_subtree`]，它会把子孙一并留下。
    pub fn remove(&mut self, id: ElementId) -> Option<AnyElement> {
        if !self.elements.contains_key(&id) {
            return None;
        }
        let parent = self.parent_id(id);
        let ids = self.subtree_ids(id);
        self.detach_from_parent(id);
        let mut root = None;
        for (index, element_id) in ids.into_iter().enumerate() {
            self.dirty_ids.remove(&element_id);
            if let Some(element) = self.elements.remove(&element_id) {
                self.release_image_element(&element);
                if index == 0 {
                    root = Some(element);
                }
            }
        }
        if root.is_some() {
            self.structure_epoch = self.structure_epoch.wrapping_add(1);
            self.bump_geometry_epoch();
        }
        if let Some(parent_id) = parent {
            self.recompute_group_bounds(parent_id);
        }
        root
    }

    /// 图片进入场景时增加引用。没有 `asset_id`，或资源不在表里时什么都不做。
    fn retain_image_element(&mut self, element: &AnyElement) {
        let AnyElement::Image(image) = element else {
            return;
        };
        self.assets.retain(&image.asset_id);
    }

    /// 图片离开场景时减少引用。最后一份引用把像素移到 [`Self::detached_assets`]。
    fn release_image_element(&mut self, element: &AnyElement) {
        let AnyElement::Image(image) = element else {
            return;
        };
        if let Some(asset) = self.assets.release(&image.asset_id) {
            self.detached_assets.push(asset);
        }
    }

    /// 取走引用已经降到 0 的资源。
    ///
    /// 撤销命令把它们放进历史；直接删除时丢掉返回值即释放像素。
    pub fn take_detached_assets(&mut self) -> Vec<ImageAsset> {
        std::mem::take(&mut self.detached_assets)
    }

    /// 把撤销栈暂存的资源放回表中（引用计数为 0，随后的 [`Self::add`] 会加一）。
    pub fn restore_assets(&mut self, assets: Vec<ImageAsset>) {
        for asset in assets {
            self.assets.reinsert(asset);
        }
    }

    /// 解码 PNG/JPEG 并放进资源表。
    ///
    /// 同一份字节只存一次。边长超过 [`MAX_IMAGE_EDGE`] 时先降采样。
    /// 新资源的引用计数为 0，直到有 [`Image`] 元素引用它。
    ///
    /// # 参数
    /// - `bytes`: 文件字节
    /// - `mime`: `image/png` 或 `image/jpeg`。空字符串时按文件头识别。
    ///
    /// # 返回
    /// 资源 id。元素的 `asset_id` 和 GPU 纹理 id 都用它。
    ///
    /// # 错误
    /// 格式不支持或解码失败。
    pub fn insert_image_bytes(&mut self, bytes: &[u8], mime: &str) -> Result<String, AssetError> {
        self.assets.insert_encoded(bytes, mime, MAX_IMAGE_EDGE)
    }

    /// 按 id 取图片资源。
    pub fn image_asset(&self, id: &str) -> Option<&ImageAsset> {
        self.assets.get(id)
    }

    /// 资源表里的条目数。
    pub fn image_asset_count(&self) -> usize {
        self.assets.len()
    }

    /// 某个资源当前被多少个元素引用。
    pub fn asset_refcount(&self, id: &str) -> usize {
        self.assets.refs(id)
    }

    /// 仍被元素引用、需要上传 GPU 的资源 id。
    pub fn referenced_asset_ids(&self) -> HashSet<String> {
        self.assets.referenced_ids()
    }

    /// 写入场景 JSON 的资源记录。只包含仍被引用的资源，按 id 排序。
    pub fn image_asset_records(&self) -> Vec<crate::asset::AssetRecord> {
        self.assets.to_records()
    }

    /// 克隆元素及其全部后代，不从场景中删除。
    ///
    /// 根不在场景中时返回 `None`。多个根要一起删除时，先对每个根调用本方法，
    /// 再 [`Scene::remove`]，这样兄弟下标还是删除前的值。
    ///
    /// # 参数
    /// - `id`: 子树根
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::new(10.0, 10.0)));
    /// let group_id = scene.group(&[1, 2]).unwrap();
    /// assert!(scene.snapshot_subtree(group_id).is_some());
    /// assert!(scene.contains(1));
    /// ```
    pub fn snapshot_subtree(&self, id: ElementId) -> Option<RemovedSubtree> {
        if !self.elements.contains_key(&id) {
            return None;
        }
        let parent = self.parent_id(id);
        let index_in_parent = parent.and_then(|parent_id| {
            self.group_child_ids(parent_id)
                .iter()
                .position(|child| *child == id)
        });
        let nodes = self
            .subtree_ids(id)
            .into_iter()
            .filter_map(|element_id| self.elements.get(&element_id).cloned())
            .collect::<Vec<_>>();
        if nodes.is_empty() {
            return None;
        }
        Some(RemovedSubtree {
            index_in_parent,
            nodes,
        })
    }

    /// 删除根及其全部后代，并留下可放回的快照。
    ///
    /// [`Scene::remove`] 只返回根。撤销删除要用这个方法，再 [`Scene::restore_subtree`]。
    ///
    /// # 参数
    /// - `id`: 子树根
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::new(10.0, 10.0)));
    /// let group_id = scene.group(&[1, 2]).unwrap();
    /// let snapshot = scene.take_subtree(group_id).unwrap();
    /// assert!(!scene.contains(1) && !scene.contains(group_id));
    /// scene.restore_subtree(snapshot);
    /// assert_eq!(scene.children_of(group_id), vec![1, 2]);
    /// ```
    pub fn take_subtree(&mut self, id: ElementId) -> Option<RemovedSubtree> {
        let snapshot = self.snapshot_subtree(id)?;
        self.remove(id)?;
        Some(snapshot)
    }

    /// 把 [`Scene::snapshot_subtree`] 或 [`Scene::take_subtree`] 的结果放回场景。
    ///
    /// 父组还在时，根按原来的下标插回子列表。父组已经不在时，根回到顶层。
    /// 组自己的变换用快照里的值，不在这里重算；父组的包围盒会按放回后的子元素刷新。
    /// 图片会重新增加资源引用。像素若还在 [`Self::take_detached_assets`] 的缓冲区里，
    /// 会先放回资源表；命令把像素暂存到撤销栈时，要先 [`Self::restore_assets`]。
    ///
    /// # 参数
    /// - `snapshot`: 删除前克隆的子树
    pub fn restore_subtree(&mut self, mut snapshot: RemovedSubtree) {
        let Some(root) = snapshot.nodes.first() else {
            return;
        };
        let root_id = root.id();
        let declared_parent = root.as_element().props().parent;
        let parent = declared_parent.filter(|parent_id| {
            self.elements
                .get(parent_id)
                .is_some_and(AnyElement::is_container)
        });
        if parent.is_none() {
            if let Some(root) = snapshot.nodes.first_mut() {
                root.as_element_mut().props_mut().parent = None;
            }
        }
        self.reclaim_detached_assets(&snapshot.nodes);
        let index = snapshot.index_in_parent.unwrap_or(usize::MAX);
        for node in snapshot.nodes {
            self.retain_image_element(&node);
            self.insert_element(node);
        }
        self.mark_subtree_dirty(root_id);
        if let Some(parent_id) = parent {
            self.insert_child(parent_id, root_id, index);
            self.refresh_bounds_upwards(parent_id);
            self.mark_dirty(parent_id);
        }
    }

    /// 把仍留在脱离缓冲区、且被这些节点引用的图片放回资源表。
    ///
    /// 引用计数保持 0，随后的 [`Self::retain_image_element`] 再加一。
    /// 缓冲区里其他资源不动。命令已经把像素取走时，这里什么都找不到。
    fn reclaim_detached_assets(&mut self, nodes: &[AnyElement]) {
        let keep: HashSet<String> = nodes
            .iter()
            .filter_map(|element| match element {
                AnyElement::Image(image) if !image.asset_id.is_empty() => {
                    Some(image.asset_id.clone())
                }
                _ => None,
            })
            .collect();
        if keep.is_empty() {
            return;
        }
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.detached_assets)
            .into_iter()
            .partition(|asset| keep.contains(&asset.id));
        self.detached_assets = rest;
        self.restore_assets(mine);
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
            self.bump_geometry_epoch();
        }
        self.elements.clear();
        self.dirty_ids.clear();
        self.assets.clear();
        self.detached_assets.clear();
        self.guides = GuideStore::default();
    }

    /// 手动参考线。空场景时也是空集合。
    pub fn guides(&self) -> &GuideStore {
        &self.guides
    }

    /// 新增一条参考线。坐标不是有限数时返回 `None`。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::snap::GuideAxis;
    /// use infinitecanvas_engine::Scene;
    ///
    /// let mut scene = Scene::new();
    /// let guide = scene.add_guide(GuideAxis::Horizontal, 12.0).unwrap();
    /// assert_eq!(scene.guides().len(), 1);
    /// assert_eq!(guide.position, 12.0);
    /// ```
    pub fn add_guide(&mut self, axis: GuideAxis, position: f32) -> Option<ManualGuide> {
        self.guides.add(axis, position)
    }

    /// 删除一条参考线。
    pub fn remove_guide(&mut self, id: u64) -> bool {
        self.guides.remove(id)
    }

    /// 移动一条参考线。
    pub fn move_guide(&mut self, id: u64, position: f32) -> bool {
        self.guides.move_to(id, position)
    }

    /// 参考线快照，给撤销命令用。
    pub fn guide_snapshot(&self) -> GuideSnapshot {
        self.guides.snapshot()
    }

    /// 用快照替换全部参考线。
    pub fn restore_guides(&mut self, snapshot: &GuideSnapshot) {
        self.guides.restore(snapshot);
    }

    /// 导入 JSON 时写入已经校验过的参考线。
    pub(crate) fn set_guide_store(&mut self, guides: GuideStore) {
        self.guides = guides;
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

    /// 可见叶子的世界包围盒并集。
    ///
    /// 组节点不计入，范围由它的子元素表达。元素自己不可见，或祖先被隐藏时，
    /// 也不计入。空场景、没有可见叶子，或包围盒含非有限值时，返回 `None`。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    /// let mut scene = Scene::new();
    /// assert!(scene.content_bounds().is_none());
    /// scene.add_rect(Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 50.0)));
    /// let bounds = scene.content_bounds().unwrap();
    /// assert!((bounds.width - 50.0).abs() < 1e-3);
    /// ```
    pub fn content_bounds(&self) -> Option<Rect> {
        let ids: Vec<ElementId> = self
            .iter()
            .filter(|element| element.as_group().is_none())
            .map(|element| element.id())
            .collect();
        let mut acc: Option<Rect> = None;
        for id in ids {
            if !self.is_effectively_visible(id) {
                continue;
            }
            let Some(element) = self.get(id) else {
                continue;
            };
            let bounds = element.as_element().world_bounds();
            if !bounds.x.is_finite()
                || !bounds.y.is_finite()
                || !bounds.width.is_finite()
                || !bounds.height.is_finite()
            {
                continue;
            }
            acc = Some(match acc {
                Some(previous) => previous.union(&bounds),
                None => bounds,
            });
        }
        acc
    }

    /// 查找指定位置的元素（从上到下）
    ///
    /// 组本身不接点击。祖先被隐藏或锁定的元素也不会被点中。
    pub fn find_at(&self, point: Vec2) -> Option<ElementId> {
        let mut elements = self.elements_sorted();
        elements.reverse(); // 从顶层开始查找

        for element in elements {
            if !self.is_effectively_visible(element.id())
                || self.is_effectively_locked(element.id())
            {
                continue;
            }
            if element.as_element().contains_point(point) {
                return Some(element.id());
            }
        }

        None
    }

    /// 把至少两个同一父节点下的元素收成一个组。
    ///
    /// 子元素留在场景里，世界坐标不变。组的平移放在子内容包围盒中心。
    ///
    /// # 参数
    /// - `ids`: 要成组的元素，顺序会变成组内顺序
    ///
    /// # 返回
    /// 新组的 ID
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(40.0, 0.0), Vec2::new(10.0, 10.0)));
    /// let group_id = scene.group(&[1, 2]).unwrap();
    /// scene.translate_element(group_id, Vec2::new(5.0, 0.0)).unwrap();
    /// let moved = scene.get(1).unwrap().as_element().props().transform.translation;
    /// assert_eq!(moved, Vec2::new(5.0, 0.0));
    /// let children = scene.ungroup(group_id).unwrap();
    /// assert_eq!(children, vec![1, 2]);
    /// assert!(scene.contains(1) && scene.contains(2));
    /// assert!(!scene.contains(group_id));
    /// ```
    pub fn group(&mut self, ids: &[ElementId]) -> Result<ElementId, GroupError> {
        let members = self.normalize_members(ids)?;
        let parent = self.parent_id(members[0]);
        for id in &members[1..] {
            if self.parent_id(*id) != parent {
                return Err(GroupError::DifferentParents);
            }
        }
        for id in &members {
            if self.member_contains_another(*id, &members) {
                return Err(GroupError::WouldCycle);
            }
        }

        let insert_at = parent
            .and_then(|parent_id| self.child_index(parent_id, &members))
            .unwrap_or(0);
        let z_index = members
            .iter()
            .filter_map(|id| {
                self.elements
                    .get(id)
                    .map(|element| element.as_element().props().z_index)
            })
            .max()
            .unwrap_or(0);

        for id in &members {
            self.detach_from_parent(*id);
        }

        let group_id = self.alloc_id();
        let mut group = Group::new(group_id);
        group.props.z_index = z_index;
        group.props.parent = parent;
        group.children = members.clone();
        self.elements.insert(group_id, AnyElement::Group(group));
        for id in &members {
            self.set_parent(*id, Some(group_id));
        }
        if let Some(parent_id) = parent {
            self.insert_child(parent_id, group_id, insert_at);
        }
        self.place_new_group_frame(group_id);
        self.refresh_bounds_upwards(group_id);
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
        Ok(group_id)
    }

    /// 解散组。组节点被删除，子元素留在场景中，世界坐标不变，并提升到组原来的父节点。
    ///
    /// # 返回
    /// 原来的直接子元素 ID，顺序与组内一致。
    pub fn ungroup(&mut self, group_id: ElementId) -> Result<Vec<ElementId>, GroupError> {
        self.ensure_editable(group_id)?;
        let children = self.group_children(group_id)?;
        let parent = self.parent_id(group_id);
        let index = parent
            .and_then(|parent_id| {
                self.group_child_ids(parent_id)
                    .iter()
                    .position(|id| *id == group_id)
            })
            .unwrap_or(0);

        self.detach_from_parent(group_id);
        self.elements.remove(&group_id);
        self.dirty_ids.remove(&group_id);
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
        for (offset, child_id) in children.iter().enumerate() {
            if !self.elements.contains_key(child_id) {
                continue;
            }
            self.set_parent(*child_id, parent);
            if let Some(parent_id) = parent {
                self.insert_child(parent_id, *child_id, index + offset);
            }
        }
        if let Some(parent_id) = parent {
            self.refresh_bounds_upwards(parent_id);
        }
        Ok(children)
    }

    /// 把元素放进组。世界坐标保持不变。
    ///
    /// # 参数
    /// - `element_id`: 要移入的元素，可以是叶子或另一个组
    /// - `group_id`: 目标组
    pub fn enter_group(
        &mut self,
        element_id: ElementId,
        group_id: ElementId,
    ) -> Result<(), GroupError> {
        self.ensure_known(element_id)?;
        self.container_child_list(group_id)?;
        if element_id == group_id || self.is_ancestor(element_id, group_id) {
            return Err(GroupError::WouldCycle);
        }
        self.ensure_editable(element_id)?;
        self.ensure_editable(group_id)?;
        if self.parent_id(element_id) == Some(group_id) {
            return Ok(());
        }
        let old_parent = self.parent_id(element_id);
        self.detach_from_parent(element_id);
        self.set_parent(element_id, Some(group_id));
        self.insert_child(group_id, element_id, usize::MAX);
        if let Some(old_parent) = old_parent {
            self.recompute_group_bounds(old_parent);
        }
        self.refresh_bounds_upwards(group_id);
        Ok(())
    }

    /// 把元素移出它所在的组，挂到组的父节点（没有则回到根）。世界坐标不变。
    pub fn leave_group(&mut self, element_id: ElementId) -> Result<(), GroupError> {
        self.ensure_known(element_id)?;
        let parent = self
            .parent_id(element_id)
            .ok_or(GroupError::NotInGroup(element_id))?;
        self.container_child_list(parent)?;
        self.ensure_editable(element_id)?;
        let grandparent = self.parent_id(parent);
        let insert_at = grandparent
            .and_then(|grandparent_id| {
                self.group_child_ids(grandparent_id)
                    .iter()
                    .position(|id| *id == parent)
                    .map(|index| index + 1)
            })
            .unwrap_or(0);
        self.detach_from_parent(element_id);
        self.set_parent(element_id, grandparent);
        if let Some(grandparent_id) = grandparent {
            self.insert_child(grandparent_id, element_id, insert_at);
        }
        self.recompute_group_bounds(parent);
        if let Some(grandparent_id) = grandparent {
            self.refresh_bounds_upwards(grandparent_id);
        }
        Ok(())
    }

    /// 平移元素。目标是组时，所有后代的世界平移加上同一个 `delta`。
    ///
    /// 子树上每个元素都会 [`Scene::mark_dirty`]，顶点缓存只重写这些元素。
    pub fn translate_element(&mut self, id: ElementId, delta: Vec2) -> Result<(), GroupError> {
        self.ensure_editable(id)?;
        if delta == Vec2::ZERO {
            return Ok(());
        }
        for element_id in self.subtree_ids(id) {
            if let Some(element) = self.elements.get_mut(&element_id) {
                element
                    .as_element_mut()
                    .props_mut()
                    .transform
                    .translate(delta);
            }
        }
        self.refresh_bounds_upwards(id);
        self.mark_subtree_dirty(id);
        Ok(())
    }

    /// 平移一批元素。列表里如果同时有组和它的子孙，子孙只跟着组走一次。
    pub fn translate_ids(&mut self, ids: &[ElementId], delta: Vec2) {
        if delta == Vec2::ZERO {
            return;
        }
        let selected: HashSet<ElementId> = ids
            .iter()
            .copied()
            .filter(|id| self.elements.contains_key(id))
            .collect();
        for id in ids {
            if !selected.contains(id) || self.has_ancestor_in(*id, &selected) {
                continue;
            }
            let _ = self.translate_element(*id, delta);
        }
    }

    /// 设置元素的世界变换。目标是组时，后代按 `new = to * inverse(from) * child` 跟着变。
    ///
    /// 与 #7 的缩放 / 旋转手柄共用这一条：手柄算出组的新变换后写回来，子元素世界坐标保持相对关系。
    /// 子树会被标脏，顶点缓存按元素重生成。
    pub fn set_world_transform(
        &mut self,
        id: ElementId,
        new_transform: Transform,
    ) -> Result<(), GroupError> {
        self.ensure_editable(id)?;
        let old = self
            .elements
            .get(&id)
            .unwrap()
            .as_element()
            .props()
            .transform;
        if old == new_transform {
            return Ok(());
        }
        let descendants = self.descendant_ids(id);
        let mut updates = Vec::with_capacity(descendants.len());
        for descendant_id in descendants {
            let current = self
                .elements
                .get(&descendant_id)
                .unwrap()
                .as_element()
                .props()
                .transform;
            updates.push((descendant_id, current.rebase_between(old, new_transform)));
        }
        for (descendant_id, transform) in updates {
            if let Some(element) = self.elements.get_mut(&descendant_id) {
                element.as_element_mut().props_mut().transform = transform;
            }
        }
        if let Some(element) = self.elements.get_mut(&id) {
            element.as_element_mut().props_mut().transform = new_transform;
        }
        self.refresh_bounds_upwards(id);
        self.mark_subtree_dirty(id);
        Ok(())
    }

    /// 写入一批世界变换。某个元素的子孙也在列表里时，不再把组的增量重复套到这些子孙上
    /// （#7 多选时已经为每个元素算好了绝对变换）。
    pub fn apply_world_transforms(&mut self, states: &[(ElementId, Transform)]) {
        let selected: HashSet<ElementId> = states
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| self.elements.contains_key(id))
            .collect();
        for (id, transform) in states {
            if !selected.contains(id) || self.lock_blocker(*id).is_some() {
                continue;
            }
            if self.has_selected_descendant(*id, &selected) {
                self.write_transform(*id, *transform);
                self.refresh_bounds_upwards(*id);
                self.mark_dirty(*id);
            } else {
                let _ = self.set_world_transform(*id, *transform);
            }
        }
    }

    /// 父元素。不在场景里或挂在根上时返回 `None`。
    pub fn parent_of(&self, id: ElementId) -> Option<ElementId> {
        self.parent_id(id)
    }

    /// 直接子元素。不是组时返回空列表。
    pub fn children_of(&self, id: ElementId) -> Vec<ElementId> {
        self.group_child_ids(id)
    }

    /// 元素自己加上全部后代，前序。
    pub fn subtree_ids(&self, id: ElementId) -> Vec<ElementId> {
        let mut ids = Vec::new();
        self.collect_subtree(id, &mut ids);
        ids
    }

    /// 自己和祖先都可见时返回 true。
    pub fn is_effectively_visible(&self, id: ElementId) -> bool {
        let mut current = Some(id);
        while let Some(element_id) = current {
            let Some(element) = self.elements.get(&element_id) else {
                return false;
            };
            if !element.as_element().props().visible {
                return false;
            }
            current = element.as_element().props().parent;
        }
        true
    }

    /// 自己或任一祖先被锁定时返回 true。
    ///
    /// 命中测试、框选和拖动都用它。元素不在场景里时返回 false。
    pub fn is_effectively_locked(&self, id: ElementId) -> bool {
        self.lock_blocker(id).is_some()
    }

    /// 设置可见性。隐藏组不会改子元素自己的标志，但命中和绘制会把子孙一并跳过。
    ///
    /// 子树会被标脏，这样隐藏或重新显示后顶点缓存会丢掉或写回子孙的三角形。
    /// 标志有变化时推进 [`Self::flags_epoch`]，不推进 [`Self::structure_epoch`]。
    pub fn set_visible(&mut self, id: ElementId, visible: bool) -> bool {
        if !self.elements.contains_key(&id) {
            return false;
        }
        let changed = self
            .elements
            .get(&id)
            .is_some_and(|element| element.as_element().props().visible != visible);
        if let Some(element) = self.elements.get_mut(&id) {
            element.as_element_mut().props_mut().visible = visible;
        }
        if changed {
            self.bump_flags_epoch();
        }
        self.mark_subtree_dirty(id);
        true
    }

    /// 设置锁定。解锁也走这里，因此不再检查当前是否已锁定。
    ///
    /// 只改这个元素自己的 `locked`。祖先已锁定时，子孙仍然 [`Self::is_effectively_locked`]。
    /// 标志有变化时推进 [`Self::flags_epoch`]，不改几何，也不推进 [`Self::structure_epoch`]。
    pub fn set_locked(&mut self, id: ElementId, locked: bool) -> bool {
        let changed = match self.elements.get(&id) {
            Some(element) => element.as_element().props().locked != locked,
            None => return false,
        };
        if let Some(element) = self.elements.get_mut(&id) {
            element.as_element_mut().props_mut().locked = locked;
        }
        if changed {
            self.bump_flags_epoch();
        }
        true
    }

    /// 导出大纲面板用的场景树：根 → 组 → 叶子，带可见性和锁定。
    pub fn scene_tree(&self) -> SceneTree {
        let mut roots: Vec<ElementId> = self
            .elements
            .keys()
            .copied()
            .filter(|id| self.parent_id(*id).is_none())
            .collect();
        roots.sort_by_key(|id| {
            let element = self.elements.get(id).unwrap().as_element();
            (element.props().z_index, *id)
        });
        let mut visiting = HashSet::new();
        let nodes = roots
            .into_iter()
            .filter_map(|id| self.build_tree_node(id, &mut visiting))
            .collect();
        SceneTree { roots: nodes }
    }

    /// 把元素挪到大纲中的新位置。
    ///
    /// `new_parent` 为 `None` 时挂到根上。`index` 是从原位置摘下之后、在目标兄弟列表里的下标，
    /// `0` 为最底层（先绘制）。成功后按深度优先重写全部 `z_index`（越靠后的兄弟越大），
    /// 并推进 [`Scene::structure_epoch`]，顶点缓存下一帧按新顺序重排。世界坐标不变。
    ///
    /// # 返回
    /// `Ok(None)` 表示位置没有变化。`Ok(Some(groups))` 是子元素集合变了、需要重算包围盒的组。
    ///
    /// # 错误
    /// 元素不存在、目标不是组、会成环，或元素 / 目标组已锁定。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(30.0, 0.0), Vec2::new(10.0, 10.0)));
    /// scene.move_in_outline(1, None, 1).unwrap();
    /// let order: Vec<_> = scene
    ///     .elements_sorted()
    ///     .into_iter()
    ///     .map(|element| element.id())
    ///     .collect();
    /// assert_eq!(order, vec![2, 1]);
    /// ```
    pub fn move_in_outline(
        &mut self,
        id: ElementId,
        new_parent: Option<ElementId>,
        index: usize,
    ) -> Result<Option<Vec<ElementId>>, GroupError> {
        let Some(plan) = self.plan_outline_move(id, new_parent, index)? else {
            return Ok(None);
        };
        let refresh = plan.refresh.clone();
        self.apply_outline_plan(plan);
        Ok(Some(refresh))
    }

    /// 同一父节点下的兄弟，下标 `0` 是最底层（先绘制）。
    ///
    /// `parent` 为 `None` 时返回根，按 `z_index` 再按 id。组则返回 `children` 的存储顺序。
    /// 父 id 不是组时返回空列表。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(8.0, 0.0), Vec2::new(4.0, 4.0)));
    /// assert_eq!(scene.sibling_order(None), vec![1, 2]);
    /// ```
    pub fn sibling_order(&self, parent: Option<ElementId>) -> Vec<ElementId> {
        match parent {
            Some(parent_id) => self.group_child_ids(parent_id),
            None => self.root_ids_back_to_front(),
        }
    }

    /// 把同一父节点下的兄弟排成 `order`（下标 `0` 为最底层）。
    ///
    /// `order` 必须是当前兄弟的一个排列。成功后按深度优先重写全部 `z_index`
    /// （越靠后的兄弟越大），并推进 [`Scene::structure_epoch`]。顶点缓存下一帧按新顺序整表重排。
    /// 不改世界坐标，也不把元素标成几何脏：三角形没变，和 [`Scene::move_in_outline`] 一样。
    ///
    /// 父组被锁定时返回 [`GroupError::Locked`]，不改顺序。顺序本来就一样时返回 `Ok(false)`，
    /// 不推进结构版本。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, Scene};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(10.0, 0.0), Vec2::new(4.0, 4.0)));
    /// assert!(scene.arrange_siblings(None, &[2, 1]).unwrap());
    /// assert_eq!(scene.sibling_order(None), vec![2, 1]);
    /// assert!(!scene.arrange_siblings(None, &[2, 1]).unwrap());
    /// ```
    pub fn arrange_siblings(
        &mut self,
        parent: Option<ElementId>,
        order: &[ElementId],
    ) -> Result<bool, GroupError> {
        if let Some(parent_id) = parent {
            self.container_child_list(parent_id)?;
        }
        let current = self.sibling_order(parent);
        if !Self::permutation_matches(&current, order) {
            return Err(GroupError::InvalidSiblingOrder);
        }
        if current == order {
            return Ok(false);
        }
        if let Some(parent_id) = parent {
            self.ensure_editable(parent_id)?;
            self.set_group_children(parent_id, order.to_vec());
            let roots = self.root_ids_back_to_front();
            self.assign_z_order(&roots);
        } else {
            self.assign_z_order(order);
        }
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
        Ok(true)
    }

    /// 记下当前大纲，供撤销时原样写回。
    pub fn outline_snapshot(&self) -> OutlineSnapshot {
        let mut ids: Vec<ElementId> = self.elements.keys().copied().collect();
        ids.sort_unstable();
        let nodes = ids
            .into_iter()
            .filter_map(|id| {
                let element = self.elements.get(&id)?;
                let parent = element.as_element().props().parent;
                let z_index = element.as_element().props().z_index;
                let children = element.container_children().map(|ids| ids.to_vec());
                Some(OutlineNode {
                    id,
                    parent,
                    z_index,
                    children,
                })
            })
            .collect();
        OutlineSnapshot { nodes }
    }

    /// 写回 [`Scene::outline_snapshot`] 的父子关系和 `z_index`。
    ///
    /// `refresh` 里的组会按当前子元素重算包围盒。结构版本前进，绘制顺序回到快照。
    pub fn restore_outline(&mut self, snapshot: &OutlineSnapshot, refresh: &[ElementId]) {
        for node in &snapshot.nodes {
            if let Some(element) = self.elements.get_mut(&node.id) {
                let props = element.as_element_mut().props_mut();
                props.parent = node.parent;
                props.z_index = node.z_index;
            }
            if let Some(children) = &node.children {
                self.set_group_children(node.id, children.clone());
            }
        }
        if !refresh.is_empty() {
            for group_id in refresh {
                if self.elements.contains_key(group_id) {
                    self.refresh_bounds_upwards(*group_id);
                }
            }
            self.bump_selection_epoch();
        }
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
    }

    fn plan_outline_move(
        &self,
        id: ElementId,
        new_parent: Option<ElementId>,
        index: usize,
    ) -> Result<Option<OutlinePlan>, GroupError> {
        self.ensure_editable(id)?;
        if let Some(parent_id) = new_parent {
            if parent_id == id || self.is_ancestor(id, parent_id) {
                return Err(GroupError::WouldCycle);
            }
            self.container_child_list(parent_id)?;
            self.ensure_editable(parent_id)?;
        }

        let old_parent = self.parent_id(id);
        let old_siblings = self.sibling_ids(old_parent)?;
        let Some(old_index) = old_siblings.iter().position(|sibling| *sibling == id) else {
            return Err(GroupError::UnknownElement(id));
        };

        let mut dest = if old_parent == new_parent {
            old_siblings.clone()
        } else {
            self.sibling_ids(new_parent)?
        };
        if old_parent == new_parent {
            dest.remove(old_index);
        }
        let clamped = index.min(dest.len());
        if old_parent == new_parent && clamped == old_index {
            return Ok(None);
        }
        dest.insert(clamped, id);

        let mut refresh = Vec::new();
        if old_parent != new_parent {
            if let Some(parent_id) = old_parent {
                refresh.push(parent_id);
            }
            if let Some(parent_id) = new_parent {
                refresh.push(parent_id);
            }
        }

        let root_order = if new_parent.is_none() {
            dest.clone()
        } else if old_parent.is_none() {
            let mut roots = old_siblings.clone();
            roots.retain(|root_id| *root_id != id);
            roots
        } else {
            self.root_ids_back_to_front()
        };
        let source_children = if old_parent == new_parent {
            None
        } else {
            old_parent.map(|parent_id| {
                let mut children = old_siblings;
                children.retain(|child| *child != id);
                (parent_id, children)
            })
        };
        let dest_children = new_parent.map(|parent_id| (parent_id, dest));
        Ok(Some(OutlinePlan {
            id,
            new_parent,
            source_children,
            dest_children,
            root_order,
            refresh,
        }))
    }

    fn apply_outline_plan(&mut self, plan: OutlinePlan) {
        if let Some((group_id, children)) = plan.source_children {
            self.set_group_children(group_id, children);
        }
        if let Some((group_id, children)) = plan.dest_children {
            self.set_group_children(group_id, children);
        }
        self.set_parent(plan.id, plan.new_parent);
        self.assign_z_order(&plan.root_order);
        if !plan.refresh.is_empty() {
            for group_id in &plan.refresh {
                self.refresh_bounds_upwards(*group_id);
            }
            self.bump_selection_epoch();
        }
        self.structure_epoch = self.structure_epoch.wrapping_add(1);
    }

    fn sibling_ids(&self, parent: Option<ElementId>) -> Result<Vec<ElementId>, GroupError> {
        match parent {
            Some(parent_id) => self.container_child_list(parent_id),
            None => Ok(self.root_ids_back_to_front()),
        }
    }

    /// `order` 含有重复项，或不是 `current` 的重排。
    fn permutation_matches(current: &[ElementId], order: &[ElementId]) -> bool {
        if current.len() != order.len() {
            return false;
        }
        let mut seen = HashSet::with_capacity(current.len());
        for id in order {
            if !seen.insert(*id) {
                return false;
            }
        }
        current.iter().all(|id| seen.contains(id))
    }

    fn root_ids_back_to_front(&self) -> Vec<ElementId> {
        let mut roots: Vec<ElementId> = self
            .elements
            .keys()
            .copied()
            .filter(|id| self.parent_id(*id).is_none())
            .collect();
        roots.sort_by_key(|id| {
            let z_index = self
                .elements
                .get(id)
                .map(|element| element.as_element().props().z_index)
                .unwrap_or(0);
            (z_index, *id)
        });
        roots
    }

    fn set_group_children(&mut self, group_id: ElementId, children: Vec<ElementId>) {
        if let Some(stored) = self
            .elements
            .get_mut(&group_id)
            .and_then(AnyElement::container_children_mut)
        {
            *stored = children;
        }
    }

    fn assign_z_order(&mut self, root_order: &[ElementId]) {
        let mut z_index = 0i32;
        let mut visiting = HashSet::new();
        for id in root_order {
            self.write_z_dfs(*id, &mut z_index, &mut visiting);
        }
    }

    fn write_z_dfs(&mut self, id: ElementId, z_index: &mut i32, visiting: &mut HashSet<ElementId>) {
        if !visiting.insert(id) {
            return;
        }
        if let Some(element) = self.elements.get_mut(&id) {
            element.as_element_mut().props_mut().z_index = *z_index;
        }
        *z_index = z_index.saturating_add(1);
        let children = self.group_child_ids(id);
        for child_id in children {
            self.write_z_dfs(child_id, z_index, visiting);
        }
    }

    fn normalize_members(&self, ids: &[ElementId]) -> Result<Vec<ElementId>, GroupError> {
        let mut members = Vec::new();
        for id in ids {
            if members.contains(id) {
                continue;
            }
            self.ensure_editable(*id)?;
            members.push(*id);
        }
        if members.len() < 2 {
            return Err(GroupError::NeedAtLeastTwo);
        }
        Ok(members)
    }

    fn ensure_known(&self, id: ElementId) -> Result<(), GroupError> {
        if self.elements.contains_key(&id) {
            Ok(())
        } else {
            Err(GroupError::UnknownElement(id))
        }
    }

    fn ensure_editable(&self, id: ElementId) -> Result<(), GroupError> {
        self.ensure_known(id)?;
        if let Some(locked_id) = self.lock_blocker(id) {
            Err(GroupError::Locked(locked_id))
        } else {
            Ok(())
        }
    }

    fn lock_blocker(&self, id: ElementId) -> Option<ElementId> {
        let mut current = Some(id);
        while let Some(element_id) = current {
            let element = self.elements.get(&element_id)?;
            if element.as_element().props().locked {
                return Some(element_id);
            }
            current = element.as_element().props().parent;
        }
        None
    }

    fn parent_id(&self, id: ElementId) -> Option<ElementId> {
        self.elements
            .get(&id)
            .and_then(|element| element.as_element().props().parent)
    }

    fn set_parent(&mut self, id: ElementId, parent: Option<ElementId>) {
        if let Some(element) = self.elements.get_mut(&id) {
            element.as_element_mut().props_mut().parent = parent;
        }
    }

    fn group_child_ids(&self, id: ElementId) -> Vec<ElementId> {
        self.elements
            .get(&id)
            .and_then(AnyElement::container_children)
            .map(|children| children.to_vec())
            .unwrap_or_default()
    }

    /// 组或画板的直接子元素。叶子返回 [`GroupError::NotAGroup`]。
    fn container_child_list(&self, id: ElementId) -> Result<Vec<ElementId>, GroupError> {
        match self
            .elements
            .get(&id)
            .and_then(AnyElement::container_children)
        {
            Some(children) => Ok(children.to_vec()),
            None if self.elements.contains_key(&id) => Err(GroupError::NotAGroup(id)),
            None => Err(GroupError::UnknownElement(id)),
        }
    }

    fn group_children(&self, id: ElementId) -> Result<Vec<ElementId>, GroupError> {
        match self.elements.get(&id) {
            Some(AnyElement::Group(group)) => Ok(group.children.clone()),
            Some(_) => Err(GroupError::NotAGroup(id)),
            None => Err(GroupError::UnknownElement(id)),
        }
    }

    fn child_index(&self, parent_id: ElementId, members: &[ElementId]) -> Option<usize> {
        self.group_child_ids(parent_id)
            .iter()
            .position(|id| members.contains(id))
    }

    fn insert_child(&mut self, group_id: ElementId, child_id: ElementId, index: usize) {
        if let Some(children) = self
            .elements
            .get_mut(&group_id)
            .and_then(AnyElement::container_children_mut)
        {
            if !children.contains(&child_id) {
                let index = index.min(children.len());
                children.insert(index, child_id);
            }
        }
    }

    fn detach_from_parent(&mut self, id: ElementId) {
        if let Some(parent_id) = self.parent_id(id) {
            if let Some(children) = self
                .elements
                .get_mut(&parent_id)
                .and_then(AnyElement::container_children_mut)
            {
                children.retain(|child| *child != id);
            }
        }
        self.set_parent(id, None);
    }

    fn member_contains_another(&self, id: ElementId, members: &[ElementId]) -> bool {
        let mut current = self.parent_id(id);
        while let Some(parent_id) = current {
            if members.contains(&parent_id) {
                return true;
            }
            current = self.parent_id(parent_id);
        }
        false
    }

    fn is_ancestor(&self, ancestor: ElementId, node: ElementId) -> bool {
        let mut current = self.parent_id(node);
        while let Some(parent_id) = current {
            if parent_id == ancestor {
                return true;
            }
            current = self.parent_id(parent_id);
        }
        false
    }

    fn has_ancestor_in(&self, id: ElementId, set: &HashSet<ElementId>) -> bool {
        let mut current = self.parent_id(id);
        while let Some(parent_id) = current {
            if set.contains(&parent_id) {
                return true;
            }
            current = self.parent_id(parent_id);
        }
        false
    }

    fn has_selected_descendant(&self, id: ElementId, set: &HashSet<ElementId>) -> bool {
        self.descendant_ids(id)
            .iter()
            .any(|descendant| set.contains(descendant))
    }

    fn descendant_ids(&self, id: ElementId) -> Vec<ElementId> {
        let mut ids = Vec::new();
        for child_id in self.group_child_ids(id) {
            self.collect_subtree(child_id, &mut ids);
        }
        ids
    }

    fn collect_subtree(&self, id: ElementId, out: &mut Vec<ElementId>) {
        if !self.elements.contains_key(&id) || out.contains(&id) {
            return;
        }
        out.push(id);
        for child_id in self.group_child_ids(id) {
            self.collect_subtree(child_id, out);
        }
    }

    fn alloc_id(&mut self) -> ElementId {
        while self.next_id == 0 || self.elements.contains_key(&self.next_id) {
            self.next_id = self.next_id.wrapping_add(1);
            if self.next_id == 0 {
                self.next_id = 1;
            }
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        id
    }

    /// 把元素和它的全部后代放进脏队列。
    fn mark_subtree_dirty(&mut self, id: ElementId) {
        let ids = self.subtree_ids(id);
        for element_id in ids {
            self.mark_dirty(element_id);
        }
    }

    fn write_transform(&mut self, id: ElementId, transform: Transform) {
        if let Some(element) = self.elements.get_mut(&id) {
            element.as_element_mut().props_mut().transform = transform;
        }
    }

    fn place_new_group_frame(&mut self, group_id: ElementId) {
        let children = self.group_child_ids(group_id);
        let center = self.union_center(&children);
        self.write_transform(group_id, Transform::from_translation(center));
        self.recompute_group_bounds(group_id);
    }

    fn union_center(&self, ids: &[ElementId]) -> Vec2 {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut any = false;
        for id in ids {
            let Some(element) = self.elements.get(id) else {
                continue;
            };
            let bounds = element.as_element().world_bounds();
            any = true;
            min_x = min_x.min(bounds.x);
            min_y = min_y.min(bounds.y);
            max_x = max_x.max(bounds.right());
            max_y = max_y.max(bounds.bottom());
        }
        if !any {
            return Vec2::ZERO;
        }
        Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5)
    }

    fn refresh_bounds_upwards(&mut self, start: ElementId) {
        let mut chain = Vec::new();
        let mut current = Some(start);
        while let Some(element_id) = current {
            chain.push(element_id);
            current = self.parent_id(element_id);
        }
        for element_id in chain {
            self.recompute_group_bounds(element_id);
        }
    }

    /// 用子元素的世界角点重算组的局部包围盒，并把组原点放到内容中心。
    /// 只改组自己的变换，不移动子元素。
    fn recompute_group_bounds(&mut self, group_id: ElementId) {
        let children = self.group_child_ids(group_id);
        if children.is_empty()
            && self
                .elements
                .get(&group_id)
                .and_then(AnyElement::as_group)
                .is_none()
        {
            return;
        }
        if self
            .elements
            .get(&group_id)
            .and_then(AnyElement::as_group)
            .is_none()
        {
            return;
        }
        let transform = self
            .elements
            .get(&group_id)
            .unwrap()
            .as_element()
            .props()
            .transform;
        if !transform.is_invertible() {
            return;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut any = false;
        for child_id in &children {
            for corner in self.world_corners(*child_id) {
                let local = transform.inverse_transform_point(corner);
                any = true;
                min_x = min_x.min(local.x);
                min_y = min_y.min(local.y);
                max_x = max_x.max(local.x);
                max_y = max_y.max(local.y);
            }
        }
        let (local_bounds, translation) = if any {
            let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            let width = (max_x - min_x).max(0.0);
            let height = (max_y - min_y).max(0.0);
            let bounds =
                infinitecanvas_math::Rect::new(min_x - center.x, min_y - center.y, width, height);
            let translation = if center.length_squared() > 1e-10 {
                transform.transform_point(center)
            } else {
                transform.translation
            };
            (bounds, translation)
        } else {
            (
                infinitecanvas_math::Rect::new(0.0, 0.0, 0.0, 0.0),
                transform.translation,
            )
        };
        if let Some(group) = self
            .elements
            .get_mut(&group_id)
            .and_then(AnyElement::as_group_mut)
        {
            group.local_bounds = local_bounds;
            group.props.transform.translation = translation;
        }
    }

    fn world_corners(&self, id: ElementId) -> [Vec2; 4] {
        let Some(element) = self.elements.get(&id) else {
            return [Vec2::ZERO; 4];
        };
        let local = element.as_element().local_bounds();
        let transform = element.as_element().props().transform;
        let min = local.min();
        let max = local.max();
        [
            transform.transform_point(min),
            transform.transform_point(Vec2::new(max.x, min.y)),
            transform.transform_point(max),
            transform.transform_point(Vec2::new(min.x, max.y)),
        ]
    }

    fn build_tree_node(
        &self,
        id: ElementId,
        visiting: &mut HashSet<ElementId>,
    ) -> Option<SceneTreeNode> {
        if !visiting.insert(id) {
            return None;
        }
        let props = self.elements.get(&id)?.as_element().props().clone();
        let element_type = self.elements.get(&id)?.as_element().type_name().to_string();
        let name = match self.elements.get(&id) {
            Some(AnyElement::Frame(frame)) => frame.name.clone(),
            _ => String::new(),
        };
        let parent_id = props.parent;
        let visible = props.visible;
        let locked = props.locked;
        let z_index = props.z_index;
        let children = self
            .group_child_ids(id)
            .into_iter()
            .filter_map(|child_id| self.build_tree_node(child_id, visiting))
            .collect();
        visiting.remove(&id);
        Some(SceneTreeNode {
            id,
            element_type,
            name,
            parent_id,
            visible,
            locked,
            z_index,
            children,
        })
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
    use crate::element::Group;
    use infinitecanvas_math::{Rect, Transform, Vec2};

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
    fn content_bounds_unions_every_element() {
        let mut scene = Scene::new();
        assert!(scene.content_bounds().is_none());

        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(100.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));

        let bounds = scene.content_bounds().unwrap();
        assert!((bounds.x - -5.0).abs() < 1e-3);
        assert!((bounds.y - -5.0).abs() < 1e-3);
        assert!((bounds.width - 110.0).abs() < 1e-3);
        assert!((bounds.height - 10.0).abs() < 1e-3);
    }

    #[test]
    fn content_bounds_skips_groups_and_hidden() {
        let mut scene = Scene::new();
        let left = scene.next_id();
        scene.add_rect(rect(left, Vec2::new(0.0, 0.0)));
        let right = scene.next_id();
        scene.add_rect(rect(right, Vec2::new(100.0, 0.0)));
        let group_id = scene.group(&[left, right]).unwrap();
        assert!(scene.contains(group_id));

        let hidden_id = scene.next_id();
        let mut hidden = rect(hidden_id, Vec2::new(500.0, 0.0));
        hidden.props.visible = false;
        scene.add_rect(hidden);

        let buried_a = scene.next_id();
        scene.add_rect(rect(buried_a, Vec2::new(-400.0, 0.0)));
        let buried_b = scene.next_id();
        scene.add_rect(rect(buried_b, Vec2::new(-380.0, 0.0)));
        let hidden_group = scene.group(&[buried_a, buried_b]).unwrap();
        assert!(scene.set_visible(hidden_group, false));

        let decoy_id = scene.next_id();
        let mut decoy = Group::new(decoy_id);
        decoy.props.transform = Transform::from_translation(Vec2::new(1000.0, 1000.0));
        decoy.local_bounds = Rect::new(-80.0, -80.0, 160.0, 160.0);
        scene.add(AnyElement::Group(decoy));

        let bounds = scene.content_bounds().unwrap();
        assert!((bounds.x - -5.0).abs() < 1e-3);
        assert!((bounds.y - -5.0).abs() < 1e-3);
        assert!((bounds.width - 110.0).abs() < 1e-3);
        assert!((bounds.height - 10.0).abs() < 1e-3);
    }

    #[test]
    fn test_next_id() {
        let mut scene = Scene::new();
        assert_eq!(scene.next_id(), 1);
        assert_eq!(scene.next_id(), 2);
        assert_eq!(scene.next_id(), 3);
    }

    fn rect(id: ElementId, center: Vec2) -> Rectangle {
        Rectangle::new(id, center, Vec2::new(10.0, 10.0))
    }

    fn translation(scene: &Scene, id: ElementId) -> Vec2 {
        scene
            .get(id)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation
    }

    fn approx(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() < 1e-3,
            "actual {actual:?} expected {expected:?}"
        );
    }

    #[test]
    fn group_then_ungroup_keeps_children_in_the_scene() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::new(0.0, 0.0)));
        scene.add_rect(rect(2, Vec2::new(100.0, 0.0)));
        assert!(matches!(scene.group(&[1]), Err(GroupError::NeedAtLeastTwo)));

        let group_id = scene.group(&[2, 1]).unwrap();
        assert_eq!(scene.count(), 3);
        assert_eq!(scene.children_of(group_id), vec![2, 1]);
        assert_eq!(scene.parent_of(1), Some(group_id));
        assert_eq!(scene.parent_of(2), Some(group_id));
        assert!(scene.parent_of(group_id).is_none());
        approx(translation(&scene, 1), Vec2::new(0.0, 0.0));
        approx(translation(&scene, 2), Vec2::new(100.0, 0.0));

        let children = scene.ungroup(group_id).unwrap();
        assert_eq!(children, vec![2, 1]);
        assert!(!scene.contains(group_id));
        assert!(scene.contains(1) && scene.contains(2));
        assert_eq!(scene.count(), 2);
        assert!(scene.parent_of(1).is_none());
        approx(translation(&scene, 2), Vec2::new(100.0, 0.0));
    }

    #[test]
    fn translating_a_group_updates_child_world_coordinates() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::new(0.0, 0.0)));
        scene.add_rect(rect(2, Vec2::new(100.0, 0.0)));
        let group_id = scene.group(&[1, 2]).unwrap();

        scene
            .translate_element(group_id, Vec2::new(15.0, -4.0))
            .unwrap();
        approx(translation(&scene, group_id), Vec2::new(65.0, -4.0));
        approx(translation(&scene, 1), Vec2::new(15.0, -4.0));
        approx(translation(&scene, 2), Vec2::new(115.0, -4.0));

        let point = Vec2::new(15.0, -4.0);
        assert_eq!(scene.find_at(point), Some(1));
        assert_eq!(
            crate::hit_test::hit_test_point(
                &scene,
                point,
                &crate::hit_test::HitTestOptions::default()
            )
            .first()
            .map(|hit| hit.id),
            Some(1)
        );
        assert!(scene.find_at(Vec2::ZERO).is_none());
    }

    #[test]
    fn rotating_and_scaling_a_group_keeps_children_in_the_group_frame() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(100.0, 0.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        let center = translation(&scene, group_id);
        approx(center, Vec2::new(50.0, 0.0));

        let mut rotated = Transform::from_translation(center);
        rotated.rotation = std::f32::consts::FRAC_PI_2;
        scene.set_world_transform(group_id, rotated).unwrap();
        approx(translation(&scene, 1), Vec2::new(50.0, -50.0));
        approx(translation(&scene, 2), Vec2::new(50.0, 50.0));
        let child_rotation = scene
            .get(2)
            .unwrap()
            .as_element()
            .props()
            .transform
            .rotation;
        assert!((child_rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-3);

        scene
            .set_world_transform(group_id, Transform::from_translation(center))
            .unwrap();
        approx(translation(&scene, 1), Vec2::ZERO);
        approx(translation(&scene, 2), Vec2::new(100.0, 0.0));

        let mut scaled = Transform::from_translation(center);
        scaled.scale = Vec2::splat(2.0);
        scene.set_world_transform(group_id, scaled).unwrap();
        approx(translation(&scene, 1), Vec2::new(-50.0, 0.0));
        approx(translation(&scene, 2), Vec2::new(150.0, 0.0));
    }

    #[test]
    fn scene_tree_walks_root_group_then_leaves() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(40.0, 0.0)));
        scene.add_circle(Circle::new(3, Vec2::new(200.0, 0.0), 8.0));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.set_locked(group_id, true);
        scene.set_visible(1, false);

        let tree = scene.scene_tree();
        assert_eq!(tree.roots.len(), 2);
        let group = tree.roots.iter().find(|node| node.id == group_id).unwrap();
        assert_eq!(group.element_type, "Group");
        assert!(group.parent_id.is_none());
        assert!(group.locked);
        assert!(group.visible);
        assert_eq!(group.children.len(), 2);
        assert_eq!(group.children[0].id, 1);
        assert_eq!(group.children[0].parent_id, Some(group_id));
        assert!(!group.children[0].visible);
        assert_eq!(group.children[1].id, 2);
        assert!(group.children[1].children.is_empty());
        assert!(tree
            .roots
            .iter()
            .any(|node| node.id == 3 && node.element_type == "Circle"));

        let json = serde_json::to_string(&tree).unwrap();
        let back: SceneTree = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tree);
    }

    #[test]
    fn element_props_without_hierarchy_fields_still_deserialize() {
        let json = r#"{
            "id": 4,
            "transform": {"translation": [1.0, 2.0], "rotation": 0.0, "scale": [1.0, 1.0]},
            "visible": true,
            "selected": false,
            "z_index": 3
        }"#;
        let props: crate::element::ElementProps = serde_json::from_str(json).unwrap();
        assert_eq!(props.id, 4);
        assert!(props.parent.is_none());
        assert!(!props.locked);
        assert_eq!(props.z_index, 3);
    }

    #[test]
    fn scene_json_round_trip_keeps_group_children_and_parent() {
        let mut scene = Scene::new();
        let left = scene.next_id();
        scene.add_rect(rect(left, Vec2::ZERO));
        let right = scene.next_id();
        scene.add_rect(rect(right, Vec2::new(20.0, 0.0)));
        let group_id = scene.group(&[left, right]).unwrap();
        scene
            .translate_element(group_id, Vec2::new(3.0, 4.0))
            .unwrap();
        scene.set_locked(group_id, true);

        let json = scene.to_json().unwrap();
        assert!(json.contains("\"type\":\"group\""));
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert_eq!(restored.children_of(group_id), vec![left, right]);
        assert_eq!(restored.parent_of(left), Some(group_id));
        assert!(restored.get(group_id).unwrap().as_element().props().locked);
        assert_eq!(
            restored
                .get(left)
                .unwrap()
                .as_element()
                .props()
                .transform
                .translation,
            Vec2::new(3.0, 4.0)
        );
        assert_eq!(restored.to_json().unwrap(), json);
    }

    #[test]
    fn enter_and_leave_group_preserve_world_position() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(30.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(80.0, 10.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.enter_group(3, group_id).unwrap();
        assert_eq!(scene.children_of(group_id), vec![1, 2, 3]);
        approx(translation(&scene, 3), Vec2::new(80.0, 10.0));

        scene.leave_group(3).unwrap();
        assert!(scene.parent_of(3).is_none());
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        approx(translation(&scene, 3), Vec2::new(80.0, 10.0));
        assert!(matches!(
            scene.leave_group(3),
            Err(GroupError::NotInGroup(3))
        ));
    }

    #[test]
    fn nested_group_translation_moves_grandchildren() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(20.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(80.0, 0.0)));
        let inner = scene.group(&[1, 2]).unwrap();
        let outer = scene.group(&[inner, 3]).unwrap();
        scene
            .translate_element(outer, Vec2::new(0.0, 10.0))
            .unwrap();
        approx(translation(&scene, 1), Vec2::new(0.0, 10.0));
        approx(translation(&scene, 2), Vec2::new(20.0, 10.0));
        approx(translation(&scene, 3), Vec2::new(80.0, 10.0));
        approx(translation(&scene, inner), Vec2::new(10.0, 10.0));
        let tree = scene.scene_tree();
        assert_eq!(tree.roots.len(), 1);
        assert_eq!(tree.roots[0].id, outer);
        assert_eq!(tree.roots[0].children[0].id, inner);
        assert_eq!(tree.roots[0].children[0].children.len(), 2);
    }

    #[test]
    fn hidden_group_is_not_hit_and_locked_group_does_not_move() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(40.0, 0.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.set_visible(group_id, false);
        assert!(scene.find_at(Vec2::ZERO).is_none());
        assert!(!scene.is_effectively_visible(1));

        scene.set_visible(group_id, true);
        scene.set_locked(group_id, true);
        assert!(matches!(
            scene.translate_element(group_id, Vec2::new(5.0, 0.0)),
            Err(GroupError::Locked(_))
        ));
        approx(translation(&scene, 1), Vec2::ZERO);
        assert!(matches!(
            scene.translate_element(1, Vec2::new(5.0, 0.0)),
            Err(GroupError::Locked(locked)) if locked == group_id
        ));
    }

    #[test]
    fn move_command_undo_restores_grouped_children() {
        use crate::command::{Command, MoveCommand};

        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::new(10.0, 10.0)));
        scene.add_rect(rect(2, Vec2::new(50.0, 10.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        let mut command = MoveCommand::new(vec![group_id], Vec2::new(8.0, -3.0));
        command.execute(&mut scene);
        approx(translation(&scene, 1), Vec2::new(18.0, 7.0));
        command.undo(&mut scene);
        approx(translation(&scene, 1), Vec2::new(10.0, 10.0));
        approx(translation(&scene, 2), Vec2::new(50.0, 10.0));
    }

    #[test]
    fn removing_a_group_removes_its_subtree() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(20.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(80.0, 0.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.remove(group_id);
        assert!(!scene.contains(group_id));
        assert!(!scene.contains(1));
        assert!(!scene.contains(2));
        assert!(scene.contains(3));
    }

    #[test]
    fn rejects_grouping_across_parents_or_into_a_cycle() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(20.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(60.0, 0.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        assert!(matches!(
            scene.group(&[1, 3]),
            Err(GroupError::DifferentParents)
        ));
        let other = scene.group(&[group_id, 3]).unwrap();
        assert!(matches!(
            scene.enter_group(other, group_id),
            Err(GroupError::WouldCycle)
        ));
    }

    fn paint_order(scene: &Scene) -> Vec<ElementId> {
        let mut ids: Vec<ElementId> = scene.iter().map(|element| element.id()).collect();
        ids.sort_by_key(|id| {
            let z_index = scene.get(*id).unwrap().as_element().props().z_index;
            (z_index, *id)
        });
        ids
    }

    #[test]
    fn reorder_siblings_changes_paint_order_without_moving_them() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(10.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(20.0, 0.0)));
        assert_eq!(paint_order(&scene), vec![1, 2, 3]);
        let epoch = scene.structure_epoch();

        assert!(scene.move_in_outline(1, None, 0).unwrap().is_none());
        assert_eq!(scene.structure_epoch(), epoch);

        scene.move_in_outline(1, None, 2).unwrap().unwrap();
        assert_eq!(paint_order(&scene), vec![2, 3, 1]);
        assert!(scene.structure_epoch() > epoch);
        approx(translation(&scene, 1), Vec2::ZERO);
    }

    #[test]
    fn arrange_siblings_rejects_a_partial_order() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(10.0, 0.0)));
        assert!(matches!(
            scene.arrange_siblings(None, &[1]),
            Err(GroupError::InvalidSiblingOrder)
        ));
        assert_eq!(scene.sibling_order(None), vec![1, 2]);
    }

    #[test]
    fn reparent_into_and_out_of_group_keeps_world_position() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(40.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(80.0, 10.0)));
        let group_id = scene.group(&[1, 2]).unwrap();
        let group_at = translation(&scene, group_id);

        scene
            .move_in_outline(3, Some(group_id), 0)
            .unwrap()
            .unwrap();
        assert_eq!(scene.parent_of(3), Some(group_id));
        assert_eq!(scene.children_of(group_id), vec![3, 1, 2]);
        approx(translation(&scene, 3), Vec2::new(80.0, 10.0));
        assert_eq!(paint_order(&scene).last().copied(), Some(2));

        scene.move_in_outline(3, None, 0).unwrap().unwrap();
        assert!(scene.parent_of(3).is_none());
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        approx(translation(&scene, 3), Vec2::new(80.0, 10.0));
        approx(translation(&scene, group_id), group_at);
        assert_eq!(paint_order(&scene).first().copied(), Some(3));
    }

    #[test]
    fn move_in_outline_rejects_cycles_and_locks() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(20.0, 0.0)));
        scene.add_rect(rect(3, Vec2::new(60.0, 0.0)));
        let inner = scene.group(&[1, 2]).unwrap();
        let outer = scene.group(&[inner, 3]).unwrap();
        assert!(matches!(
            scene.move_in_outline(outer, Some(inner), 0),
            Err(GroupError::WouldCycle)
        ));
        scene.set_locked(outer, true);
        assert!(matches!(
            scene.move_in_outline(3, None, 0),
            Err(GroupError::Locked(_))
        ));
        assert!(matches!(
            scene.move_in_outline(1, Some(outer), 0),
            Err(GroupError::Locked(_))
        ));
    }

    #[test]
    fn hidden_and_locked_elements_are_not_pickable() {
        let mut scene = Scene::new();
        scene.add_rect(rect(1, Vec2::ZERO));
        scene.add_rect(rect(2, Vec2::new(40.0, 0.0)));
        assert!(!scene.set_visible(99, false));
        scene.set_visible(1, false);
        assert!(scene.find_at(Vec2::ZERO).is_none());
        assert!(!scene.get(1).unwrap().as_element().props().visible);

        scene.set_visible(1, true);
        let group_id = scene.group(&[1, 2]).unwrap();
        scene.set_visible(group_id, false);
        assert!(scene.get(1).unwrap().as_element().props().visible);
        assert!(!scene.is_effectively_visible(1));
        assert!(scene.find_at(Vec2::ZERO).is_none());

        scene.set_visible(group_id, true);
        scene.set_locked(1, true);
        assert!(scene.is_effectively_locked(1));
        assert!(scene.find_at(Vec2::ZERO).is_none());
        scene.set_locked(1, false);
        scene.set_locked(group_id, true);
        assert!(scene.is_effectively_locked(2));
        assert!(scene.find_at(Vec2::new(40.0, 0.0)).is_none());
        assert!(!scene.get(2).unwrap().as_element().props().locked);
    }

    #[test]
    fn scene_tree_for_ten_thousand_elements_stays_responsive() {
        let mut scene = Scene::new();
        for index in 0..10_000 {
            scene.add_rect(Rectangle::new(
                index + 1,
                Vec2::new((index % 200) as f32, (index / 200) as f32),
                Vec2::new(1.0, 1.0),
            ));
        }
        let started = std::time::Instant::now();
        let tree = scene.scene_tree();
        let elapsed = started.elapsed();
        assert_eq!(tree.roots.len(), 10_000);
        assert!(
            elapsed.as_millis() < 2_000,
            "scene_tree 10k took {elapsed:?}"
        );
        eprintln!("scene_tree 10k: {elapsed:?}");
    }
}

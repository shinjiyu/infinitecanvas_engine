//! 选中形状的布尔运算。
//!
//! 至少两个未锁定、可见、带闭合填充轮廓的叶子才能运算。
//! 结果是一条 [`Path`]，替换这些叶子；撤销一次把它们放回原父节点。
//!
//! ```text
//! 选中叶子（矩形 / 圆 / 多边形 / 闭合路径）
//!        │  跳过锁定、隐藏、开口路径、线、文本、图片、组、画板
//!        ▼
//! 世界坐标多边形 ── i_overlay ── Union / Subtract / Intersect / XOR
//!        │
//!        ▼
//! 一条 Path（shapes：外轮廓 + 孔）
//!        │  样式和 z 取最上层操作数
//!        │  相减 = 最上层 − 其余（前 − 后）
//!        └─ 一条历史；Ctrl+Z 还原操作数
//! ```
//!
//! 叠放顺序是 `(z_index, id)`，较大者在上层。相减的被减数是最上层，
//! 其余操作数不论选中顺序，都从它里面挖去。并集、交集、异或与顺序无关。
//! 父节点沿用最上层操作数：同组或同画板的结果留在原父节点里。
//! 结果仍是叶子 [`Path`]，不新建元素种类，也不改画板尺寸。不重写其他元素的 `z_index`。

use crate::command::{Command, CommandHistory};
use crate::element::{ElementId, ElementProps, ElementStyle, FillShape, Path};
use crate::math::Vec2;
use crate::scene::RemovedSubtree;
use crate::{AnyElement, Scene};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::scale::FixedScaleFloatOverlay;
use i_overlay::float::single::SingleFloatOverlay;
use std::collections::HashSet;
use std::f32::consts::TAU;

/// 圆和椭圆采样段数。和圆形填充的分段同一量级。
const CURVE_SEGMENTS: usize = 48;
/// 浮点布尔网格：1 个整数单位 = 0.001 世界单位。
const OVERLAY_SCALE: f64 = 1000.0;
/// 环面积小于该值时丢掉，避免整数网格留下碎屑。
const MIN_AREA: f64 = 1.0e-4;
/// 闭合判断：首尾距离不超过该值才当作可填充路径。
const CLOSE_EPSILON: f32 = 1.0e-3;

type OverlayPoint = [f64; 2];
type OverlayShapes = Vec<Vec<Vec<OverlayPoint>>>;

/// 四种布尔运算。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::BooleanOp;
///
/// assert_eq!(BooleanOp::parse("Subtract"), Some(BooleanOp::Subtract));
/// assert_eq!(BooleanOp::Xor.as_str(), "xor");
/// assert!(BooleanOp::parse("bevel").is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    /// 并集。全部操作数的覆盖区域。
    Union,
    /// 相减。最上层减去其余操作数（前 − 后）。
    Subtract,
    /// 交集。只保留全部操作数共同覆盖的区域。
    Intersect,
    /// 异或。只保留恰好被奇数个操作数覆盖的区域。
    Xor,
}

impl BooleanOp {
    /// 解析运算名。
    ///
    /// 接受 `union`、`subtract` / `difference`、`intersect`、`xor` / `exclude`。
    /// 大小写不敏感，两端空白忽略。
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "union" | "add" => Some(Self::Union),
            "subtract" | "difference" | "minus" => Some(Self::Subtract),
            "intersect" | "intersection" => Some(Self::Intersect),
            "xor" | "exclude" => Some(Self::Xor),
            _ => None,
        }
    }

    /// 稳定短名，与 [`BooleanOp::parse`] 对应。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
            Self::Xor => "xor",
        }
    }
}

/// 一次布尔运算替换的组内子元素列表。
#[derive(Debug, Clone)]
struct GroupPatch {
    parent: ElementId,
    before: Vec<ElementId>,
    after: Vec<ElementId>,
}

/// 把当前选择做成布尔运算，并记入一条撤销记录。
///
/// 可运算的叶子少于 2 个时不入栈。锁定、隐藏、组、直线、文本、图片和开口路径会跳过，
/// 不计入这 2 个。结果是一条新的 [`Path`]，样式和 `z_index` 来自最上层操作数，
/// 并替换这些操作数。空选或单选返回 `None`。
///
/// # 参数
/// - `history`: 撤销栈
/// - `scene`: 要修改的场景
/// - `selected`: 当前选中的元素 id
/// - `op`: 运算
///
/// # 返回
/// 新路径的 id。没有执行时为 `None`。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     apply_boolean, BooleanOp, CommandHistory, Rectangle, Scene,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0)));
/// scene.add_rect(Rectangle::new(2, Vec2::new(10.0, 0.0), Vec2::new(20.0, 20.0)));
/// let mut history = CommandHistory::new();
/// let id = apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Union).unwrap();
/// assert!(scene.contains(id));
/// assert!(!scene.contains(1) && !scene.contains(2));
/// assert_eq!(history.undo_count(), 1);
/// assert!(history.undo(&mut scene));
/// assert!(scene.contains(1) && scene.contains(2));
/// assert!(!scene.contains(id));
/// ```
pub fn apply_boolean(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    op: BooleanOp,
) -> Option<ElementId> {
    let command = BooleanElementsCommand::from_selection(scene, selected, op)?;
    let id = command.result_id();
    history.execute(Box::new(command), scene);
    Some(id)
}

/// 当前选择里可以参与布尔运算的叶子，从底到顶。
///
/// 少于 2 个时运算不会入栈。顺序按 `(z_index, id)` 升序，最后一个是最上层。
///
/// # 参数
/// - `scene`: 当前场景
/// - `selected`: 当前选中的元素 id
///
/// # 返回
/// 可运算的元素 id。没有时为空。
pub fn boolean_targets(scene: &Scene, selected: &[ElementId]) -> Vec<ElementId> {
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for id in selected {
        if !seen.insert(*id) {
            continue;
        }
        if world_shapes(scene, *id).is_some() {
            ids.push(*id);
        }
    }
    ids.sort_by_key(|id| stack_key(scene, *id));
    ids
}

/// 一次布尔运算。执行时删掉操作数并插入结果路径，撤销时按原兄弟顺序放回。
#[derive(Debug)]
pub struct BooleanElementsCommand {
    op: BooleanOp,
    operands: Vec<RemovedSubtree>,
    result: Path,
    patches: Vec<GroupPatch>,
}

impl BooleanElementsCommand {
    /// 按当前选择构造一条尚未执行的布尔命令。
    ///
    /// 可运算叶子少于 2，或轮廓无法闭合时返回 `None`。会消耗一个 [`Scene::next_id`]。
    ///
    /// # 参数
    /// - `scene`: 当前场景
    /// - `selected`: 当前选中的元素 id
    /// - `op`: 运算
    pub fn from_selection(
        scene: &mut Scene,
        selected: &[ElementId],
        op: BooleanOp,
    ) -> Option<Self> {
        let operands = boolean_targets(scene, selected);
        if operands.len() < 2 {
            return None;
        }
        let shapes = operands
            .iter()
            .map(|id| world_shapes(scene, *id))
            .collect::<Option<Vec<_>>>()?;
        let merged = combine(&shapes, op);
        let front = *operands.last()?;
        let front_element = scene.get(front)?;
        let style = style_of(front_element);
        let z_index = front_element.as_element().props().z_index;
        let snapshots = operands
            .iter()
            .map(|id| scene.snapshot_subtree(*id))
            .collect::<Option<Vec<_>>>()?;
        // `next_id` 不跳过已经占用的 id。显式指定 id 的元素不能和结果撞号。
        let result_id = fresh_id(scene);
        let result = path_from_overlay(result_id, merged, style, z_index);
        let patches = group_patches(scene, &operands, front, result_id);
        Some(Self {
            op,
            operands: snapshots,
            result,
            patches,
        })
    }

    /// 结果路径的 id。执行前就可以拿来更新选择。
    pub fn result_id(&self) -> ElementId {
        self.result.props.id
    }

    fn operand_ids(&self) -> Vec<ElementId> {
        self.operands
            .iter()
            .filter_map(|snapshot| snapshot.nodes().first().map(AnyElement::id))
            .collect()
    }
}

impl Command for BooleanElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        for id in self.operand_ids() {
            scene.remove(id);
        }
        let result_id = self.result.props.id;
        scene.add(AnyElement::Path(self.result.clone()));
        for patch in &self.patches {
            let _ = scene.replace_group_children(patch.parent, &patch.after);
        }
        scene.mark_dirty(result_id);
    }

    fn undo(&mut self, scene: &mut Scene) {
        scene.remove(self.result.props.id);
        for snapshot in &self.operands {
            scene.restore_subtree(snapshot.clone());
        }
        for patch in &self.patches {
            let _ = scene.replace_group_children(patch.parent, &patch.before);
        }
    }

    fn name(&self) -> &'static str {
        match self.op {
            BooleanOp::Union => "BooleanUnion",
            BooleanOp::Subtract => "BooleanSubtract",
            BooleanOp::Intersect => "BooleanIntersect",
            BooleanOp::Xor => "BooleanXor",
        }
    }
}

/// 取一个场景里还没有的 id，并推进 `next_id`。
fn fresh_id(scene: &mut Scene) -> ElementId {
    loop {
        let id = scene.next_id();
        if id != 0 && !scene.contains(id) {
            return id;
        }
    }
}

fn stack_key(scene: &Scene, id: ElementId) -> (i32, ElementId) {
    let z = scene
        .get(id)
        .map(|element| element.as_element().props().z_index)
        .unwrap_or(0);
    (z, id)
}

fn style_of(element: &AnyElement) -> ElementStyle {
    match element {
        AnyElement::Rectangle(shape) => shape.style.clone(),
        AnyElement::Circle(shape) => shape.style.clone(),
        AnyElement::Polygon(shape) => shape.style.clone(),
        AnyElement::Path(shape) => shape.style.clone(),
        _ => ElementStyle::new(),
    }
}

fn group_patches(
    scene: &Scene,
    operands: &[ElementId],
    front: ElementId,
    result_id: ElementId,
) -> Vec<GroupPatch> {
    let operand_set: HashSet<ElementId> = operands.iter().copied().collect();
    let front_parent = scene.parent_of(front);
    let mut parents = Vec::new();
    for id in operands {
        if let Some(parent) = scene.parent_of(*id) {
            if !parents.contains(&parent) {
                parents.push(parent);
            }
        }
    }
    parents
        .into_iter()
        .map(|parent| {
            let before = scene.children_of(parent);
            let front_index = before.iter().position(|id| *id == front);
            let mut after: Vec<ElementId> = before
                .iter()
                .copied()
                .filter(|id| !operand_set.contains(id))
                .collect();
            if front_parent == Some(parent) {
                let insert_at = front_index
                    .map(|index| {
                        let removed_before = before
                            .iter()
                            .take(index)
                            .filter(|id| operand_set.contains(id))
                            .count();
                        index - removed_before
                    })
                    .unwrap_or(after.len());
                after.insert(insert_at.min(after.len()), result_id);
            }
            GroupPatch {
                parent,
                before,
                after,
            }
        })
        .collect()
}

fn world_shapes(scene: &Scene, id: ElementId) -> Option<OverlayShapes> {
    if !scene.is_effectively_visible(id) || scene.is_effectively_locked(id) {
        return None;
    }
    let element = scene.get(id)?;
    let shapes = match element {
        AnyElement::Rectangle(rect) => ring_shape(&rect.props.transform, &rect.local_outline()),
        AnyElement::Circle(circle) => {
            ring_shape(&circle.props.transform, &circle_local(circle.radius))
        }
        AnyElement::Polygon(polygon) => {
            ring_shape(&polygon.props.transform, &polygon.local_points())
        }
        AnyElement::Path(path) => path_shapes(path)?,
        AnyElement::Line(_)
        | AnyElement::Text(_)
        | AnyElement::Image(_)
        | AnyElement::Group(_)
        | AnyElement::Frame(_) => return None,
    };
    if shapes.is_empty() {
        None
    } else {
        Some(shapes)
    }
}

fn circle_local(radius: f32) -> Vec<Vec2> {
    let radius = radius.abs();
    if !radius.is_finite() || radius <= f32::EPSILON {
        return Vec::new();
    }
    (0..CURVE_SEGMENTS)
        .map(|index| {
            let angle = index as f32 / CURVE_SEGMENTS as f32 * TAU;
            Vec2::new(angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}

fn ring_shape(transform: &crate::math::Transform, local: &[Vec2]) -> OverlayShapes {
    let world: Vec<OverlayPoint> = local
        .iter()
        .map(|point| transform.transform_point(*point))
        .filter(|point| point.x.is_finite() && point.y.is_finite())
        .map(|point| [point.x as f64, point.y as f64])
        .collect();
    match clean_ring(world, true) {
        Some(ring) => vec![vec![ring]],
        None => Vec::new(),
    }
}

fn path_shapes(path: &Path) -> Option<OverlayShapes> {
    if path.is_filled() {
        let mut shapes = Vec::new();
        for shape in path.shapes() {
            let mut contours = Vec::new();
            for (index, ring) in shape.rings.iter().enumerate() {
                let world = ring
                    .iter()
                    .map(|point| path.props.transform.transform_point(*point))
                    .filter(|point| point.x.is_finite() && point.y.is_finite())
                    .map(|point| [point.x as f64, point.y as f64])
                    .collect();
                if let Some(cleaned) = clean_ring(world, index == 0) {
                    contours.push(cleaned);
                }
            }
            if contours.first().is_some_and(|ring| ring.len() >= 3) {
                shapes.push(contours);
            }
        }
        return if shapes.is_empty() {
            None
        } else {
            Some(shapes)
        };
    }
    let points = path.points();
    if points.len() < 4 {
        return None;
    }
    let first = points[0];
    let last = *points.last()?;
    if (first - last).length() > CLOSE_EPSILON {
        return None;
    }
    let world = points
        .iter()
        .map(|point| path.props.transform.transform_point(*point))
        .map(|point| [point.x as f64, point.y as f64])
        .collect();
    clean_ring(world, true).map(|ring| vec![vec![ring]])
}

fn clean_ring(mut ring: Vec<OverlayPoint>, positive: bool) -> Option<Vec<OverlayPoint>> {
    if ring.len() >= 2 && point_distance(ring[0], *ring.last().unwrap()) <= CLOSE_EPSILON as f64 {
        ring.pop();
    }
    ring.dedup_by(|a, b| point_distance(*a, *b) <= CLOSE_EPSILON as f64);
    if ring.len() >= 2 && point_distance(ring[0], *ring.last().unwrap()) <= CLOSE_EPSILON as f64 {
        ring.pop();
    }
    if ring.len() < 3 || signed_area(&ring).abs() < MIN_AREA {
        return None;
    }
    let area = signed_area(&ring);
    if positive && area < 0.0 || !positive && area > 0.0 {
        ring.reverse();
    }
    Some(ring)
}

fn point_distance(a: OverlayPoint, b: OverlayPoint) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx.hypot(dy)
}

fn signed_area(ring: &[OverlayPoint]) -> f64 {
    let mut sum = 0.0;
    for index in 0..ring.len() {
        let a = ring[index];
        let b = ring[(index + 1) % ring.len()];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum * 0.5
}

fn combine(parts: &[OverlayShapes], op: BooleanOp) -> OverlayShapes {
    match op {
        BooleanOp::Union => fold(parts, OverlayRule::Union),
        BooleanOp::Intersect => fold(parts, OverlayRule::Intersect),
        BooleanOp::Xor => fold(parts, OverlayRule::Xor),
        BooleanOp::Subtract => {
            let mut acc = parts.last().cloned().unwrap_or_default();
            for behind in parts.iter().rev().skip(1) {
                acc = overlay(&acc, behind, OverlayRule::Difference);
            }
            acc
        }
    }
}

fn fold(parts: &[OverlayShapes], rule: OverlayRule) -> OverlayShapes {
    let Some(first) = parts.first() else {
        return Vec::new();
    };
    let mut acc = first.clone();
    for next in parts.iter().skip(1) {
        acc = overlay(&acc, next, rule);
    }
    acc
}

fn overlay(subject: &OverlayShapes, clip: &OverlayShapes, rule: OverlayRule) -> OverlayShapes {
    match subject.overlay_with_fixed_scale_as::<i64>(clip, rule, FillRule::NonZero, OVERLAY_SCALE) {
        Ok(shapes) => shapes,
        Err(_) => subject.overlay_as::<i64>(clip, rule, FillRule::NonZero),
    }
}

fn path_from_overlay(
    id: ElementId,
    shapes: OverlayShapes,
    style: ElementStyle,
    z_index: i32,
) -> Path {
    let mut samples = Vec::new();
    for shape in &shapes {
        for ring in shape {
            for point in ring {
                if point[0].is_finite() && point[1].is_finite() {
                    samples.push(Vec2::new(point[0] as f32, point[1] as f32));
                }
            }
        }
    }
    let origin = bounds_center(&samples);
    let mut fill_shapes = Vec::new();
    for shape in shapes {
        let mut rings = Vec::new();
        for ring in shape {
            let local: Vec<Vec2> = ring
                .iter()
                .filter(|point| point[0].is_finite() && point[1].is_finite())
                .map(|point| Vec2::new(point[0] as f32, point[1] as f32) - origin)
                .collect();
            if local.len() >= 3 {
                rings.push(local);
            }
        }
        if rings.first().is_some_and(|ring| ring.len() >= 3) {
            fill_shapes.push(FillShape { rings });
        }
    }
    let mut props = ElementProps::new(id);
    props.transform.translation = origin;
    props.z_index = z_index;
    props.selected = true;
    Path {
        props,
        points: Vec::new(),
        style,
        shapes: fill_shapes,
        point_handles: Vec::new(),
    }
}

fn bounds_center(points: &[Vec2]) -> Vec2 {
    if points.is_empty() {
        return Vec2::ZERO;
    }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for point in points {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Circle, FillKind, GradientStop, Line, Polygon, Rectangle};
    use crate::math::{Color, Rect};
    use crate::{Element, ElementStyle};

    const EPS: f32 = 0.05;

    fn overlapping_rects() -> Scene {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );
        scene.add_rect(
            Rectangle::new(2, Vec2::new(10.0, 0.0), Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        scene
    }

    fn run(scene: &mut Scene, ids: &[ElementId], op: BooleanOp) -> Option<ElementId> {
        let mut history = CommandHistory::new();
        let id = apply_boolean(&mut history, scene, ids, op)?;
        assert_eq!(history.undo_count(), 1);
        Some(id)
    }

    fn path_of(scene: &Scene, id: ElementId) -> &Path {
        match scene.get(id) {
            Some(AnyElement::Path(path)) => path,
            other => panic!("expected path, got {other:?}"),
        }
    }

    fn assert_bounds(bounds: Rect, x: f32, y: f32, width: f32, height: f32) {
        assert!((bounds.x - x).abs() < EPS, "x {} != {x}", bounds.x);
        assert!((bounds.y - y).abs() < EPS, "y {} != {y}", bounds.y);
        assert!(
            (bounds.width - width).abs() < EPS,
            "width {} != {width}",
            bounds.width
        );
        assert!(
            (bounds.height - height).abs() < EPS,
            "height {} != {height}",
            bounds.height
        );
    }

    #[test]
    fn union_of_overlapping_rects_replaces_them_with_one_path() {
        let mut scene = overlapping_rects();
        let mut history = CommandHistory::new();
        let id = apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Union).unwrap();
        assert!(!scene.contains(1) && !scene.contains(2));
        assert_eq!(scene.count(), 1);
        let path = path_of(&scene, id);
        assert_eq!(path.style.fill, Some(Color::RED));
        assert!(path.is_filled());
        assert!(path.is_dirty() || scene.get(id).unwrap().as_element().is_dirty());
        assert!(!path.generate_vertices().is_empty());
        assert_bounds(path.world_bounds(), -10.0, -10.0, 30.0, 20.0);
        assert!(path.contains_point(Vec2::new(-5.0, 0.0)));
        assert!(path.contains_point(Vec2::new(5.0, 0.0)));
        assert!(path.contains_point(Vec2::new(15.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(40.0, 0.0)));
    }

    #[test]
    fn subtract_is_front_minus_back() {
        let mut scene = overlapping_rects();
        let id = run(&mut scene, &[2, 1], BooleanOp::Subtract).unwrap();
        let path = path_of(&scene, id);
        assert_eq!(path.style.fill, Some(Color::RED));
        assert_bounds(path.world_bounds(), 10.0, -10.0, 10.0, 20.0);
        assert!(path.contains_point(Vec2::new(15.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(5.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(-5.0, 0.0)));
    }

    #[test]
    fn subtract_uses_higher_z_index_as_front() {
        let mut scene = overlapping_rects();
        scene
            .get_mut(1)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 8;
        scene
            .get_mut(2)
            .unwrap()
            .as_element_mut()
            .props_mut()
            .z_index = 1;
        let id = run(&mut scene, &[1, 2], BooleanOp::Subtract).unwrap();
        let path = path_of(&scene, id);
        assert_eq!(path.style.fill, Some(Color::BLUE));
        assert_bounds(path.world_bounds(), -10.0, -10.0, 10.0, 20.0);
        assert!(path.contains_point(Vec2::new(-5.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(5.0, 0.0)));
    }

    #[test]
    fn intersect_keeps_only_the_overlap() {
        let mut scene = overlapping_rects();
        let id = run(&mut scene, &[1, 2], BooleanOp::Intersect).unwrap();
        let path = path_of(&scene, id);
        assert_bounds(path.world_bounds(), 0.0, -10.0, 10.0, 20.0);
        assert!(path.contains_point(Vec2::new(5.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(-5.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(15.0, 0.0)));
        assert!(!path.generate_vertices().is_empty());
    }

    #[test]
    fn xor_drops_the_overlap_and_keeps_both_wings() {
        let mut scene = overlapping_rects();
        let id = run(&mut scene, &[1, 2], BooleanOp::Xor).unwrap();
        let path = path_of(&scene, id);
        assert_bounds(path.world_bounds(), -10.0, -10.0, 30.0, 20.0);
        assert!(path.contains_point(Vec2::new(-5.0, 0.0)));
        assert!(path.contains_point(Vec2::new(15.0, 0.0)));
        assert!(!path.contains_point(Vec2::new(5.0, 0.0)));
    }

    #[test]
    fn undo_restores_operands_in_one_step_and_redo_replaces_them() {
        let mut scene = overlapping_rects();
        let mut history = CommandHistory::new();
        let id = apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Union).unwrap();
        assert_eq!(history.undo_count(), 1);
        assert!(history.undo(&mut scene));
        assert_eq!(history.undo_count(), 0);
        assert!(scene.contains(1) && scene.contains(2));
        assert!(!scene.contains(id));
        assert_bounds(
            scene.get(1).unwrap().as_element().world_bounds(),
            -10.0,
            -10.0,
            20.0,
            20.0,
        );
        assert_bounds(
            scene.get(2).unwrap().as_element().world_bounds(),
            0.0,
            -10.0,
            20.0,
            20.0,
        );
        assert!(history.redo(&mut scene));
        assert!(scene.contains(id));
        assert!(!scene.contains(1) && !scene.contains(2));
        assert!(!history.undo(&mut scene) || scene.contains(1));
        assert!(!history.undo(&mut scene));
    }

    #[test]
    fn empty_single_locked_hidden_or_unsupported_selection_is_noop() {
        let mut scene = overlapping_rects();
        let mut history = CommandHistory::new();
        assert!(apply_boolean(&mut history, &mut scene, &[], BooleanOp::Union).is_none());
        assert!(apply_boolean(&mut history, &mut scene, &[1], BooleanOp::Intersect).is_none());
        assert_eq!(history.undo_count(), 0);
        assert_eq!(scene.count(), 2);

        scene.set_locked(2, true);
        assert!(apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Union).is_none());
        scene.set_locked(2, false);
        scene.set_visible(1, false);
        assert!(apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Subtract).is_none());
        scene.set_visible(1, true);

        let line_id = 9;
        scene.add_line(Line::from_world(
            line_id,
            Vec2::ZERO,
            Vec2::new(30.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 2.0),
            false,
        ));
        assert!(apply_boolean(&mut history, &mut scene, &[line_id, 1], BooleanOp::Union).is_none());
        assert!(scene.contains(line_id));
        let id =
            apply_boolean(&mut history, &mut scene, &[line_id, 1, 2], BooleanOp::Union).unwrap();
        assert!(scene.contains(line_id));
        assert!(scene.contains(id));
        assert!(!scene.contains(1) && !scene.contains(2));
        assert_eq!(history.undo_count(), 1);
    }

    #[test]
    fn group_parent_survives_and_undo_restores_children() {
        let mut scene = overlapping_rects();
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(80.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2, 3]).unwrap();
        let sibling = scene
            .get(3)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation;
        let mut history = CommandHistory::new();
        let id = apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Union).unwrap();
        assert_eq!(scene.parent_of(id), Some(group_id));
        assert_eq!(scene.children_of(group_id), vec![id, 3]);
        assert_eq!(scene.parent_of(3), Some(group_id));
        assert_eq!(
            scene
                .get(3)
                .unwrap()
                .as_element()
                .props()
                .transform
                .translation,
            sibling
        );
        assert!(history.undo(&mut scene));
        assert_eq!(scene.children_of(group_id), vec![1, 2, 3]);
        assert_eq!(scene.parent_of(1), Some(group_id));
        assert_eq!(scene.parent_of(2), Some(group_id));
        assert!(!scene.contains(id));
    }

    #[test]
    fn union_inside_a_frame_stays_a_leaf_path_and_keeps_frame_size() {
        use crate::click_target;
        use crate::command::apply_reparent_into_frame;
        use crate::element::Frame;

        let mut scene = Scene::new();
        let frame_id =
            scene.add_frame(Frame::new(1, Vec2::ZERO, Vec2::new(200.0, 120.0)).with_name("看板"));
        scene.add_rect(
            Rectangle::new(2, Vec2::new(-10.0, 0.0), Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::BLUE)),
        );
        scene.add_rect(
            Rectangle::new(3, Vec2::new(10.0, 0.0), Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::RED)),
        );
        let mut history = CommandHistory::new();
        assert!(apply_reparent_into_frame(
            &mut history,
            &mut scene,
            &[2, 3],
            frame_id,
        ));
        assert!(apply_boolean(&mut history, &mut scene, &[frame_id], BooleanOp::Union).is_none());
        let id = apply_boolean(&mut history, &mut scene, &[2, 3], BooleanOp::Union).unwrap();
        assert_eq!(scene.parent_of(id), Some(frame_id));
        assert_eq!(scene.children_of(frame_id), vec![id]);
        assert!(!scene.contains(2) && !scene.contains(3));
        match scene.get(frame_id) {
            Some(AnyElement::Frame(frame)) => {
                assert_eq!(frame.name, "看板");
                assert_eq!(frame.size, Vec2::new(200.0, 120.0));
            }
            other => panic!("expected frame, got {other:?}"),
        }
        assert!(matches!(scene.get(id), Some(AnyElement::Path(_))));
        assert_eq!(click_target(&scene, Vec2::ZERO, false), Some(id));
        assert_eq!(click_target(&scene, Vec2::ZERO, true), Some(frame_id));
        assert!(history.undo(&mut scene));
        assert_eq!(scene.children_of(frame_id), vec![2, 3]);
        assert!(!scene.contains(id));
        match scene.get(frame_id) {
            Some(AnyElement::Frame(frame)) => {
                assert_eq!(frame.name, "看板");
                assert_eq!(frame.size, Vec2::new(200.0, 120.0));
            }
            other => panic!("expected frame, got {other:?}"),
        }
        assert_eq!(history.undo_count(), 1);
    }

    #[test]
    fn circle_and_polygon_are_operands_and_old_documents_still_load() {
        let mut scene = Scene::new();
        scene.add_circle(Circle::new(1, Vec2::ZERO, 10.0));
        scene.add_polygon(Polygon::hexagon(2, Vec2::new(8.0, 0.0), 12.0));
        let mut history = CommandHistory::new();
        let id = apply_boolean(&mut history, &mut scene, &[1, 2], BooleanOp::Intersect).unwrap();
        assert!(path_of(&scene, id).is_filled());
        assert!(history.undo(&mut scene));

        let mut plain = Scene::new();
        let plain_id = plain.next_id();
        plain.add_rect(Rectangle::new(
            plain_id,
            Vec2::new(4.0, 5.0),
            Vec2::new(6.0, 8.0),
        ));
        let json = plain.to_json().unwrap();
        assert_eq!(Scene::from_json(&json).unwrap(), plain);

        let legacy = r#"{"version":1,"elements":[{"type":"path","data":{"props":{"id":3}}}]}"#;
        let loaded = Scene::from_json(legacy).unwrap();
        match loaded.get(3).unwrap() {
            AnyElement::Path(path) => {
                assert!(path.points().is_empty());
                assert!(path.shapes().is_empty());
            }
            other => panic!("expected path, got {other:?}"),
        }
    }

    #[test]
    fn boolean_result_round_trips_and_exports_svg() {
        let mut scene = overlapping_rects();
        let id = run(&mut scene, &[1, 2], BooleanOp::Subtract).unwrap();
        let json = scene.to_json().unwrap();
        let restored = Scene::from_json(&json).unwrap();
        assert_eq!(restored, scene);
        assert!(restored
            .get(id)
            .unwrap()
            .as_element()
            .contains_point(Vec2::new(15.0, 0.0)));
        let svg = crate::export_svg(&scene, false);
        assert!(svg.contains("<path"), "{svg}");
        assert!(svg.contains("fill-rule=\"evenodd\""));
    }

    /// 渐变字段和 `shapes` 都是 `serde(default)`，布尔结果必须两套都留下。
    #[test]
    fn boolean_keeps_gradient_fill_next_to_path_shapes() {
        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(40.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::WHITE)),
        );
        let mut front = Rectangle::new(2, Vec2::new(10.0, 0.0), Vec2::new(40.0, 20.0));
        front.style.fill = Some(Color::WHITE);
        front.style.fill_kind = FillKind::Linear;
        front.style.fill_angle = 0.0;
        front.style.fill_stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            GradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ];
        scene.add_rect(front);
        let id = run(&mut scene, &[1, 2], BooleanOp::Union).unwrap();
        let path = path_of(&scene, id);
        assert_eq!(path.style.fill_kind, FillKind::Linear);
        assert_eq!(path.style.fill_stops.len(), 2);
        assert!(!path.shapes().is_empty());
        let json = scene.to_json().unwrap();
        assert!(json.contains("\"fill_kind\":\"linear\""), "{json}");
        assert!(json.contains("\"shapes\""), "{json}");
        assert_eq!(Scene::from_json(&json).unwrap(), scene);

        let svg = crate::export_svg(&scene, false);
        assert!(svg.contains("fill-rule=\"evenodd\""), "{svg}");
        assert!(svg.contains("<linearGradient"), "{svg}");

        let vertices = path.generate_vertices();
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.color[0] > 0.9 && vertex.color[2] < 0.1),
            "expected a red stop on the union"
        );
        assert!(
            vertices
                .iter()
                .any(|vertex| vertex.color[2] > 0.9 && vertex.color[0] < 0.1),
            "expected a blue stop on the union"
        );
        let left = path.color_at(Vec2::new(-18.0, 0.0)).expect("left sample");
        let right = path.color_at(Vec2::new(28.0, 0.0)).expect("right sample");
        assert!(left.r > 0.8 && left.b < 0.2, "{left:?}");
        assert!(right.b > 0.8 && right.r < 0.2, "{right:?}");

        let mut radial = Scene::new();
        radial.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(20.0, 20.0))
                .with_style(ElementStyle::fill_only(Color::BLACK)),
        );
        let mut top = Rectangle::new(2, Vec2::new(8.0, 0.0), Vec2::new(20.0, 20.0));
        top.style.fill_kind = FillKind::Radial;
        top.style.fill_radius = 12.0;
        top.style.fill_stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::GREEN,
            },
            GradientStop {
                offset: 1.0,
                color: Color::WHITE,
            },
        ];
        radial.add_rect(top);
        let radial_id = run(&mut radial, &[1, 2], BooleanOp::Union).unwrap();
        let radial_path = path_of(&radial, radial_id);
        assert_eq!(radial_path.style.fill_kind, FillKind::Radial);
        let center = radial_path
            .color_at(radial_path.props.transform.translation)
            .expect("radial center");
        let corner = radial_path
            .color_at(Vec2::new(-9.0, -9.0))
            .expect("radial corner");
        assert!(
            center.g > 0.8 && center.r < 0.2 && center.b < 0.2,
            "{center:?}"
        );
        assert!(corner.r > 0.8 && corner.g > 0.8, "{corner:?}");
        assert!(!radial_path.generate_vertices().is_empty());
    }
}

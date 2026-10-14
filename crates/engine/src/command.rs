//! 命令系统
//!
//! 实现命令模式，支持撤销/重做功能

use crate::element::{ElementId, TextVisual};
use crate::scene::OutlineSnapshot;
use crate::style::{
    apply_style_snapshot, capture_style, style_locked, style_target_ids, StylePatch, StyleSnapshot,
};
use crate::{math::Vec2, AnyElement, Scene};
use std::collections::{HashSet, VecDeque};

/// 方向键微调的默认步长，世界单位。
///
/// 与视口缩放无关：画布缩小后按一次方向键仍然移动 1 个画布单位。
/// 世界坐标 Y 轴向下，与屏幕一致（上为 -Y，下为 +Y）。
pub const NUDGE_STEP: f32 = 1.0;

/// 按住 Shift 时，步长相对 [`NUDGE_STEP`] 的倍数。
pub const NUDGE_SHIFT_FACTOR: f32 = 10.0;

/// 命令 Trait
///
/// 所有可撤销的操作都需要实现这个 trait
pub trait Command: std::fmt::Debug {
    /// 执行命令
    fn execute(&mut self, scene: &mut Scene);

    /// 撤销命令
    fn undo(&mut self, scene: &mut Scene);

    /// 获取命令名称（用于调试）
    fn name(&self) -> &'static str;
}

/// 移动元素命令
#[derive(Debug)]
pub struct MoveCommand {
    /// 被移动的元素 ID 列表
    element_ids: Vec<ElementId>,
    /// 移动量
    delta: Vec2,
}

impl MoveCommand {
    pub fn new(element_ids: Vec<ElementId>, delta: Vec2) -> Self {
        Self { element_ids, delta }
    }
}

impl Command for MoveCommand {
    fn execute(&mut self, scene: &mut Scene) {
        // 组的平移会带到子孙，并把子树标脏。列表里同时有组及其子孙时只移动一次。
        scene.translate_ids(&self.element_ids, self.delta);
    }

    fn undo(&mut self, scene: &mut Scene) {
        scene.translate_ids(&self.element_ids, -self.delta);
    }

    fn name(&self) -> &'static str {
        "MoveElements"
    }
}

/// 方向键。位移沿世界轴，不随元素自身的旋转改变。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeDirection {
    /// 世界 X 减小。
    Left,
    /// 世界 X 增大。
    Right,
    /// 世界 Y 减小（屏幕向上）。
    Up,
    /// 世界 Y 增大（屏幕向下）。
    Down,
}

impl NudgeDirection {
    /// 该方向上的世界位移。
    ///
    /// `shift` 为 true 时步长是 [`NUDGE_STEP`] × [`NUDGE_SHIFT_FACTOR`]，否则是 [`NUDGE_STEP`]。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::math::Vec2;
    /// use infinitecanvas_engine::NudgeDirection;
    ///
    /// assert_eq!(NudgeDirection::Right.delta(false), Vec2::new(1.0, 0.0));
    /// assert_eq!(NudgeDirection::Up.delta(true), Vec2::new(0.0, -10.0));
    /// ```
    pub fn delta(self, shift: bool) -> Vec2 {
        let step = if shift {
            NUDGE_STEP * NUDGE_SHIFT_FACTOR
        } else {
            NUDGE_STEP
        };
        match self {
            Self::Left => Vec2::new(-step, 0.0),
            Self::Right => Vec2::new(step, 0.0),
            Self::Up => Vec2::new(0.0, -step),
            Self::Down => Vec2::new(0.0, step),
        }
    }

    /// 解析方向键名称。
    ///
    /// 接受 `left` / `right` / `up` / `down`，以及 DOM 的 `ArrowLeft` 等 `key`。
    /// 大小写不敏感，两端空白忽略。无法识别时返回 `None`。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::NudgeDirection;
    ///
    /// assert_eq!(NudgeDirection::parse("ArrowDown"), Some(NudgeDirection::Down));
    /// assert!(NudgeDirection::parse("v").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "left" | "arrowleft" => Some(Self::Left),
            "right" | "arrowright" => Some(Self::Right),
            "up" | "arrowup" => Some(Self::Up),
            "down" | "arrowdown" => Some(Self::Down),
            _ => None,
        }
    }
}

/// 从当前选中里挑出方向键要平移的元素。
///
/// - 不在场景里、不可见（含被隐藏的祖先挡住）、或已锁定（含被锁定的祖先挡住）的 id 去掉
/// - 选中组时保留组根。组的子孙若也在这批目标里则去掉，避免同一棵子树移动两次
///
/// 结果按 id 升序，方便测试和撤销记录稳定。
///
/// # 参数
/// - `scene`: 当前场景
/// - `selected_ids`: 当前选中的元素
///
/// # 返回
/// 要平移的元素 id。没有可移动目标时为空。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::command::nudge_targets;
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{Rectangle, Scene};
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0)));
/// scene.set_locked(1, true);
/// assert!(nudge_targets(&scene, &[1]).is_empty());
/// ```
pub fn nudge_targets(scene: &Scene, selected_ids: &[ElementId]) -> Vec<ElementId> {
    let selected: HashSet<ElementId> = selected_ids
        .iter()
        .copied()
        .filter(|id| {
            scene.contains(*id)
                && scene.is_effectively_visible(*id)
                && !scene.is_effectively_locked(*id)
        })
        .collect();
    let mut ids: Vec<ElementId> = selected
        .iter()
        .copied()
        .filter(|id| !ancestor_in(scene, *id, &selected))
        .collect();
    ids.sort_unstable();
    ids
}

/// `id` 的某个祖先是否也在 `set` 里。
fn ancestor_in(scene: &Scene, id: ElementId, set: &HashSet<ElementId>) -> bool {
    let mut current = scene.parent_of(id);
    while let Some(parent) = current {
        if set.contains(&parent) {
            return true;
        }
        current = scene.parent_of(parent);
    }
    false
}

/// 方向键平移。只改平移分量，组根会带动子孙，并标脏子树。
///
/// 拖拽吸附不经过这条命令。多选的全部目标放在同一条历史里。
#[derive(Debug)]
pub struct NudgeElementsCommand {
    ids: Vec<ElementId>,
    delta: Vec2,
}

impl NudgeElementsCommand {
    /// 按当前选择构造一条尚未执行的平移命令。
    ///
    /// 没有可移动目标时返回 `None`，调用方不应入栈。
    ///
    /// # 参数
    /// - `scene`: 当前场景
    /// - `selected`: 当前选中的元素 id
    /// - `direction`: 方向键
    /// - `shift`: 是否按住 Shift（步长 × [`NUDGE_SHIFT_FACTOR`]）
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{NudgeDirection, NudgeElementsCommand, Scene};
    ///
    /// let scene = Scene::new();
    /// assert!(NudgeElementsCommand::from_selection(
    ///     &scene,
    ///     &[],
    ///     NudgeDirection::Left,
    ///     false,
    /// )
    /// .is_none());
    /// ```
    pub fn from_selection(
        scene: &Scene,
        selected: &[ElementId],
        direction: NudgeDirection,
        shift: bool,
    ) -> Option<Self> {
        let ids = nudge_targets(scene, selected);
        if ids.is_empty() {
            return None;
        }
        Some(Self {
            ids,
            delta: direction.delta(shift),
        })
    }
}

impl Command for NudgeElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        // 与拖拽共用 `translate_ids`：只加平移，选中组时子孙跟着走，并标脏。
        scene.translate_ids(&self.ids, self.delta);
    }

    fn undo(&mut self, scene: &mut Scene) {
        scene.translate_ids(&self.ids, -self.delta);
    }

    fn name(&self) -> &'static str {
        "NudgeElements"
    }
}

/// 对当前选择执行一次方向键微调，并记入一条撤销记录。
///
/// 空选、全部锁定、全部不可见，或没有可移动目标时不入栈，返回 `false`。
///
/// # 参数
/// - `history`: 撤销栈
/// - `scene`: 要修改的场景
/// - `selected`: 当前选中 id
/// - `direction`: 方向键
/// - `shift`: 为 true 时步长是 [`NUDGE_STEP`] 的 [`NUDGE_SHIFT_FACTOR`] 倍
///
/// # 返回
/// 是否写入了撤销栈
///
/// # 示例
/// ```
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{
///     apply_nudge, CommandHistory, Element, NudgeDirection, Rectangle, Scene,
/// };
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
/// let mut history = CommandHistory::new();
/// assert!(apply_nudge(
///     &mut history,
///     &mut scene,
///     &[1],
///     NudgeDirection::Right,
///     false,
/// ));
/// assert_eq!(
///     scene
///         .get(1)
///         .unwrap()
///         .as_element()
///         .props()
///         .transform
///         .translation,
///     Vec2::new(1.0, 0.0)
/// );
/// assert!(history.undo(&mut scene));
/// assert_eq!(
///     scene
///         .get(1)
///         .unwrap()
///         .as_element()
///         .props()
///         .transform
///         .translation,
///     Vec2::ZERO
/// );
/// ```
pub fn apply_nudge(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    direction: NudgeDirection,
    shift: bool,
) -> bool {
    let Some(command) = NudgeElementsCommand::from_selection(scene, selected, direction, shift)
    else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

/// 添加元素命令
#[derive(Debug)]
pub struct AddElementCommand {
    /// 要添加的元素（执行后为 None）
    element: Option<AnyElement>,
    /// 元素 ID（用于撤销时删除）
    element_id: ElementId,
    /// 撤销后暂存的图片资源。重做时放回场景。
    parked: Vec<crate::asset::ImageAsset>,
}

impl AddElementCommand {
    pub fn new(element: AnyElement) -> Self {
        let element_id = element.id();
        Self {
            element: Some(element),
            element_id,
            parked: Vec::new(),
        }
    }
}

impl Command for AddElementCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if let Some(element) = self.element.take() {
            scene.restore_assets(std::mem::take(&mut self.parked));
            scene.add(element);
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        if let Some(element) = scene.remove(self.element_id) {
            self.parked = parked_asset_for(scene, &element);
            self.element = Some(element);
        }
    }

    fn name(&self) -> &'static str {
        "AddElement"
    }
}

/// 删除元素命令
#[derive(Debug)]
pub struct RemoveElementCommand {
    /// 被删除的元素（撤销时恢复）
    element: Option<AnyElement>,
    /// 元素 ID
    element_id: ElementId,
    /// 删除后暂存的图片资源。撤销时放回场景。
    parked: Vec<crate::asset::ImageAsset>,
}

impl RemoveElementCommand {
    pub fn new(element_id: ElementId) -> Self {
        Self {
            element: None,
            element_id,
            parked: Vec::new(),
        }
    }
}

impl Command for RemoveElementCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if let Some(element) = scene.remove(self.element_id) {
            self.parked = parked_asset_for(scene, &element);
            self.element = Some(element);
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        if let Some(element) = self.element.take() {
            scene.restore_assets(std::mem::take(&mut self.parked));
            scene.add(element);
        }
    }

    fn name(&self) -> &'static str {
        "RemoveElement"
    }
}

/// 缩放 / 旋转元素。
///
/// 记录每个元素变换前、变换后的快照。拖动手柄时变换已经写进场景，
/// 因此用 [`CommandHistory::push_executed`] 入栈，撤销时恢复 before。
#[derive(Debug)]
pub struct TransformElementsCommand {
    before: Vec<(ElementId, crate::math::Transform)>,
    after: Vec<(ElementId, crate::math::Transform)>,
}

impl TransformElementsCommand {
    /// 创建变换命令。
    ///
    /// `before` 与 `after` 按相同的元素顺序排列。缺失的元素会被跳过。
    pub fn new(
        before: Vec<(ElementId, crate::math::Transform)>,
        after: Vec<(ElementId, crate::math::Transform)>,
    ) -> Self {
        Self { before, after }
    }

    fn apply(scene: &mut Scene, states: &[(ElementId, crate::math::Transform)]) {
        // 组变换通过 Scene 传播到子孙，并标脏需要重生成的顶点。
        scene.apply_world_transforms(states);
    }
}

impl Command for TransformElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        Self::apply(scene, &self.after);
    }

    fn undo(&mut self, scene: &mut Scene) {
        Self::apply(scene, &self.before);
    }

    fn name(&self) -> &'static str {
        "TransformElements"
    }
}

/// 修改文本外观。
///
/// 执行和撤销都会丢掉 GPU 纹理引用，并 [`Scene::mark_dirty`]，
/// 下一帧按当前缩放重新光栅化。这样撤销不会指向已经释放的纹理。
#[derive(Debug)]
pub struct UpdateTextCommand {
    /// 文本元素 ID。
    element_id: ElementId,
    /// 修改前的外观。
    before: TextVisual,
    /// 修改后的外观。
    after: TextVisual,
}

impl UpdateTextCommand {
    /// 记录一次已经发生或即将发生的文本修改。
    pub fn new(element_id: ElementId, before: TextVisual, after: TextVisual) -> Self {
        Self {
            element_id,
            before,
            after,
        }
    }
}

impl Command for UpdateTextCommand {
    fn execute(&mut self, scene: &mut Scene) {
        apply_text_visual(scene, self.element_id, &self.after);
    }

    fn undo(&mut self, scene: &mut Scene) {
        apply_text_visual(scene, self.element_id, &self.before);
    }

    fn name(&self) -> &'static str {
        "UpdateText"
    }
}

/// 调整大纲顺序：同一父节点下改绘制顺序，或移入 / 移出组。
///
/// 执行前会记下 [`OutlineSnapshot`]。撤销写回父指针、子元素顺序和 `z_index`，
/// 世界坐标保持不动。拖拽已经落在场景里时用 [`ReorderOutlineCommand::applied`]，
/// 再 [`CommandHistory::push_executed`]。
#[derive(Debug)]
pub struct ReorderOutlineCommand {
    id: ElementId,
    new_parent: Option<ElementId>,
    index: usize,
    before: Option<OutlineSnapshot>,
    refresh: Vec<ElementId>,
}

impl ReorderOutlineCommand {
    /// 创建一条尚未执行的重排命令。
    ///
    /// # 参数
    /// - `id`: 被移动的元素
    /// - `new_parent`: 新的父组。`None` 表示根
    /// - `index`: 摘下之后在目标兄弟列表中的下标，`0` 为最底层
    pub fn new(id: ElementId, new_parent: Option<ElementId>, index: usize) -> Self {
        Self {
            id,
            new_parent,
            index,
            before: None,
            refresh: Vec::new(),
        }
    }

    /// 移动已经写入场景时，用移动前的快照入栈。
    ///
    /// # 参数
    /// - `before`: [`Scene::outline_snapshot`] 在移动之前的结果
    /// - `refresh`: [`Scene::move_in_outline`] 返回的、需要重算包围盒的组
    pub fn applied(
        id: ElementId,
        new_parent: Option<ElementId>,
        index: usize,
        before: OutlineSnapshot,
        refresh: Vec<ElementId>,
    ) -> Self {
        Self {
            id,
            new_parent,
            index,
            before: Some(before),
            refresh,
        }
    }
}

impl Command for ReorderOutlineCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if self.before.is_none() {
            self.before = Some(scene.outline_snapshot());
        }
        if let Ok(Some(refresh)) = scene.move_in_outline(self.id, self.new_parent, self.index) {
            self.refresh = refresh;
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        if let Some(before) = &self.before {
            scene.restore_outline(before, &self.refresh);
        }
    }

    fn name(&self) -> &'static str {
        "ReorderOutline"
    }
}

/// 选中元素在同一层兄弟之间的 z 序动作。
///
/// 下标 `0` 是最底层。`Front` / `Back` 把选中块整组放到最前或最后，并保持它们原来的相对顺序。
/// `Forward` / `Backward` 只跨过一个未选中的兄弟；挨在一起的选中元素会作为一块移动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackOrderAction {
    /// 置顶。
    Front,
    /// 置底。
    Back,
    /// 前移一层。
    Forward,
    /// 后移一层。
    Backward,
}

impl StackOrderAction {
    /// 解析 `reorder_selection` 的动作名。
    ///
    /// 接受 `front`、`back`、`forward`、`backward`。大小写不敏感，两端空白忽略。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::StackOrderAction;
    ///
    /// assert_eq!(StackOrderAction::parse("Front"), Some(StackOrderAction::Front));
    /// assert!(StackOrderAction::parse("up").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "front" => Some(Self::Front),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            "backward" => Some(Self::Backward),
            _ => None,
        }
    }
}

/// 对当前选中做一次同层重排，整次操作是一条历史。
///
/// 只在同一个父节点下改兄弟顺序。选中组时移动组根，不改组内子元素顺序。
/// 执行前记下 [`OutlineSnapshot`]，撤销时写回父指针、子列表和 `z_index`。
/// 世界坐标不动。绘制顺序靠 [`Scene::structure_epoch`] 让顶点缓存整表重排，
/// 不把元素标成几何脏。
#[derive(Debug)]
pub struct ReorderSelectionCommand {
    action: StackOrderAction,
    parent: Option<ElementId>,
    order: Vec<ElementId>,
    before: Option<OutlineSnapshot>,
}

impl ReorderSelectionCommand {
    /// 按当前选择构造一条尚未执行的重排命令。
    ///
    /// 空选、跨父、全部被锁定（含锁定的祖先），或顺序不会变化时返回 `None`。
    /// 同一层里被锁定的 id 会跳过，其余未锁定的兄弟照常重排。
    /// 隐藏（含被隐藏的祖先挡住）不阻止重排，和图层面板拖拽一致。
    ///
    /// # 参数
    /// - `scene`: 当前场景
    /// - `selected_ids`: 当前选中的元素。重复 id 只算一次
    /// - `action`: 置顶、置底、前移或后移
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_engine::{Rectangle, ReorderSelectionCommand, Scene, StackOrderAction};
    /// use infinitecanvas_math::Vec2;
    ///
    /// let mut scene = Scene::new();
    /// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0)));
    /// scene.add_rect(Rectangle::new(2, Vec2::new(10.0, 0.0), Vec2::new(4.0, 4.0)));
    /// assert!(ReorderSelectionCommand::from_selection(
    ///     &scene,
    ///     &[1],
    ///     StackOrderAction::Front,
    /// )
    /// .is_some());
    /// assert!(ReorderSelectionCommand::from_selection(&scene, &[], StackOrderAction::Back).is_none());
    /// ```
    pub fn from_selection(
        scene: &Scene,
        selected_ids: &[ElementId],
        action: StackOrderAction,
    ) -> Option<Self> {
        let (parent, order) = plan_stack_order(scene, selected_ids, action)?;
        Some(Self {
            action,
            parent,
            order,
            before: None,
        })
    }
}

impl Command for ReorderSelectionCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if self.before.is_none() {
            self.before = Some(scene.outline_snapshot());
        }
        let _ = scene.arrange_siblings(self.parent, &self.order);
    }

    fn undo(&mut self, scene: &mut Scene) {
        if let Some(before) = &self.before {
            scene.restore_outline(before, &[]);
        }
    }

    fn name(&self) -> &'static str {
        match self.action {
            StackOrderAction::Front => "ReorderSelectionFront",
            StackOrderAction::Back => "ReorderSelectionBack",
            StackOrderAction::Forward => "ReorderSelectionForward",
            StackOrderAction::Backward => "ReorderSelectionBackward",
        }
    }
}

/// 把选中元素按 [`StackOrderAction`] 重排，并压入一条撤销记录。
///
/// 没有产生顺序变化时不入栈，返回 `false`。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{
///     apply_stack_reorder, CommandHistory, Rectangle, Scene, StackOrderAction,
/// };
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
/// scene.add_rect(Rectangle::new(2, Vec2::new(20.0, 0.0), Vec2::new(10.0, 10.0)));
/// scene.add_rect(Rectangle::new(3, Vec2::new(40.0, 0.0), Vec2::new(10.0, 10.0)));
/// let mut history = CommandHistory::new();
/// assert!(apply_stack_reorder(
///     &mut history,
///     &mut scene,
///     &[1],
///     StackOrderAction::Front,
/// ));
/// assert_eq!(scene.sibling_order(None), vec![2, 3, 1]);
/// assert!(history.undo(&mut scene));
/// assert_eq!(scene.sibling_order(None), vec![1, 2, 3]);
/// ```
pub fn apply_stack_reorder(
    history: &mut CommandHistory,
    scene: &mut Scene,
    selected: &[ElementId],
    action: StackOrderAction,
) -> bool {
    let Some(command) = ReorderSelectionCommand::from_selection(scene, selected, action) else {
        return false;
    };
    history.execute(Box::new(command), scene);
    true
}

/// 算出重排后的整层兄弟顺序。不会改场景。
fn plan_stack_order(
    scene: &Scene,
    selected_ids: &[ElementId],
    action: StackOrderAction,
) -> Option<(Option<ElementId>, Vec<ElementId>)> {
    let mut known = Vec::new();
    for id in selected_ids {
        if scene.contains(*id) && !known.contains(id) {
            known.push(*id);
        }
    }
    if known.is_empty() {
        return None;
    }
    let parent = scene.parent_of(known[0]);
    if known.iter().any(|id| scene.parent_of(*id) != parent) {
        return None;
    }
    known.retain(|id| !scene.is_effectively_locked(*id));
    if known.is_empty() {
        return None;
    }
    let siblings = scene.sibling_order(parent);
    let selected: HashSet<ElementId> = known.into_iter().collect();
    if selected.iter().any(|id| !siblings.contains(id)) {
        return None;
    }
    let order = match action {
        StackOrderAction::Front => move_selected(&siblings, &selected, StackEnd::Front),
        StackOrderAction::Back => move_selected(&siblings, &selected, StackEnd::Back),
        StackOrderAction::Forward => step_selected(&siblings, &selected, StepWay::Forward),
        StackOrderAction::Backward => step_selected(&siblings, &selected, StepWay::Backward),
    };
    if order == siblings {
        None
    } else {
        Some((parent, order))
    }
}

#[derive(Clone, Copy)]
enum StackEnd {
    Front,
    Back,
}

#[derive(Clone, Copy)]
enum StepWay {
    Forward,
    Backward,
}

/// 保持选中元素的相对顺序，整组放到最前或最后。
fn move_selected(
    siblings: &[ElementId],
    selected: &HashSet<ElementId>,
    end: StackEnd,
) -> Vec<ElementId> {
    let mut rest = Vec::with_capacity(siblings.len());
    let mut picked = Vec::with_capacity(selected.len());
    for id in siblings {
        if selected.contains(id) {
            picked.push(*id);
        } else {
            rest.push(*id);
        }
    }
    match end {
        StackEnd::Front => {
            rest.extend(picked);
            rest
        }
        StackEnd::Back => {
            picked.extend(rest);
            picked
        }
    }
}

/// 每个选中元素只跨过一个未选中兄弟。从移动方向的远端开始交换，避免互相挡住。
fn step_selected(
    siblings: &[ElementId],
    selected: &HashSet<ElementId>,
    way: StepWay,
) -> Vec<ElementId> {
    let mut order = siblings.to_vec();
    match way {
        StepWay::Forward => {
            let mut index = order.len();
            while index > 1 {
                index -= 1;
                if selected.contains(&order[index - 1]) && !selected.contains(&order[index]) {
                    order.swap(index - 1, index);
                }
            }
        }
        StepWay::Backward => {
            for index in 0..order.len().saturating_sub(1) {
                if !selected.contains(&order[index]) && selected.contains(&order[index + 1]) {
                    order.swap(index, index + 1);
                }
            }
        }
    }
    order
}

/// 原地克隆一批元素。
///
/// 第一次执行调用 [`Scene::duplicate`]。撤销删掉整棵新子树，重做按同样的 ID 插回，
/// 因此重做不会再分配一套新 ID。新的根会被标成选中，锁定的根除外。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{CommandHistory, DuplicateElementsCommand, Rectangle, Scene};
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0)));
/// let mut history = CommandHistory::new();
/// history.execute(
///     Box::new(DuplicateElementsCommand::new(vec![1], Vec2::new(10.0, 10.0))),
///     &mut scene,
/// );
/// assert_eq!(scene.count(), 2);
/// history.undo(&mut scene);
/// assert_eq!(scene.count(), 1);
/// history.redo(&mut scene);
/// assert_eq!(scene.count(), 2);
/// ```
#[derive(Debug)]
pub struct DuplicateElementsCommand {
    source_ids: Vec<ElementId>,
    offset: Vec2,
    started: bool,
    batch: SpawnBatch,
}

impl DuplicateElementsCommand {
    /// 克隆 `source_ids`，并把每个副本的世界平移加上 `offset`。
    ///
    /// # 参数
    /// - `source_ids`: 要克隆的元素。祖先也在列表里时，子孙不会再克隆一次
    /// - `offset`: 世界位移。Demo 的 Ctrl+D 传入 `(10, 10)`
    pub fn new(source_ids: Vec<ElementId>, offset: Vec2) -> Self {
        Self {
            source_ids,
            offset,
            started: false,
            batch: SpawnBatch::default(),
        }
    }

    /// 克隆已经写入场景时使用，再 [`CommandHistory::push_executed`]。
    ///
    /// 调用前把要选中的根标上 `selected`，快照会连同这个标记一起保存。
    ///
    /// # 参数
    /// - `roots`: [`Scene::duplicate`](crate::Scene::duplicate) 返回的新根
    pub fn applied(
        source_ids: Vec<ElementId>,
        offset: Vec2,
        roots: Vec<ElementId>,
        scene: &Scene,
    ) -> Self {
        Self {
            source_ids,
            offset,
            started: true,
            batch: capture_spawn(scene, &roots),
        }
    }
}

impl Command for DuplicateElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if !self.started {
            self.started = true;
            let roots = scene.duplicate(&self.source_ids, self.offset);
            select_spawn_roots(scene, &roots);
            self.batch = capture_spawn(scene, &roots);
            return;
        }
        replay_spawn(scene, &mut self.batch);
    }

    fn undo(&mut self, scene: &mut Scene) {
        remove_spawn(scene, &mut self.batch);
    }

    fn name(&self) -> &'static str {
        "DuplicateElements"
    }
}

/// 从版本化 JSON 粘贴元素。
///
/// 与 [`DuplicateElementsCommand`] 一样，撤销删除新元素，重做恢复同一批 ID。
/// `focus` 是叶子包围盒中心要落到的世界坐标；连续粘贴的额外位移放在 `offset` 里。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{CommandHistory, PasteElementsCommand, Rectangle, Scene};
/// use infinitecanvas_math::Vec2;
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(4.0, 4.0)));
/// let json = scene.copy_elements_json(&[1]).unwrap();
/// let mut history = CommandHistory::new();
/// history.execute(
///     Box::new(PasteElementsCommand::new(json, Vec2::new(10.0, 10.0), None)),
///     &mut scene,
/// );
/// assert_eq!(scene.count(), 2);
/// history.undo(&mut scene);
/// assert_eq!(scene.count(), 1);
/// ```
#[derive(Debug)]
pub struct PasteElementsCommand {
    json: String,
    offset: Vec2,
    focus: Option<Vec2>,
    started: bool,
    batch: SpawnBatch,
}

impl PasteElementsCommand {
    /// 创建一条尚未执行的粘贴命令。
    ///
    /// # 参数
    /// - `json`: [`Scene::copy_elements_json`](crate::Scene::copy_elements_json) 或整份场景 JSON
    /// - `offset`: 额外世界位移
    /// - `focus`: 粘贴中心。`None` 时元素留在文档坐标上，只加 `offset`
    pub fn new(json: String, offset: Vec2, focus: Option<Vec2>) -> Self {
        Self {
            json,
            offset,
            focus,
            started: false,
            batch: SpawnBatch::default(),
        }
    }

    /// 粘贴已经写入场景时使用，再 [`CommandHistory::push_executed`]。
    ///
    /// # 参数
    /// - `roots`: [`Scene::paste_json`](crate::Scene::paste_json) 返回的新根
    pub fn applied(
        json: String,
        offset: Vec2,
        focus: Option<Vec2>,
        roots: Vec<ElementId>,
        scene: &Scene,
    ) -> Self {
        Self {
            json,
            offset,
            focus,
            started: true,
            batch: capture_spawn(scene, &roots),
        }
    }
}

impl Command for PasteElementsCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if !self.started {
            self.started = true;
            if let Ok(roots) = scene.paste_json(&self.json, self.offset, self.focus) {
                select_spawn_roots(scene, &roots);
                self.batch = capture_spawn(scene, &roots);
            }
            return;
        }
        replay_spawn(scene, &mut self.batch);
    }

    fn undo(&mut self, scene: &mut Scene) {
        remove_spawn(scene, &mut self.batch);
    }

    fn name(&self) -> &'static str {
        "PasteElements"
    }
}

#[derive(Debug, Default)]
struct SpawnBatch {
    elements: Vec<AnyElement>,
    roots: Vec<ElementId>,
    /// 撤销后引用归零的图片像素。重做时先放回资源表，插入元素再加引用。
    parked: Vec<crate::asset::ImageAsset>,
}

fn select_spawn_roots(scene: &mut Scene, roots: &[ElementId]) {
    for id in roots {
        if !scene.contains(*id) || scene.is_effectively_locked(*id) {
            continue;
        }
        if let Some(element) = scene.get_mut(*id) {
            element.as_element_mut().props_mut().selected = true;
        }
    }
}

fn capture_spawn(scene: &Scene, roots: &[ElementId]) -> SpawnBatch {
    let mut seen = std::collections::HashSet::new();
    let mut elements = Vec::new();
    for root in roots {
        for id in scene.subtree_ids(*root) {
            if seen.insert(id) {
                if let Some(element) = scene.get(id) {
                    elements.push(element.clone());
                }
            }
        }
    }
    SpawnBatch {
        elements,
        roots: roots.to_vec(),
        parked: Vec::new(),
    }
}

fn remove_spawn(scene: &mut Scene, batch: &mut SpawnBatch) {
    for root in batch.roots.iter().rev() {
        scene.remove(*root);
    }
    batch.parked = parked_assets_for_spawn(scene, &batch.elements);
}

fn replay_spawn(scene: &mut Scene, batch: &mut SpawnBatch) {
    scene.restore_assets(std::mem::take(&mut batch.parked));
    scene.insert_copied(&batch.elements);
}

/// 克隆和粘贴撤销时，留下这棵新子树里引用已经归零的图片。
///
/// 与 [`parked_asset_for`] 不同：这里包含组里的子图片。删除组的命令仍只留根节点自己的像素。
fn parked_assets_for_spawn(
    scene: &mut Scene,
    elements: &[AnyElement],
) -> Vec<crate::asset::ImageAsset> {
    let keep: std::collections::HashSet<String> = elements
        .iter()
        .filter_map(|element| match element {
            AnyElement::Image(image) if !image.asset_id.is_empty() => Some(image.asset_id.clone()),
            _ => None,
        })
        .collect();
    scene
        .take_detached_assets()
        .into_iter()
        .filter(|asset| keep.contains(&asset.id))
        .collect()
}

/// 撤销栈只保留被存下来的那个元素自己的图片。
///
/// 删除组时子孙会离开场景，但命令只存根节点。它们的像素如果已经没有引用，就在这里丢掉，
/// 避免撤销时把无人引用的资源塞回表里。
fn parked_asset_for(scene: &mut Scene, element: &AnyElement) -> Vec<crate::asset::ImageAsset> {
    let keep = match element {
        AnyElement::Image(image) if !image.asset_id.is_empty() => Some(image.asset_id.clone()),
        _ => None,
    };
    scene
        .take_detached_assets()
        .into_iter()
        .filter(|asset| keep.as_ref() == Some(&asset.id))
        .collect()
}

fn apply_text_visual(scene: &mut Scene, id: ElementId, visual: &TextVisual) {
    if let Some(AnyElement::Text(text)) = scene.get_mut(id) {
        let _ = text.apply_visual(visual);
    }
    // 执行、撤销和重做都要重生成纹理四边形。
    scene.mark_dirty(id);
}

/// 一次样式修改前后的快照。
#[derive(Debug, Clone)]
struct StyleChange {
    id: ElementId,
    before: StyleSnapshot,
    after: StyleSnapshot,
}

/// 修改一个或多个元素的填充、描边、线宽和不透明度。
///
/// 文本还会记录字号和文字颜色。图片只接受不透明度，写到 `Image::alpha`，
/// 不改 `asset_id`，也不动资源表。执行和撤销都会 [`Scene::mark_dirty`]，
/// 顶点缓存和文本光栅因此会刷新。拖动滑块时由调用方合成一条命令：
/// 预览直接写场景，松手后用 [`SetStyleCommand::from_states`] 入栈。
///
/// # 示例
/// ```
/// use infinitecanvas_engine::{Command, Rectangle, Scene, SetStyleCommand, StylePatch};
/// use infinitecanvas_math::{Color, Vec2};
///
/// let mut scene = Scene::new();
/// scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
/// let patch = StylePatch {
///     fill: Some(Some(Color::RED)),
///     opacity: Some(0.5),
///     ..StylePatch::default()
/// };
/// let mut cmd = SetStyleCommand::from_patch(&scene, &[1], &patch).unwrap();
/// cmd.execute(&mut scene);
/// ```
#[derive(Debug)]
pub struct SetStyleCommand {
    changes: Vec<StyleChange>,
}

impl SetStyleCommand {
    /// 根据当前场景和补丁生成命令。没有实际变化时返回 `None`。
    ///
    /// 锁定的元素（含被锁定的祖先）会跳过。选中组时补丁作用在子孙叶子上。
    pub fn from_patch(scene: &Scene, ids: &[ElementId], patch: &StylePatch) -> Option<Self> {
        let mut changes = Vec::new();
        for id in style_target_ids(scene, ids) {
            if style_locked(scene, id) {
                continue;
            }
            let Some(before) = capture_style(scene, id) else {
                continue;
            };
            let mut after = before.clone();
            if after.apply_patch(patch) {
                changes.push(StyleChange { id, before, after });
            }
        }
        if changes.is_empty() {
            None
        } else {
            Some(Self { changes })
        }
    }

    /// 用已经发生的前后快照组一条命令。供滑块松手时入栈，不会再次执行。
    ///
    /// `before` 与 `after` 按相同元素顺序排列。
    pub fn from_states(
        before: Vec<(ElementId, StyleSnapshot)>,
        after: Vec<(ElementId, StyleSnapshot)>,
    ) -> Self {
        let changes = before
            .into_iter()
            .zip(after)
            .map(|((id, before), (_after_id, after))| StyleChange { id, before, after })
            .collect();
        Self { changes }
    }
}

impl Command for SetStyleCommand {
    fn execute(&mut self, scene: &mut Scene) {
        for change in &self.changes {
            apply_style_snapshot(scene, change.id, &change.after);
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        for change in &self.changes {
            apply_style_snapshot(scene, change.id, &change.before);
        }
    }

    fn name(&self) -> &'static str {
        "SetStyle"
    }
}

/// 命令历史（撤销/重做栈）
#[derive(Default)]
pub struct CommandHistory {
    /// 撤销栈
    undo_stack: VecDeque<Box<dyn Command>>,
    /// 重做栈
    redo_stack: VecDeque<Box<dyn Command>>,
    /// 最大历史记录数
    max_history: usize,
}

impl CommandHistory {
    /// 创建命令历史
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_history: 100,
        }
    }

    /// 设置最大历史记录数
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// 执行命令并添加到历史
    pub fn execute(&mut self, mut command: Box<dyn Command>, scene: &mut Scene) {
        command.execute(scene);

        // 清空重做栈（新操作后不能重做之前的操作）
        self.redo_stack.clear();

        // 添加到撤销栈
        self.undo_stack.push_back(command);

        // 限制历史记录数
        while self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }
    }

    /// 将已执行的命令添加到历史（用于拖动等实时操作）
    ///
    /// 与 `execute` 不同，这个方法不会执行命令，只是将其添加到历史。
    /// 用于那些已经在其他地方执行过的操作（如实时拖动）。
    pub fn push_executed(&mut self, command: Box<dyn Command>) {
        // 清空重做栈
        self.redo_stack.clear();

        // 添加到撤销栈
        self.undo_stack.push_back(command);

        // 限制历史记录数
        while self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }
    }

    /// 撤销上一个命令
    pub fn undo(&mut self, scene: &mut Scene) -> bool {
        if let Some(mut command) = self.undo_stack.pop_back() {
            command.undo(scene);
            self.redo_stack.push_back(command);
            true
        } else {
            false
        }
    }

    /// 重做上一个撤销的命令
    pub fn redo(&mut self, scene: &mut Scene) -> bool {
        if let Some(mut command) = self.redo_stack.pop_back() {
            command.execute(scene);
            self.undo_stack.push_back(command);
            true
        } else {
            false
        }
    }

    /// 是否可以撤销
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// 是否可以重做
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 获取撤销栈大小
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// 获取重做栈大小
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// 清空历史
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl std::fmt::Debug for CommandHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandHistory")
            .field("undo_count", &self.undo_stack.len())
            .field("redo_count", &self.redo_stack.len())
            .field("max_history", &self.max_history)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SelectionManager;
    use crate::{element::Rectangle, math::Vec2};

    #[test]
    fn test_move_command() {
        let mut scene = Scene::new();
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 50.0));
        scene.add_rect(rect);

        let mut cmd = MoveCommand::new(vec![1], Vec2::new(10.0, 20.0));

        // 执行
        cmd.execute(&mut scene);
        let bounds = scene.get(1).unwrap().as_element().world_bounds();
        assert!((bounds.x - 85.0).abs() < 0.1); // 100 - 25 (half width) + 10

        // 撤销
        cmd.undo(&mut scene);
        let bounds = scene.get(1).unwrap().as_element().world_bounds();
        assert!((bounds.x - 75.0).abs() < 0.1); // 100 - 25 (half width)
    }

    #[test]
    fn test_command_history_undo_redo() {
        let mut scene = Scene::new();
        let rect = Rectangle::new(1, Vec2::new(100.0, 100.0), Vec2::new(50.0, 50.0));
        scene.add_rect(rect);

        let mut history = CommandHistory::new();

        // 执行移动命令
        history.execute(
            Box::new(MoveCommand::new(vec![1], Vec2::new(50.0, 0.0))),
            &mut scene,
        );

        assert!(history.can_undo());
        assert!(!history.can_redo());

        // 撤销
        history.undo(&mut scene);
        assert!(!history.can_undo());
        assert!(history.can_redo());

        // 重做
        history.redo(&mut scene);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    fn paint_ids(scene: &Scene) -> Vec<ElementId> {
        let mut ids: Vec<ElementId> = scene.iter().map(|element| element.id()).collect();
        ids.sort_by_key(|id| {
            let z_index = scene.get(*id).unwrap().as_element().props().z_index;
            (z_index, *id)
        });
        ids
    }

    #[test]
    fn reorder_command_undo_redo_restores_z_order() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(40.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let mut history = CommandHistory::new();
        history.execute(Box::new(ReorderOutlineCommand::new(1, None, 2)), &mut scene);
        assert_eq!(paint_ids(&scene), vec![2, 3, 1]);

        assert!(history.undo(&mut scene));
        assert_eq!(paint_ids(&scene), vec![1, 2, 3]);
        assert!(history.redo(&mut scene));
        assert_eq!(paint_ids(&scene), vec![2, 3, 1]);
    }

    #[test]
    fn reparent_command_undo_restores_group_membership() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 10.0)));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(30.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(70.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let before = scene.outline_snapshot();
        let refresh = scene
            .move_in_outline(3, Some(group_id), 2)
            .unwrap()
            .expect("child moved into the group");
        let mut history = CommandHistory::new();
        history.push_executed(Box::new(ReorderOutlineCommand::applied(
            3,
            Some(group_id),
            2,
            before,
            refresh,
        )));
        assert_eq!(scene.parent_of(3), Some(group_id));
        assert_eq!(scene.children_of(group_id), vec![1, 2, 3]);
        let moved = scene
            .get(3)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation;
        assert_eq!(moved, Vec2::new(70.0, 0.0));

        history.undo(&mut scene);
        assert!(scene.parent_of(3).is_none());
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        assert_eq!(
            scene
                .get(3)
                .unwrap()
                .as_element()
                .props()
                .transform
                .translation,
            Vec2::new(70.0, 0.0)
        );
        history.redo(&mut scene);
        assert_eq!(scene.children_of(group_id), vec![1, 2, 3]);
    }

    #[test]
    fn set_style_apply_undo_and_redo_cover_shapes_and_text() {
        use crate::element::{Circle, ElementStyle, Line, Path, Text};
        use crate::style::{capture_style, StyleSnapshot};
        use infinitecanvas_math::Color;

        let mut scene = Scene::new();
        scene.add_rect(
            Rectangle::new(1, Vec2::ZERO, Vec2::new(10.0, 8.0))
                .with_style(ElementStyle::fill_only(Color::BLACK)),
        );
        scene.add_circle(
            Circle::new(2, Vec2::new(20.0, 0.0), 6.0)
                .with_style(ElementStyle::fill_only(Color::BLACK)),
        );
        scene.add_line(Line::from_world(
            3,
            Vec2::ZERO,
            Vec2::new(12.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 1.0),
            false,
        ));
        scene.add_path(Path::from_world_points(
            4,
            &[Vec2::ZERO, Vec2::new(8.0, 3.0)],
            ElementStyle::stroke_only(Color::BLACK, 1.0),
        ));
        scene.add_text(Text::new(5, Vec2::new(0.0, 20.0), "A").with_font_size(16.0));

        let patch = StylePatch {
            fill: Some(Some(Color::RED)),
            stroke: Some(Some(Color::BLUE)),
            stroke_width: Some(5.0),
            opacity: Some(0.4),
            font_size: Some(28.0),
            text_color: Some(Color::GREEN),
            ..StylePatch::default()
        };
        let mut history = CommandHistory::new();
        history.execute(
            Box::new(SetStyleCommand::from_patch(&scene, &[1, 2, 3, 4, 5], &patch).unwrap()),
            &mut scene,
        );
        assert!(scene.has_dirty_elements());
        assert_eq!(history.undo_count(), 1);
        match capture_style(&scene, 1) {
            Some(StyleSnapshot::Shape(style)) => {
                assert_eq!(style.fill, Some(Color::RED));
                assert_eq!(style.stroke, Some(Color::BLUE));
                assert_eq!(style.stroke_width, 5.0);
                assert_eq!(style.opacity, 0.4);
            }
            other => panic!("shape, got {other:?}"),
        }
        match scene.get(5).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.font_size, 28.0);
                assert_eq!(text.color, Color::GREEN);
                assert_eq!(text.opacity, 0.4);
                assert_eq!(text.stroke_width, 5.0);
                assert!(text.needs_raster(0));
            }
            other => panic!("text, got {other:?}"),
        }

        assert!(history.undo(&mut scene));
        match capture_style(&scene, 1) {
            Some(StyleSnapshot::Shape(style)) => {
                assert_eq!(style.fill, Some(Color::BLACK));
                assert_eq!(style.stroke, None);
                assert_eq!(style.opacity, 1.0);
            }
            other => panic!("shape, got {other:?}"),
        }
        match scene.get(5).unwrap() {
            AnyElement::Text(text) => {
                assert_eq!(text.font_size, 16.0);
                assert_eq!(text.opacity, 1.0);
                assert_eq!(text.stroke_width, 0.0);
            }
            other => panic!("text, got {other:?}"),
        }
        assert!(scene.has_dirty_elements());

        assert!(history.redo(&mut scene));
        match scene.get(2).unwrap() {
            AnyElement::Circle(circle) => {
                assert_eq!(circle.style.fill, Some(Color::RED));
                assert_eq!(circle.style.stroke_width, 5.0);
            }
            other => panic!("circle, got {other:?}"),
        }
        match scene.get(3).unwrap() {
            AnyElement::Line(line) => assert_eq!(line.style.stroke_width, 5.0),
            other => panic!("line, got {other:?}"),
        }
        match scene.get(4).unwrap() {
            AnyElement::Path(path) => assert_eq!(path.style.opacity, 0.4),
            other => panic!("path, got {other:?}"),
        }

        assert!(SetStyleCommand::from_patch(&scene, &[1], &patch).is_none());
    }

    fn translation_of(scene: &Scene, id: ElementId) -> Vec2 {
        scene
            .get(id)
            .unwrap()
            .as_element()
            .props()
            .transform
            .translation
    }

    fn approx_eq(actual: Vec2, expected: Vec2) {
        assert!(
            (actual - expected).length() < 1e-3,
            "actual {actual:?} expected {expected:?}"
        );
    }

    fn selected(scene: &mut Scene, ids: &[ElementId]) -> Vec<ElementId> {
        let mut selection = SelectionManager::new();
        for (index, id) in ids.iter().enumerate() {
            assert!(
                selection.select_element(scene, *id, index > 0),
                "select {id}"
            );
        }
        selection.selected_ids()
    }

    /// 单选向右移动 1 个世界单位。旋转和缩放不动，撤销/重做只恢复平移。
    #[test]
    fn single_selection_nudge_moves_one_world_unit() {
        let mut scene = Scene::new();
        let mut rect = Rectangle::new(1, Vec2::new(3.0, 4.0), Vec2::new(8.0, 6.0));
        rect.props.set_rotation(0.4);
        rect.props.transform.scale = Vec2::new(1.5, 0.5);
        let z_index = rect.props.z_index;
        scene.add_rect(rect);
        let ids = selected(&mut scene, &[1]);
        let mut history = CommandHistory::new();

        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Right,
            false,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(4.0, 4.0));
        let props = scene.get(1).unwrap().as_element().props();
        assert!((props.rotation() - 0.4).abs() < 1e-6);
        assert_eq!(props.transform.scale, Vec2::new(1.5, 0.5));
        assert_eq!(props.z_index, z_index);
        assert!(scene.parent_of(1).is_none());
        assert!(scene.has_dirty_elements());
        assert_eq!(history.undo_count(), 1);
        assert!(!history.can_redo());

        assert!(history.undo(&mut scene));
        approx_eq(translation_of(&scene, 1), Vec2::new(3.0, 4.0));
        assert!(history.can_redo());
        assert!(scene.has_dirty_elements());

        assert!(history.redo(&mut scene));
        approx_eq(translation_of(&scene, 1), Vec2::new(4.0, 4.0));
        assert_eq!(
            scene.get(1).unwrap().as_element().props().transform.scale,
            Vec2::new(1.5, 0.5)
        );
    }

    /// 四个方向沿世界轴。Y 向下，所以上是 -Y。
    #[test]
    fn nudge_directions_follow_world_axes() {
        assert_eq!(NudgeDirection::Left.delta(false), Vec2::new(-1.0, 0.0));
        assert_eq!(NudgeDirection::Right.delta(false), Vec2::new(1.0, 0.0));
        assert_eq!(NudgeDirection::Up.delta(false), Vec2::new(0.0, -1.0));
        assert_eq!(NudgeDirection::Down.delta(false), Vec2::new(0.0, 1.0));
        assert_eq!(
            NudgeDirection::parse(" arrowLEFT "),
            Some(NudgeDirection::Left)
        );
        assert!(NudgeDirection::parse("Delete").is_none());

        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(10.0, 20.0),
            Vec2::new(4.0, 4.0),
        ));
        let ids = selected(&mut scene, &[1]);
        let mut history = CommandHistory::new();
        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Down,
            false,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(10.0, 21.0));
        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Left,
            false,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(9.0, 21.0));
        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Up,
            false,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(9.0, 20.0));
    }

    /// Shift 步长是普通步长的 10 倍。
    #[test]
    fn shift_nudge_step_is_ten_times() {
        assert_eq!(NUDGE_SHIFT_FACTOR, 10.0);
        assert_eq!(NudgeDirection::Right.delta(true).x, NUDGE_STEP * 10.0);

        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::new(2.0, 5.0), Vec2::new(4.0, 4.0)));
        let ids = selected(&mut scene, &[1]);
        let mut history = CommandHistory::new();
        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Up,
            true,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(2.0, -5.0));
        assert_eq!(history.undo_count(), 1);
        history.undo(&mut scene);
        approx_eq(translation_of(&scene, 1), Vec2::new(2.0, 5.0));
    }

    /// 多选一次移动全部目标，只产生一条历史。未选中的元素不动。
    #[test]
    fn multi_selection_nudge_is_one_history_entry() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(30.0, 8.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            3,
            Vec2::new(80.0, -4.0),
            Vec2::new(10.0, 10.0),
        ));
        let ids = selected(&mut scene, &[1, 2]);
        let mut history = CommandHistory::new();

        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Right,
            false,
        ));
        assert_eq!(history.undo_count(), 1);
        approx_eq(translation_of(&scene, 1), Vec2::new(1.0, 0.0));
        approx_eq(translation_of(&scene, 2), Vec2::new(31.0, 8.0));
        approx_eq(translation_of(&scene, 3), Vec2::new(80.0, -4.0));

        assert!(history.undo(&mut scene));
        approx_eq(translation_of(&scene, 1), Vec2::ZERO);
        approx_eq(translation_of(&scene, 2), Vec2::new(30.0, 8.0));
        assert!(history.redo(&mut scene));
        approx_eq(translation_of(&scene, 1), Vec2::new(1.0, 0.0));
        approx_eq(translation_of(&scene, 2), Vec2::new(31.0, 8.0));
    }

    /// 选中组时平移组根，子孙只移动一次。组内子元素单独选中时不带动兄弟。
    #[test]
    fn group_selection_nudge_translates_group_root() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(
            1,
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        scene.add_rect(Rectangle::new(
            2,
            Vec2::new(20.0, 0.0),
            Vec2::new(10.0, 10.0),
        ));
        let group_id = scene.group(&[1, 2]).unwrap();
        let group_at = translation_of(&scene, group_id);
        let mut history = CommandHistory::new();

        let both = selected(&mut scene, &[group_id, 1]);
        assert_eq!(nudge_targets(&scene, &both), vec![group_id]);
        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &both,
            NudgeDirection::Right,
            false,
        ));
        assert_eq!(history.undo_count(), 1);
        approx_eq(translation_of(&scene, 1), Vec2::new(1.0, 0.0));
        approx_eq(translation_of(&scene, 2), Vec2::new(21.0, 0.0));
        approx_eq(
            translation_of(&scene, group_id),
            group_at + Vec2::new(1.0, 0.0),
        );
        let dirty = scene.take_dirty_ids();
        assert!(dirty.contains(&group_id) && dirty.contains(&1) && dirty.contains(&2));

        assert!(history.undo(&mut scene));
        approx_eq(translation_of(&scene, 1), Vec2::ZERO);
        approx_eq(translation_of(&scene, 2), Vec2::new(20.0, 0.0));
        approx_eq(translation_of(&scene, group_id), group_at);
        assert!(history.redo(&mut scene));
        approx_eq(translation_of(&scene, 1), Vec2::new(1.0, 0.0));
        history.undo(&mut scene);

        let child_only = selected(&mut scene, &[1]);
        assert!(apply_nudge(
            &mut history,
            &mut scene,
            &child_only,
            NudgeDirection::Down,
            false,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(0.0, 1.0));
        approx_eq(translation_of(&scene, 2), Vec2::new(20.0, 0.0));
        history.undo(&mut scene);
        approx_eq(translation_of(&scene, 1), Vec2::ZERO);
        approx_eq(translation_of(&scene, 2), Vec2::new(20.0, 0.0));
    }

    /// 锁定、隐藏或空选不入栈，位置不变。
    #[test]
    fn locked_hidden_or_empty_selection_is_noop() {
        let mut scene = Scene::new();
        scene.add_rect(Rectangle::new(1, Vec2::new(6.0, 7.0), Vec2::new(4.0, 4.0)));
        scene.add_rect(Rectangle::new(2, Vec2::new(16.0, 7.0), Vec2::new(4.0, 4.0)));
        let mut history = CommandHistory::new();

        assert!(!apply_nudge(
            &mut history,
            &mut scene,
            &[],
            NudgeDirection::Right,
            false,
        ));
        assert!(
            NudgeElementsCommand::from_selection(&scene, &[], NudgeDirection::Left, true,)
                .is_none()
        );
        assert_eq!(history.undo_count(), 0);

        let ids = selected(&mut scene, &[1]);
        scene.set_locked(1, true);
        assert!(!apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Right,
            true,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(6.0, 7.0));
        assert_eq!(history.undo_count(), 0);
        scene.set_locked(1, false);

        scene.set_visible(1, false);
        assert!(!apply_nudge(
            &mut history,
            &mut scene,
            &ids,
            NudgeDirection::Down,
            false,
        ));
        approx_eq(translation_of(&scene, 1), Vec2::new(6.0, 7.0));
        assert_eq!(history.undo_count(), 0);
        scene.set_visible(1, true);

        let group_id = scene.group(&[1, 2]).unwrap();
        scene.set_locked(group_id, true);
        assert!(NudgeElementsCommand::from_selection(
            &scene,
            &[group_id, 1],
            NudgeDirection::Left,
            false,
        )
        .is_none());
        approx_eq(translation_of(&scene, 1), Vec2::new(6.0, 7.0));
        approx_eq(translation_of(&scene, 2), Vec2::new(16.0, 7.0));
        assert!(!history.can_undo());
    }

    fn stack_scene() -> Scene {
        let mut scene = Scene::new();
        for (id, x) in [(1, 0.0), (2, 20.0), (3, 40.0), (4, 60.0), (5, 80.0)] {
            scene.add_rect(Rectangle::new(id, Vec2::new(x, 0.0), Vec2::new(10.0, 10.0)));
        }
        scene
    }

    fn assert_stack(action: StackOrderAction, selected: &[ElementId], expected: &[ElementId]) {
        let mut scene = stack_scene();
        let mut history = CommandHistory::new();
        let origin = translation_of(&scene, selected[0]);
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            selected,
            action
        ));
        assert_eq!(scene.sibling_order(None), expected);
        assert_eq!(history.undo_count(), 1);
        assert_eq!(history.redo_count(), 0);
        approx_eq(translation_of(&scene, selected[0]), origin);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.sibling_order(None), vec![1, 2, 3, 4, 5]);
        assert!(history.redo(&mut scene));
        assert_eq!(scene.sibling_order(None), expected);
    }

    #[test]
    fn stack_actions_reorder_same_parent_siblings() {
        assert_stack(StackOrderAction::Front, &[2, 4], &[1, 3, 5, 2, 4]);
        assert_stack(StackOrderAction::Back, &[2, 4], &[2, 4, 1, 3, 5]);
        assert_stack(StackOrderAction::Forward, &[2, 4], &[1, 3, 2, 5, 4]);
        assert_stack(StackOrderAction::Backward, &[2, 4], &[2, 1, 4, 3, 5]);
        assert_stack(StackOrderAction::Forward, &[2, 3], &[1, 4, 2, 3, 5]);
        assert_stack(StackOrderAction::Backward, &[2, 3], &[2, 3, 1, 4, 5]);
        assert_stack(StackOrderAction::Front, &[1], &[2, 3, 4, 5, 1]);
        assert_stack(StackOrderAction::Back, &[5], &[5, 1, 2, 3, 4]);
    }

    #[test]
    fn stack_reorder_already_at_end_is_noop() {
        let mut scene = stack_scene();
        let mut history = CommandHistory::new();
        let epoch = scene.structure_epoch();
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[5],
            StackOrderAction::Front,
        ));
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[1],
            StackOrderAction::Back,
        ));
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[4, 5],
            StackOrderAction::Forward,
        ));
        assert_eq!(scene.sibling_order(None), vec![1, 2, 3, 4, 5]);
        assert_eq!(scene.structure_epoch(), epoch);
        assert_eq!(history.undo_count(), 0);
    }

    #[test]
    fn cross_parent_or_empty_selection_does_not_reorder() {
        let mut scene = stack_scene();
        let group_id = scene.group(&[1, 2]).unwrap();
        let mut history = CommandHistory::new();
        let epoch = scene.structure_epoch();
        let roots = scene.sibling_order(None);
        let children = scene.children_of(group_id);

        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[],
            StackOrderAction::Front,
        ));
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[99],
            StackOrderAction::Back,
        ));
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[1, 3],
            StackOrderAction::Forward,
        ));
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[group_id, 1],
            StackOrderAction::Backward,
        ));
        assert_eq!(scene.sibling_order(None), roots);
        assert_eq!(scene.children_of(group_id), children);
        assert_eq!(scene.structure_epoch(), epoch);
        assert!(!history.can_undo());
    }

    #[test]
    fn selecting_a_group_moves_only_the_group_root() {
        let mut scene = stack_scene();
        let group_id = scene.group(&[1, 2]).unwrap();
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        assert_eq!(scene.sibling_order(None), vec![3, 4, 5, group_id]);

        let mut history = CommandHistory::new();
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[group_id],
            StackOrderAction::Back,
        ));
        assert_eq!(scene.sibling_order(None), vec![group_id, 3, 4, 5]);
        assert_eq!(scene.children_of(group_id), vec![1, 2]);
        assert_eq!(scene.parent_of(1), Some(group_id));
        assert_eq!(scene.parent_of(2), Some(group_id));
        approx_eq(translation_of(&scene, 1), Vec2::ZERO);
        approx_eq(translation_of(&scene, 2), Vec2::new(20.0, 0.0));
        assert_eq!(history.undo_count(), 1);

        assert!(history.undo(&mut scene));
        assert_eq!(scene.sibling_order(None), vec![3, 4, 5, group_id]);
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[1],
            StackOrderAction::Front,
        ));
        assert_eq!(scene.children_of(group_id), vec![2, 1]);
        assert_eq!(scene.sibling_order(None), vec![3, 4, 5, group_id]);
        assert_eq!(scene.parent_of(1), Some(group_id));
    }

    #[test]
    fn locked_selection_is_skipped_and_hidden_still_reorders() {
        let mut scene = stack_scene();
        let mut history = CommandHistory::new();
        scene.set_locked(2, true);
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[1, 2],
            StackOrderAction::Front,
        ));
        assert_eq!(scene.sibling_order(None), vec![2, 3, 4, 5, 1]);
        assert!(history.undo(&mut scene));

        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[2],
            StackOrderAction::Back,
        ));
        assert_eq!(history.undo_count(), 0);

        let group_id = scene.group(&[3, 4]).unwrap();
        scene.set_locked(group_id, true);
        let children = scene.children_of(group_id);
        let roots = scene.sibling_order(None);
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[3],
            StackOrderAction::Forward,
        ));
        assert!(!apply_stack_reorder(
            &mut history,
            &mut scene,
            &[group_id],
            StackOrderAction::Front,
        ));
        assert_eq!(scene.children_of(group_id), children);
        assert_eq!(scene.sibling_order(None), roots);

        scene.set_locked(group_id, false);
        scene.set_visible(group_id, false);
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[4],
            StackOrderAction::Back,
        ));
        assert_eq!(scene.children_of(group_id), vec![4, 3]);
        assert!(!scene.is_effectively_visible(4));
        scene.set_visible(group_id, true);
        scene.set_visible(4, false);
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[4],
            StackOrderAction::Front,
        ));
        assert_eq!(scene.children_of(group_id), vec![3, 4]);
    }

    #[test]
    fn stack_reorder_repacks_vertex_cache_without_geometry_dirty() {
        use crate::VertexCache;

        let mut scene = stack_scene();
        let mut cache = VertexCache::new();
        let first = cache.sync(&mut scene);
        assert!(first.full_upload);
        let epoch = scene.structure_epoch();
        let mut history = CommandHistory::new();
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[1, 2],
            StackOrderAction::Front,
        ));
        assert_eq!(scene.structure_epoch(), epoch + 1);
        assert!(!scene.has_dirty_elements());
        let second = cache.sync(&mut scene);
        assert!(second.full_upload);
        assert!(!second.geometry_unchanged);
        assert_eq!(scene.sibling_order(None), vec![3, 4, 5, 1, 2]);
    }

    #[test]
    fn drag_reorder_and_stack_reorder_undo_separately() {
        let mut scene = stack_scene();
        let mut history = CommandHistory::new();
        history.execute(Box::new(ReorderOutlineCommand::new(1, None, 4)), &mut scene);
        assert_eq!(scene.sibling_order(None), vec![2, 3, 4, 5, 1]);
        assert!(apply_stack_reorder(
            &mut history,
            &mut scene,
            &[2, 3],
            StackOrderAction::Front,
        ));
        assert_eq!(scene.sibling_order(None), vec![4, 5, 1, 2, 3]);
        assert_eq!(history.undo_count(), 2);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.sibling_order(None), vec![2, 3, 4, 5, 1]);
        assert!(history.undo(&mut scene));
        assert_eq!(scene.sibling_order(None), vec![1, 2, 3, 4, 5]);
    }
}

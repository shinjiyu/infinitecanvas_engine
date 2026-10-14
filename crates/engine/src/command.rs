//! 命令系统
//!
//! 实现命令模式，支持撤销/重做功能

use crate::element::{ElementId, TextVisual};
use crate::scene::OutlineSnapshot;
use crate::style::{
    apply_style_snapshot, capture_style, style_locked, style_target_ids, StylePatch, StyleSnapshot,
};
use crate::{math::Vec2, AnyElement, Scene};
use std::collections::VecDeque;

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

/// 添加元素命令
#[derive(Debug)]
pub struct AddElementCommand {
    /// 要添加的元素（执行后为 None）
    element: Option<AnyElement>,
    /// 元素 ID（用于撤销时删除）
    element_id: ElementId,
}

impl AddElementCommand {
    pub fn new(element: AnyElement) -> Self {
        let element_id = element.id();
        Self {
            element: Some(element),
            element_id,
        }
    }
}

impl Command for AddElementCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if let Some(element) = self.element.take() {
            scene.add(element);
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        if let Some(element) = scene.remove(self.element_id) {
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
}

impl RemoveElementCommand {
    pub fn new(element_id: ElementId) -> Self {
        Self {
            element: None,
            element_id,
        }
    }
}

impl Command for RemoveElementCommand {
    fn execute(&mut self, scene: &mut Scene) {
        if let Some(element) = scene.remove(self.element_id) {
            self.element = Some(element);
        }
    }

    fn undo(&mut self, scene: &mut Scene) {
        if let Some(element) = self.element.take() {
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
        scene.insert_copied(&self.batch.elements);
    }

    fn undo(&mut self, scene: &mut Scene) {
        remove_spawn(scene, &self.batch);
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
        scene.insert_copied(&self.batch.elements);
    }

    fn undo(&mut self, scene: &mut Scene) {
        remove_spawn(scene, &self.batch);
    }

    fn name(&self) -> &'static str {
        "PasteElements"
    }
}

#[derive(Debug, Default)]
struct SpawnBatch {
    elements: Vec<AnyElement>,
    roots: Vec<ElementId>,
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
    }
}

fn remove_spawn(scene: &mut Scene, batch: &SpawnBatch) {
    for root in batch.roots.iter().rev() {
        scene.remove(*root);
    }
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
/// 文本还会记录字号和文字颜色。执行和撤销都会 [`Scene::mark_dirty`]，
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
}

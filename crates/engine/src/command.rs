//! 命令系统
//!
//! 实现命令模式，支持撤销/重做功能

use crate::element::{ElementId, TextVisual};
use crate::scene::OutlineSnapshot;
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

fn apply_text_visual(scene: &mut Scene, id: ElementId, visual: &TextVisual) {
    if let Some(AnyElement::Text(text)) = scene.get_mut(id) {
        let _ = text.apply_visual(visual);
    }
    // 执行、撤销和重做都要重生成纹理四边形。
    scene.mark_dirty(id);
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
}

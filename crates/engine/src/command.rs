//! 命令系统
//!
//! 实现命令模式，支持撤销/重做功能

use crate::{element::ElementId, math::Vec2, AnyElement, Scene};
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
        for id in &self.element_ids {
            if let Some(element) = scene.get_mut(*id) {
                element.as_element_mut().props_mut().transform.translate(self.delta);
            }
        }
    }
    
    fn undo(&mut self, scene: &mut Scene) {
        for id in &self.element_ids {
            if let Some(element) = scene.get_mut(*id) {
                element.as_element_mut().props_mut().transform.translate(-self.delta);
            }
        }
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
            &mut scene
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
}


//! 工具系统
//!
//! 管理不同的编辑工具和模式

use serde::{Deserialize, Serialize};

/// 工具类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Tool {
    /// 选择工具（默认）- 选择、移动、框选
    #[default]
    Select,
    /// 矩形工具 - 绘制矩形
    Rectangle,
    /// 圆形工具 - 绘制圆形
    Circle,
    /// 平移工具 - 平移画布
    Pan,
    /// 手型工具 - 按住空格临时切换
    Hand,
}

impl Tool {
    /// 从字符串创建工具类型
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "select" | "v" => Some(Tool::Select),
            "rectangle" | "rect" | "r" => Some(Tool::Rectangle),
            "circle" | "ellipse" | "o" => Some(Tool::Circle),
            "pan" | "hand" | "h" => Some(Tool::Pan),
            _ => None,
        }
    }

    /// 获取工具名称
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Rectangle => "rectangle",
            Tool::Circle => "circle",
            Tool::Pan => "pan",
            Tool::Hand => "hand",
        }
    }

    /// 获取工具的光标样式
    pub fn cursor(&self) -> &'static str {
        match self {
            Tool::Select => "default",
            Tool::Rectangle => "crosshair",
            Tool::Circle => "crosshair",
            Tool::Pan => "grab",
            Tool::Hand => "grab",
        }
    }

    /// 是否是绘制工具
    pub fn is_drawing_tool(&self) -> bool {
        matches!(self, Tool::Rectangle | Tool::Circle)
    }
}

/// 工具管理器
#[derive(Debug, Default)]
pub struct ToolManager {
    /// 当前工具
    current: Tool,
    /// 上一个工具（用于临时工具切换）
    previous: Option<Tool>,
}

impl ToolManager {
    /// 创建工具管理器
    pub fn new() -> Self {
        Self {
            current: Tool::Select,
            previous: None,
        }
    }

    /// 获取当前工具
    pub fn current(&self) -> Tool {
        self.current
    }

    /// 设置当前工具
    pub fn set_tool(&mut self, tool: Tool) {
        if self.current != tool {
            self.previous = Some(self.current);
            self.current = tool;
        }
    }

    /// 临时切换到工具（如按住空格）
    pub fn push_temporary(&mut self, tool: Tool) {
        if self.current != tool {
            self.previous = Some(self.current);
            self.current = tool;
        }
    }

    /// 恢复到上一个工具
    pub fn pop_temporary(&mut self) {
        if let Some(prev) = self.previous.take() {
            self.current = prev;
        }
    }

    /// 是否是选择工具
    pub fn is_select(&self) -> bool {
        self.current == Tool::Select
    }

    /// 是否是绘制工具
    pub fn is_drawing(&self) -> bool {
        self.current.is_drawing_tool()
    }

    /// 是否是平移工具
    pub fn is_panning(&self) -> bool {
        matches!(self.current, Tool::Pan | Tool::Hand)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_from_str() {
        assert_eq!(Tool::from_str("select"), Some(Tool::Select));
        assert_eq!(Tool::from_str("v"), Some(Tool::Select));
        assert_eq!(Tool::from_str("rectangle"), Some(Tool::Rectangle));
        assert_eq!(Tool::from_str("r"), Some(Tool::Rectangle));
        assert_eq!(Tool::from_str("circle"), Some(Tool::Circle));
        assert_eq!(Tool::from_str("o"), Some(Tool::Circle));
        assert_eq!(Tool::from_str("unknown"), None);
    }

    #[test]
    fn test_tool_manager() {
        let mut manager = ToolManager::new();
        assert_eq!(manager.current(), Tool::Select);

        manager.set_tool(Tool::Rectangle);
        assert_eq!(manager.current(), Tool::Rectangle);
        assert!(manager.is_drawing());

        // 临时切换
        manager.push_temporary(Tool::Hand);
        assert_eq!(manager.current(), Tool::Hand);
        assert!(manager.is_panning());

        // 恢复
        manager.pop_temporary();
        assert_eq!(manager.current(), Tool::Rectangle);
    }
}






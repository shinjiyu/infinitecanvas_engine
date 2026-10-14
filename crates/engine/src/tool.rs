//! 工具系统
//!
//! 管理不同的编辑工具和模式

use serde::{Deserialize, Serialize};

/// 工具类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Tool {
    /// 选择工具（默认）- 选择、移动、框选、手柄缩放/旋转
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
    /// 直线工具 - 拖动绘制直线，按住 Shift 带箭头
    Line,
    /// 画笔工具 - 自由绘制路径
    Pen,
    /// 文本工具 - 单击创建，双击已有文本进入编辑
    Text,
    /// 吸管 - 单击采样最上层可见叶子的填充和描边，写到当前选择
    ///
    /// 快捷键是不带修饰键的 `I`。`Shift+I`、`Ctrl/Cmd+I`、`Alt+I` 不进入这个工具，
    /// 也不占用 V / L / P / R / O / T / H / N / K / F。
    Eyedropper,
    /// 正多边形工具 - 拖拽外接圆，默认六边形
    Polygon,
    /// 星形工具 - 拖拽外接圆，默认五角星
    Star,
    /// 画板工具 - 拖拽放置命名画板
    ///
    /// 快捷键是不带修饰键的 `F`。不占用 V / L / P / R / O / T / H / I / M / G / N / K。
    Frame,
}

impl std::str::FromStr for Tool {
    type Err = ();

    /// 从字符串创建工具类型。
    ///
    /// 未知名称返回 `Err(())`。`"hand"` 与 `"pan"` 都映射到 [`Tool::Pan`]。
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "select" | "v" => Ok(Tool::Select),
            "rectangle" | "rect" | "r" => Ok(Tool::Rectangle),
            "circle" | "ellipse" | "o" => Ok(Tool::Circle),
            "pan" | "hand" | "h" => Ok(Tool::Pan),
            "line" | "l" => Ok(Tool::Line),
            "pen" | "path" | "p" => Ok(Tool::Pen),
            "text" | "t" => Ok(Tool::Text),
            "eyedropper" | "dropper" | "i" => Ok(Tool::Eyedropper),
            "polygon" | "hexagon" | "n" => Ok(Tool::Polygon),
            "star" | "k" => Ok(Tool::Star),
            "frame" | "artboard" | "f" => Ok(Tool::Frame),
            _ => Err(()),
        }
    }
}

impl Tool {
    /// 获取工具名称
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Rectangle => "rectangle",
            Tool::Circle => "circle",
            Tool::Pan => "pan",
            Tool::Hand => "hand",
            Tool::Line => "line",
            Tool::Pen => "pen",
            Tool::Text => "text",
            Tool::Eyedropper => "eyedropper",
            Tool::Polygon => "polygon",
            Tool::Star => "star",
            Tool::Frame => "frame",
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
            Tool::Line => "crosshair",
            Tool::Pen => "crosshair",
            Tool::Text => "text",
            Tool::Eyedropper => "crosshair",
            Tool::Polygon | Tool::Star | Tool::Frame => "crosshair",
        }
    }

    /// 是否是绘制工具
    pub fn is_drawing_tool(&self) -> bool {
        matches!(
            self,
            Tool::Rectangle
                | Tool::Circle
                | Tool::Line
                | Tool::Pen
                | Tool::Polygon
                | Tool::Star
                | Tool::Frame
        )
    }
}

/// 工具管理器
#[derive(Debug, Default)]
pub struct ToolManager {
    /// 当前工具
    current: Tool,
    /// 上一个工具（用于临时工具切换）
    previous: Option<Tool>,
    /// 进入吸管之前的工具。按住空格切到手型时不会覆盖它。
    eyedropper_return: Option<Tool>,
}

impl ToolManager {
    /// 创建工具管理器
    pub fn new() -> Self {
        Self {
            current: Tool::Select,
            previous: None,
            eyedropper_return: None,
        }
    }

    /// 获取当前工具
    pub fn current(&self) -> Tool {
        self.current
    }

    /// 设置当前工具
    pub fn set_tool(&mut self, tool: Tool) {
        if self.current != tool {
            if tool == Tool::Eyedropper {
                self.eyedropper_return = Some(self.current);
            } else {
                self.eyedropper_return = None;
            }
            self.previous = Some(self.current);
            self.current = tool;
        }
    }

    /// 结束吸管，回到进入吸管之前的工具。
    ///
    /// 当前不是吸管时什么也不做。之前的工具若已不在（或是手型），回到选择工具。
    /// 临时手型只用 [`Self::previous`]，不会丢掉这里记下的返回目标。
    pub fn exit_eyedropper(&mut self) {
        if self.current != Tool::Eyedropper {
            return;
        }
        let back = self.eyedropper_return.take().unwrap_or(Tool::Select);
        self.current = if matches!(back, Tool::Eyedropper | Tool::Hand) {
            Tool::Select
        } else {
            back
        };
        self.previous = None;
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
    use std::str::FromStr;

    #[test]
    fn test_tool_from_str() {
        assert_eq!(Tool::from_str("select"), Ok(Tool::Select));
        assert_eq!(Tool::from_str("v"), Ok(Tool::Select));
        assert_eq!(Tool::from_str("rectangle"), Ok(Tool::Rectangle));
        assert_eq!(Tool::from_str("r"), Ok(Tool::Rectangle));
        assert_eq!(Tool::from_str("circle"), Ok(Tool::Circle));
        assert_eq!(Tool::from_str("o"), Ok(Tool::Circle));
        assert_eq!(Tool::from_str("l"), Ok(Tool::Line));
        assert_eq!(Tool::from_str("line"), Ok(Tool::Line));
        assert_eq!("p".parse::<Tool>(), Ok(Tool::Pen));
        assert_eq!("pen".parse::<Tool>(), Ok(Tool::Pen));
        assert_eq!("path".parse::<Tool>(), Ok(Tool::Pen));
        assert_eq!(Tool::from_str("text"), Ok(Tool::Text));
        assert_eq!(Tool::from_str("t"), Ok(Tool::Text));
        assert_eq!(Tool::from_str("eyedropper"), Ok(Tool::Eyedropper));
        assert_eq!(Tool::from_str("i"), Ok(Tool::Eyedropper));
        assert_eq!(Tool::Eyedropper.name(), "eyedropper");
        assert_eq!(Tool::Eyedropper.cursor(), "crosshair");
        assert!(!Tool::Eyedropper.is_drawing_tool());
        assert_eq!(Tool::from_str("polygon"), Ok(Tool::Polygon));
        assert_eq!(Tool::from_str("n"), Ok(Tool::Polygon));
        assert_eq!(Tool::from_str("star"), Ok(Tool::Star));
        assert_eq!(Tool::from_str("k"), Ok(Tool::Star));
        assert!(Tool::Polygon.is_drawing_tool());
        assert_eq!(Tool::Star.cursor(), "crosshair");
        assert_eq!(Tool::from_str("frame"), Ok(Tool::Frame));
        assert_eq!(Tool::from_str("artboard"), Ok(Tool::Frame));
        assert_eq!(Tool::from_str("f"), Ok(Tool::Frame));
        assert_eq!(Tool::Frame.name(), "frame");
        assert!(Tool::Frame.is_drawing_tool());
        assert_eq!(Tool::Text.cursor(), "text");
        assert!(!Tool::Text.is_drawing_tool());
        assert!(Tool::from_str("unknown").is_err());
        assert!(Tool::Line.is_drawing_tool());
        assert!(Tool::Pen.is_drawing_tool());
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

    #[test]
    fn eyedropper_returns_to_the_tool_it_interrupted() {
        let mut manager = ToolManager::new();
        manager.set_tool(Tool::Rectangle);
        manager.set_tool(Tool::Eyedropper);
        manager.push_temporary(Tool::Hand);
        manager.pop_temporary();
        assert_eq!(manager.current(), Tool::Eyedropper);
        manager.exit_eyedropper();
        assert_eq!(manager.current(), Tool::Rectangle);

        manager.set_tool(Tool::Select);
        manager.set_tool(Tool::Eyedropper);
        manager.exit_eyedropper();
        assert_eq!(manager.current(), Tool::Select);
        manager.exit_eyedropper();
        assert_eq!(manager.current(), Tool::Select);
    }
}

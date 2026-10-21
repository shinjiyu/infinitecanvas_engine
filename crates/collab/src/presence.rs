//! Yjs awareness 上的本地/远端在场状态（光标、选区、显示名）。
//!
//! 载荷只存在 awareness 里，不进 [`crate::ELEMENTS_ROOT`]，也不会出现在场景 JSON。
//! 权限、冲突 UI 不在本模块范围。

use infinitecanvas_engine::ElementId;
use serde::{Deserialize, Serialize};

/// 指针在世界坐标里的位置。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PresenceCursor {
    /// 世界坐标 X
    pub x: f64,
    /// 世界坐标 Y
    pub y: f64,
}

impl PresenceCursor {
    /// 构造世界坐标光标。
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 一个对等端广播的在场状态。
///
/// 经 yrs [`yrs::sync::Awareness`] 复制。字段名与
/// `packages/core/src/collaboration/presence.ts` 对齐，方便和现有
/// WebSocket 协同路径软协调（同一 JSON 形状；坐标语义以世界坐标为准）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenceState {
    /// 显示名
    pub name: String,
    /// CSS 颜色，如 `#4090f2`
    pub color: String,
    /// 本地指针世界坐标；没有指针时为 `None`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<PresenceCursor>,
    /// 当前选中的元素 id（十进制，与场景 id 相同）
    #[serde(default)]
    pub selection: Vec<ElementId>,
}

impl PresenceState {
    /// 构造只有显示名和颜色的在场状态。
    pub fn new(name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: color.into(),
            cursor: None,
            selection: Vec::new(),
        }
    }

    /// 设置世界坐标光标。
    pub fn with_cursor(mut self, x: f64, y: f64) -> Self {
        self.cursor = Some(PresenceCursor::new(x, y));
        self
    }

    /// 设置选中元素 id 列表。
    pub fn with_selection(mut self, selection: impl IntoIterator<Item = ElementId>) -> Self {
        self.selection = selection.into_iter().collect();
        self
    }
}

/// 远端在场快照：yrs client id + 状态。
#[derive(Debug, Clone, PartialEq)]
pub struct RemotePresence {
    /// yrs 副本 id（与 [`crate::CollabSession::client_id`] 同空间）
    pub client_id: u64,
    /// 在场状态
    pub state: PresenceState,
}

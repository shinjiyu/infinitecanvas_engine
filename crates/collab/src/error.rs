//! CRDT 会话返回的错误。

use crate::session::DOC_SCHEMA_VERSION;
use infinitecanvas_engine::ElementId;

/// 写入文档、应用远端更新或把文档物化成场景时的失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollabError {
    /// 二进制更新或元素 JSON 无法解析。
    Decode(String),
    /// yrs 拒绝了这批更新。
    Update(String),
    /// y-sync 握手失败。
    Protocol(String),
    /// 元素 JSON 能解析，但还原场景失败（未知种类、重复 id 等）。
    Persist(String),
    /// 文档 `meta.version` 不是本骨架认识的版本。
    UnsupportedVersion(String),
    /// `elements` 的键和 JSON 里的 `data.props.id` 不一致。
    KeyMismatch {
        /// 地图键
        key: String,
        /// JSON 里的元素 id
        id: ElementId,
    },
    /// [`crate::CollabSession::update`] 的 id 在本地场景里不存在。
    Missing(ElementId),
    /// [`crate::CollabSession::insert`] 的 id 已经在本地场景里。
    Duplicate(ElementId),
}

impl std::fmt::Display for CollabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollabError::Decode(message) => write!(f, "invalid CRDT payload: {message}"),
            CollabError::Update(message) => write!(f, "failed to apply Yjs update: {message}"),
            CollabError::Protocol(message) => write!(f, "y-sync protocol failed: {message}"),
            CollabError::Persist(message) => write!(f, "failed to materialize scene: {message}"),
            CollabError::UnsupportedVersion(found) => write!(
                f,
                "unsupported CRDT schema version {found} (supported: {DOC_SCHEMA_VERSION})"
            ),
            CollabError::KeyMismatch { key, id } => {
                write!(f, "element key {key} does not match id {id}")
            }
            CollabError::Missing(id) => write!(f, "element {id} is not in the local scene"),
            CollabError::Duplicate(id) => write!(f, "element {id} is already in the local scene"),
        }
    }
}

impl std::error::Error for CollabError {}

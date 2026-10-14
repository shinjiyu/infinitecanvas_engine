//! 场景 CRDT 骨架。
//!
//! 绑定层在 Rust 的 [yrs](https://docs.rs/yrs)（Yjs 的兼容实现），不在 WASM，
//! 也不在浏览器里的 `yjs` 包。场景元素已经在引擎里，测试可以在 `cargo test`
//! 里起两个文档，不必先有房间服务。
//!
//! ```text
//! CollabSession A                         MemoryProvider                        CollabSession B
//! Scene ── insert/update/delete/group/ungroup ──▶ Y.Doc ── SyncStep1/SyncStep2 ──▶ Y.Doc ──▶ Scene
//!                                         │
//!                                         └── encode_state_v1 / apply_update
//!                                              （Yjs v1 字节，可交给以后的 JS provider）
//! ```
//!
//! 直接改 [`infinitecanvas_engine::Scene`] 不会进文档。本地编辑要走
//! [`CollabSession::insert`]、[`CollabSession::update`]、[`CollabSession::delete`]，
//! 以及 [`CollabSession::group`]、[`CollabSession::ungroup`]、
//! [`CollabSession::set_locked`]、[`CollabSession::set_world_transform`]。
//! 远端字节走 [`CollabSession::apply_update`] 或 [`MemoryProvider::sync`]。
//!
//! 冲突策略和限制见 `docs/design/crdt.md`。

mod error;
mod provider;
mod session;

pub use error::CollabError;
pub use provider::MemoryProvider;
pub use session::{
    CollabSession, DOC_SCHEMA_VERSION, ELEMENTS_ROOT, MAX_PEER_SLOT, META_ROOT, PEER_SLOT_BITS,
    VERSION_FIELD,
};

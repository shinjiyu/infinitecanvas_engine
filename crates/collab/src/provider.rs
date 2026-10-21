//! 进程内的 y-sync provider。
//!
//! 两端交换 [`yrs::sync::Protocol`] 的 SyncStep1 / SyncStep2，不打开 socket。

use crate::error::CollabError;
use crate::session::CollabSession;
use yrs::sync::{DefaultProtocol, Protocol};
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};

/// 内存里的双连接。
///
/// 每调用一次 [`Self::sync`]，双方都发一次 SyncStep1，并把对方回的 SyncStep2
/// 应用到自己的文档上，再交换一次 awareness（含离开时的 null 状态）。
/// 这是 y-sync 的对等握手，不是房间服务。
///
/// # 示例
///
/// ```
/// use infinitecanvas_collab::{CollabSession, MemoryProvider};
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{AnyElement, Circle, Rectangle};
///
/// let mut a = CollabSession::with_client_id(1, 1);
/// let mut b = CollabSession::with_client_id(2, 2);
/// let rect_id = a.allocate_id();
/// let circle_id = b.allocate_id();
/// a.insert(AnyElement::Rectangle(Rectangle::new(
///     rect_id,
///     Vec2::new(1.0, 1.0),
///     Vec2::new(4.0, 4.0),
/// )))?;
/// b.insert(AnyElement::Circle(Circle::new(
///     circle_id,
///     Vec2::new(9.0, 9.0),
///     3.0,
/// )))?;
/// MemoryProvider::sync(&mut a, &mut b)?;
/// assert_eq!(a.scene(), b.scene());
/// assert_eq!(a.scene().count(), 2);
/// # Ok::<(), infinitecanvas_collab::CollabError>(())
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryProvider;

impl MemoryProvider {
    /// 把两个会话同步到同一文档状态，并刷新两边的场景。
    ///
    /// # 参数
    /// - `left`: 连接 A
    /// - `right`: 连接 B
    ///
    /// # 返回
    /// 成功时两边的 [`CollabSession::scene`] 元素集合一致。`next_id` 取已有
    /// 最大 id 加 1，因此也一致。
    ///
    /// # 错误
    /// 协议编码失败，或某一端无法把合并后的元素还原成场景。
    pub fn sync(left: &mut CollabSession, right: &mut CollabSession) -> Result<(), CollabError> {
        exchange(left, right)?;
        exchange(right, left)?;
        // y-sync 的 start() 只带有 data 的 awareness；离开（null）再单独交换一次。
        exchange_awareness(left, right)?;
        exchange_awareness(right, left)?;
        Ok(())
    }
}

fn exchange(from: &mut CollabSession, to: &mut CollabSession) -> Result<(), CollabError> {
    let protocol = DefaultProtocol;
    let hello = {
        let mut encoder = EncoderV1::new();
        protocol
            .start(from.awareness(), &mut encoder)
            .map_err(|err| CollabError::Protocol(err.to_string()))?;
        encoder.to_vec()
    };
    let replies = protocol
        .handle(to.awareness_mut(), &hello)
        .map_err(|err| CollabError::Protocol(err.to_string()))?;
    if !replies.is_empty() {
        let mut encoder = EncoderV1::new();
        for reply in &replies {
            reply.encode(&mut encoder);
        }
        protocol
            .handle(from.awareness_mut(), &encoder.to_vec())
            .map_err(|err| CollabError::Protocol(err.to_string()))?;
    }
    from.refresh_scene()?;
    to.refresh_scene()?;
    Ok(())
}

fn exchange_awareness(from: &mut CollabSession, to: &mut CollabSession) -> Result<(), CollabError> {
    let bytes = from.encode_awareness_update()?;
    to.apply_awareness_update(&bytes)
}

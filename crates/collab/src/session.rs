//! 一个对等端上的场景文档。
//!
//! 本地增删改走 yrs 事务，物化回 [`Scene`] 时复用场景 JSON（#6）的解码。

use crate::error::CollabError;
use infinitecanvas_engine::math::Transform;
use infinitecanvas_engine::{AnyElement, ElementId, GroupError, Scene, SCENE_FORMAT_VERSION};
use serde_json::Value;
use yrs::sync::Awareness;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Any, Doc, Map, Out, ReadTxn, StateVector, Transact, Update};

/// 根地图，按元素 id 存放整份元素 JSON。
pub const ELEMENTS_ROOT: &str = "elements";

/// 根地图，目前只放 schema 版本。
pub const META_ROOT: &str = "meta";

/// [`META_ROOT`] 里的版本字段。值是字符串，避免 Yjs number 的浮点歧义。
pub const VERSION_FIELD: &str = "version";

/// 当前 CRDT schema。元素载荷仍是场景 JSON version 1。
pub const DOC_SCHEMA_VERSION: &str = "1";

/// [`CollabSession::allocate_id`] 里 peer slot 可用的位数。
///
/// JavaScript `Number` 能精确表示的最大整数是 `2^53 - 1`。元素 id 的低 32 位
/// 是本端序号，高位只剩 21 位给 peer slot。再宽的 id 传到 JS 会丢精度，键也会对不上。
pub const PEER_SLOT_BITS: u32 = 21;

/// [`CollabSession::with_client_id`] 接受的最大 `peer_slot`（含）。
///
/// 等于 `2^21 - 1`。和序号 `u32::MAX` 拼起来正好是 `2^53 - 1`。
pub const MAX_PEER_SLOT: u32 = (1 << PEER_SLOT_BITS) - 1;

/// 一个对等端：一份 Y.Doc，加一份由它物化出来的 [`Scene`]。
///
/// # 文档结构
///
/// ```text
/// Y.Doc
/// ├── meta:     Y.Map { version: "1" }
/// └── elements: Y.Map { "<id>": "{\"type\":\"rectangle\",\"data\":{...}}" }
/// ```
///
/// 键是元素 id 的十进制字符串。值是 [`AnyElement`] 的 JSON，和场景文件里的
/// 单个元素相同。用地图而不是数组，是为了让不同 id 的增删不必抢下标。
///
/// # 示例
///
/// ```
/// use infinitecanvas_collab::{CollabSession, MemoryProvider};
/// use infinitecanvas_engine::math::Vec2;
/// use infinitecanvas_engine::{AnyElement, Rectangle};
///
/// let mut local = CollabSession::with_client_id(1, 1);
/// let mut remote = CollabSession::with_client_id(2, 2);
/// let id = local.allocate_id();
/// local.insert(AnyElement::Rectangle(Rectangle::new(
///     id,
///     Vec2::new(1.0, 2.0),
///     Vec2::new(3.0, 4.0),
/// )))?;
/// MemoryProvider::sync(&mut local, &mut remote)?;
/// assert_eq!(local.scene(), remote.scene());
/// assert_eq!(remote.scene().count(), 1);
/// # Ok::<(), infinitecanvas_collab::CollabError>(())
/// ```
pub struct CollabSession {
    awareness: Awareness,
    /// [`Self::allocate_id`] 从 bit 32 起的那一段，最多 [`PEER_SLOT_BITS`] 位。
    /// 并发分配的两端必须不同。
    peer_slot: u32,
    /// [`Self::allocate_id`] 的低 32 位，从 1 递增。
    local_seq: u32,
    scene: Scene,
}

impl CollabSession {
    /// 用随机的 yrs client id 创建一个空会话。
    ///
    /// client id 和 peer slot 都从该随机 id 推导。推导出的 slot 会掩到
    /// [`PEER_SLOT_BITS`] 位，所以 [`Self::allocate_id`] 仍落在 `2^53 - 1` 以内。
    /// 要对拍两个连接，用 [`Self::with_client_id`] 显式指定，避免撞车。
    ///
    /// # 返回
    /// 空场景。`meta.version` 已经写成 [`DOC_SCHEMA_VERSION`]。
    pub fn new() -> Self {
        let doc = Doc::new();
        let peer_slot = peer_slot_from_client(doc.client_id().get());
        Self::from_doc(doc, peer_slot)
    }

    /// 创建会话，并指定 yrs client id 和元素 id 的 peer slot。
    ///
    /// # 参数
    /// - `client_id`: yrs 副本 id。同时在线的两端必须不同，且必须落在 53 位以内
    ///   （Yjs 的安全整数）。相同 client id 的并发写入会损坏文档。
    /// - `peer_slot`: 放进 [`Self::allocate_id`]、从 bit 32 起的那一段。必须
    ///   `<= `[`MAX_PEER_SLOT`]（21 位），两端还必须不同，否则可能分到同一个元素
    ///   id，随后按 last-write-wins 收成一个元素。
    ///
    /// # 返回
    /// 空场景会话。
    ///
    /// # Panics
    /// `peer_slot` 大于 [`MAX_PEER_SLOT`] 时 panic。超过 21 位的 slot 会让元素 id
    /// 大于 `2^53 - 1`，JavaScript `Number` 无法精确保存。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::{CollabSession, MAX_PEER_SLOT};
    ///
    /// let session = CollabSession::with_client_id(1, 1);
    /// assert_eq!(session.client_id(), 1);
    /// assert_eq!(session.peer_slot(), 1);
    /// assert!(session.scene().is_empty());
    /// assert_eq!(MAX_PEER_SLOT, (1u32 << 21) - 1);
    /// ```
    pub fn with_client_id(client_id: u64, peer_slot: u32) -> Self {
        assert!(
            peer_slot <= MAX_PEER_SLOT,
            "peer_slot {peer_slot} exceeds {PEER_SLOT_BITS} bits; allocate_id would not fit in a JS safe integer (2^53-1)"
        );
        Self::from_doc(Doc::with_client_id(client_id), peer_slot)
    }

    /// yrs 副本 id。
    ///
    /// # 返回
    /// 构造时传入的 client id，或 [`Self::new`] 随机到的 id。
    pub fn client_id(&self) -> u64 {
        self.doc().client_id().get()
    }

    /// 元素 id 里序号以上的那一段。
    ///
    /// # 返回
    /// 构造时的 `peer_slot`，范围是 `0..=`[`MAX_PEER_SLOT`]。
    pub fn peer_slot(&self) -> u32 {
        self.peer_slot
    }

    /// 分配一个还没在本会话用过的元素 id。
    ///
    /// # 返回
    /// `(peer_slot << 32) | seq`。`seq` 从 1 开始，用尽 `u32` 后回绕。
    /// `peer_slot` 最多 [`PEER_SLOT_BITS`] 位，所以结果始终 `<= 2^53 - 1`。
    /// 这不是 CRDT 计数器，只保证本会话内部不重复；跨端唯一靠 peer slot。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::CollabSession;
    ///
    /// let mut session = CollabSession::with_client_id(1, 1);
    /// let first = session.allocate_id();
    /// let second = session.allocate_id();
    /// assert_eq!(first, (1u64 << 32) | 1);
    /// assert_eq!(second, (1u64 << 32) | 2);
    /// ```
    pub fn allocate_id(&mut self) -> ElementId {
        let seq = self.local_seq;
        self.local_seq = self.local_seq.wrapping_add(1);
        (u64::from(self.peer_slot) << 32) | u64::from(seq)
    }

    /// 当前文档物化出的场景。
    ///
    /// # 返回
    /// 最近一次本地事务或远端更新成功物化后的场景。选中标记总是 `false`。
    /// 下一次 `&mut self` 的写入会换掉这份场景，不要把元素引用留过那次调用。
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// 把元素插入文档。
    ///
    /// # 参数
    /// - `element`: 要同步的元素。`selected` 会在写入前清掉。id 必须尚不在本地场景里。
    ///
    /// # 错误
    /// 本地已经有这个 id 时返回 [`CollabError::Duplicate`]。JSON 编码失败时返回
    /// [`CollabError::Decode`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::CollabSession;
    /// use infinitecanvas_engine::math::Vec2;
    /// use infinitecanvas_engine::{AnyElement, Rectangle};
    ///
    /// let mut session = CollabSession::with_client_id(1, 1);
    /// let id = session.allocate_id();
    /// let mut rect = Rectangle::new(id, Vec2::ZERO, Vec2::new(2.0, 2.0));
    /// rect.props.selected = true;
    /// session.insert(AnyElement::Rectangle(rect))?;
    /// assert!(!session.scene().get(id).unwrap().as_element().props().selected);
    /// # Ok::<(), infinitecanvas_collab::CollabError>(())
    /// ```
    pub fn insert(&mut self, element: AnyElement) -> Result<(), CollabError> {
        let id = element.id();
        if self.scene.contains(id) {
            return Err(CollabError::Duplicate(id));
        }
        self.write_element(element)
    }

    /// 用整份元素 JSON 替换已有元素。
    ///
    /// # 参数
    /// - `element`: 新内容，id 必须已经在本地场景里。这是整键替换，不是字段合并。
    ///
    /// # 错误
    /// 本地没有这个 id 时返回 [`CollabError::Missing`]。
    pub fn update(&mut self, element: AnyElement) -> Result<(), CollabError> {
        let id = element.id();
        if !self.scene.contains(id) {
            return Err(CollabError::Missing(id));
        }
        self.write_element(element)
    }

    /// 把已有元素收成一组，并把组、`parent`、`children` 和世界变换写进文档。
    ///
    /// 组 id 来自 [`Self::allocate_id`]，不用 `Scene::next_id`。这样两个对等端同时成组
    /// 不会抢到同一个 `max_id + 1`。子元素的 `transform` 仍是世界变换；组自己的平移落在
    /// 子内容中心。这些字段和场景 JSON 里的 `group` 是同一份。
    ///
    /// # 参数
    /// - `ids`: 要成组的元素，顺序变成组内顺序。至少两个，且当前父节点相同。
    ///
    /// # 返回
    /// 新组的元素 id。
    ///
    /// # 错误
    /// 场景拒绝成组（不足两个、未知元素、不同父节点、会成环、已锁定）时返回
    /// [`CollabError::Group`]。文档保持原样。组 id 和已有元素冲突时返回
    /// [`CollabError::Duplicate`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::{CollabSession, MemoryProvider};
    /// use infinitecanvas_engine::math::Vec2;
    /// use infinitecanvas_engine::{AnyElement, Rectangle, Text};
    ///
    /// let mut local = CollabSession::with_client_id(1, 1);
    /// let mut remote = CollabSession::with_client_id(2, 2);
    /// let rect_id = local.allocate_id();
    /// let text_id = local.allocate_id();
    /// local.insert(AnyElement::Rectangle(Rectangle::new(
    ///     rect_id,
    ///     Vec2::ZERO,
    ///     Vec2::new(10.0, 10.0),
    /// )))?;
    /// local.insert(AnyElement::Text(Text::new(
    ///     text_id,
    ///     Vec2::new(30.0, 0.0),
    ///     "Hi",
    /// )))?;
    /// let group_id = local.group(&[rect_id, text_id])?;
    /// local.set_locked(group_id, true)?;
    /// MemoryProvider::sync(&mut local, &mut remote)?;
    /// assert_eq!(remote.scene().parent_of(rect_id), Some(group_id));
    /// assert_eq!(remote.scene().children_of(group_id), vec![rect_id, text_id]);
    /// assert!(remote.scene().get(group_id).unwrap().as_element().props().locked);
    /// # Ok::<(), infinitecanvas_collab::CollabError>(())
    /// ```
    pub fn group(&mut self, ids: &[ElementId]) -> Result<ElementId, CollabError> {
        let mut scratch = self.snapshot()?;
        let auto_id = scratch.group(ids).map_err(group_error)?;
        let group_id = self.allocate_id();
        if group_id != auto_id {
            scratch = reassign_id(&scratch, auto_id, group_id)?;
        }
        self.commit_scene(&scratch)?;
        Ok(group_id)
    }

    /// 解散组。组键从文档删除，子元素留在场景里，父指针改到组原来的父节点。
    ///
    /// 世界变换不改。锁定的组要先 [`Self::set_locked`] 解开，否则场景拒绝解散。
    ///
    /// # 参数
    /// - `group_id`: 组元素 id。
    ///
    /// # 返回
    /// 原来的直接子元素 id，顺序与组内一致。
    ///
    /// # 错误
    /// 不是组、元素不存在或已锁定时返回 [`CollabError::Group`]。文档保持原样。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::{CollabSession, MemoryProvider};
    /// use infinitecanvas_engine::math::Vec2;
    /// use infinitecanvas_engine::{AnyElement, Rectangle};
    ///
    /// let mut local = CollabSession::with_client_id(1, 1);
    /// let mut remote = CollabSession::with_client_id(2, 2);
    /// let left = local.allocate_id();
    /// let right = local.allocate_id();
    /// local.insert(AnyElement::Rectangle(Rectangle::new(
    ///     left,
    ///     Vec2::ZERO,
    ///     Vec2::new(8.0, 8.0),
    /// )))?;
    /// local.insert(AnyElement::Rectangle(Rectangle::new(
    ///     right,
    ///     Vec2::new(20.0, 0.0),
    ///     Vec2::new(8.0, 8.0),
    /// )))?;
    /// let group_id = local.group(&[left, right])?;
    /// MemoryProvider::sync(&mut local, &mut remote)?;
    /// let children = local.ungroup(group_id)?;
    /// MemoryProvider::sync(&mut local, &mut remote)?;
    /// assert_eq!(children, vec![left, right]);
    /// assert!(remote.scene().parent_of(left).is_none());
    /// assert!(!remote.scene().contains(group_id));
    /// # Ok::<(), infinitecanvas_collab::CollabError>(())
    /// ```
    pub fn ungroup(&mut self, group_id: ElementId) -> Result<Vec<ElementId>, CollabError> {
        let mut scratch = self.snapshot()?;
        let children = scratch.ungroup(group_id).map_err(group_error)?;
        self.commit_scene(&scratch)?;
        Ok(children)
    }

    /// 设置锁定，并把 `props.locked` 写进该元素的 JSON。
    ///
    /// 锁定会随整份元素同步。对端物化后，组和它的子孙都拒绝变换，和本地场景一样。
    ///
    /// # 参数
    /// - `id`: 元素 id
    /// - `locked`: `true` 为锁定，`false` 为解锁
    ///
    /// # 错误
    /// 本地没有这个 id 时返回 [`CollabError::Missing`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::CollabSession;
    /// use infinitecanvas_engine::math::Vec2;
    /// use infinitecanvas_engine::{AnyElement, Rectangle};
    ///
    /// let mut session = CollabSession::with_client_id(1, 1);
    /// let id = session.allocate_id();
    /// session.insert(AnyElement::Rectangle(Rectangle::new(
    ///     id,
    ///     Vec2::ZERO,
    ///     Vec2::new(4.0, 4.0),
    /// )))?;
    /// session.set_locked(id, true)?;
    /// assert!(session.scene().get(id).unwrap().as_element().props().locked);
    /// # Ok::<(), infinitecanvas_collab::CollabError>(())
    /// ```
    pub fn set_locked(&mut self, id: ElementId, locked: bool) -> Result<(), CollabError> {
        if !self.scene.contains(id) {
            return Err(CollabError::Missing(id));
        }
        let mut scratch = self.snapshot()?;
        if !scratch.set_locked(id, locked) {
            return Err(CollabError::Missing(id));
        }
        self.commit_scene(&scratch)
    }

    /// 设置世界变换，并把组的子孙变换一起写进文档。
    ///
    /// 目标是组时，后代按 `new = to * inverse(from) * child` 跟着变，和
    /// [`infinitecanvas_engine::Scene::set_world_transform`] 相同。只改本地 `Scene`
    /// 不会进文档；必须走这里，对端的世界坐标才会一起变。
    ///
    /// # 参数
    /// - `id`: 元素 id
    /// - `transform`: 该元素的新世界变换
    ///
    /// # 错误
    /// 元素不存在，或它（或祖先）已锁定时返回 [`CollabError::Group`]。文档保持原样。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::{CollabSession, MemoryProvider};
    /// use infinitecanvas_engine::math::{Transform, Vec2};
    /// use infinitecanvas_engine::{AnyElement, Rectangle};
    ///
    /// let mut local = CollabSession::with_client_id(1, 1);
    /// let mut remote = CollabSession::with_client_id(2, 2);
    /// let id = local.allocate_id();
    /// local.insert(AnyElement::Rectangle(Rectangle::new(
    ///     id,
    ///     Vec2::ZERO,
    ///     Vec2::new(4.0, 4.0),
    /// )))?;
    /// local.set_world_transform(id, Transform::from_translation(Vec2::new(6.0, 7.0)))?;
    /// MemoryProvider::sync(&mut local, &mut remote)?;
    /// let translation = remote
    ///     .scene()
    ///     .get(id)
    ///     .unwrap()
    ///     .as_element()
    ///     .props()
    ///     .transform
    ///     .translation;
    /// assert_eq!(translation, Vec2::new(6.0, 7.0));
    /// # Ok::<(), infinitecanvas_collab::CollabError>(())
    /// ```
    pub fn set_world_transform(
        &mut self,
        id: ElementId,
        transform: Transform,
    ) -> Result<(), CollabError> {
        let mut scratch = self.snapshot()?;
        scratch
            .set_world_transform(id, transform)
            .map_err(group_error)?;
        self.commit_scene(&scratch)
    }

    /// 从文档删除元素。
    ///
    /// # 参数
    /// - `id`: 元素 id。本地没有它时文档不变。
    ///
    /// # 返回
    /// 删除前键存在则为 `true`。
    ///
    /// # 错误
    /// 删除之后无法把文档物化回场景时返回错误，本地场景保持上一次成功的快照。
    pub fn delete(&mut self, id: ElementId) -> Result<bool, CollabError> {
        let removed = {
            let elements = self.doc().get_or_insert_map(ELEMENTS_ROOT);
            let mut txn = self.doc().transact_mut();
            elements.remove(&mut txn, &id.to_string())
        };
        self.refresh_scene()?;
        Ok(removed.is_some())
    }

    /// 编码整份文档的 Yjs v1 更新（相对空 state vector）。
    ///
    /// # 返回
    /// 与 JavaScript `Y.encodeStateAsUpdate(doc)` 相同的字节。另一个
    /// [`CollabSession::apply_update`] 或 Yjs 文档可以吃下这些字节。
    pub fn encode_state_v1(&self) -> Vec<u8> {
        let txn = self.doc().transact();
        txn.encode_state_as_update_v1(&empty_state_vector())
    }

    /// 编码对方还没看到的 Yjs v1 增量。
    ///
    /// # 参数
    /// - `remote_state_vector`: [`Self::state_vector_v1`] 的输出。
    ///
    /// # 返回
    /// 对方缺失的更新。
    ///
    /// # 错误
    /// state vector 不是合法的 v1 编码时返回 [`CollabError::Decode`]。
    pub fn encode_diff_v1(&self, remote_state_vector: &[u8]) -> Result<Vec<u8>, CollabError> {
        let state_vector = StateVector::decode_v1(remote_state_vector)
            .map_err(|err| CollabError::Decode(err.to_string()))?;
        Ok(self.doc().transact().encode_diff_v1(&state_vector))
    }

    /// 编码本副本已经整合的 state vector。
    ///
    /// # 返回
    /// Yjs v1 state vector 字节，可传给另一端的 [`Self::encode_diff_v1`]。
    pub fn state_vector_v1(&self) -> Vec<u8> {
        self.doc().transact().state_vector().encode_v1()
    }

    /// 把远端的 Yjs v1 更新应用到本地文档，并刷新场景。
    ///
    /// # 参数
    /// - `update`: [`Self::encode_state_v1`] 或 [`Self::encode_diff_v1`] 的字节。
    ///
    /// # 错误
    /// 字节不合法、yrs 拒绝更新，或更新里的元素无法还原成场景。
    /// 还原失败时文档已经吃进更新，但 [`Self::scene`] 仍是上一份成功快照。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::CollabSession;
    /// use infinitecanvas_engine::math::Vec2;
    /// use infinitecanvas_engine::{AnyElement, Rectangle};
    ///
    /// let mut source = CollabSession::with_client_id(1, 1);
    /// let id = source.allocate_id();
    /// source.insert(AnyElement::Rectangle(Rectangle::new(
    ///     id,
    ///     Vec2::new(8.0, 8.0),
    ///     Vec2::new(2.0, 2.0),
    /// )))?;
    /// let mut replay = CollabSession::with_client_id(2, 2);
    /// replay.apply_update(&source.encode_state_v1())?;
    /// assert_eq!(source.scene(), replay.scene());
    /// # Ok::<(), infinitecanvas_collab::CollabError>(())
    /// ```
    pub fn apply_update(&mut self, update: &[u8]) -> Result<(), CollabError> {
        let decoded =
            Update::decode_v1(update).map_err(|err| CollabError::Decode(err.to_string()))?;
        {
            let mut txn = self.doc().transact_mut();
            txn.apply_update(decoded)
                .map_err(|err| CollabError::Update(err.to_string()))?;
        }
        self.refresh_scene()
    }

    pub(crate) fn awareness(&self) -> &Awareness {
        &self.awareness
    }

    pub(crate) fn awareness_mut(&mut self) -> &mut Awareness {
        &mut self.awareness
    }

    pub(crate) fn refresh_scene(&mut self) -> Result<(), CollabError> {
        self.scene = self.materialize()?;
        Ok(())
    }

    fn from_doc(doc: Doc, peer_slot: u32) -> Self {
        debug_assert!(
            peer_slot <= MAX_PEER_SLOT,
            "peer_slot must fit in {PEER_SLOT_BITS} bits"
        );
        let meta = doc.get_or_insert_map(META_ROOT);
        {
            let mut txn = doc.transact_mut();
            meta.try_update(&mut txn, VERSION_FIELD, DOC_SCHEMA_VERSION);
        }
        doc.get_or_insert_map(ELEMENTS_ROOT);
        let mut session = Self {
            awareness: Awareness::new(doc),
            peer_slot,
            local_seq: 1,
            scene: Scene::new(),
        };
        session
            .refresh_scene()
            .expect("fresh CRDT document materializes to an empty scene");
        session
    }

    fn doc(&self) -> &Doc {
        self.awareness.doc()
    }

    fn write_element(&mut self, element: AnyElement) -> Result<(), CollabError> {
        let key = element.id().to_string();
        let json = element_to_json(&element)?;
        {
            let elements = self.doc().get_or_insert_map(ELEMENTS_ROOT);
            let mut txn = self.doc().transact_mut();
            elements.try_update(&mut txn, key.as_str(), json);
        }
        self.refresh_scene()
    }

    /// 复制一份场景，供 [`Scene::group`] 这类会改父子和变换的操作使用。
    ///
    /// 直接改 [`Self::scene`] 不会进文档，所以先在副本上算出结果，再整批写回。
    fn snapshot(&self) -> Result<Scene, CollabError> {
        let json = self
            .scene
            .to_json()
            .map_err(|err| CollabError::Persist(err.to_string()))?;
        Scene::from_json(&json).map_err(|err| CollabError::Persist(err.to_string()))
    }

    /// 把 `next` 相对当前场景的增删改写进同一个 yrs 事务，再物化。
    ///
    /// 成组会同时插入组、改写每个子元素的 `parent`。拆成多次事务的话，对端可能看到
    /// 子元素已经指向一个还不存在的组。
    fn commit_scene(&mut self, next: &Scene) -> Result<(), CollabError> {
        let mut upserts = Vec::new();
        for element in next.iter() {
            let id = element.id();
            let changed = self.scene.get(id).is_none_or(|current| current != element);
            if changed {
                upserts.push((id.to_string(), element_to_json(element)?));
            }
        }
        let removals: Vec<String> = self
            .scene
            .iter()
            .map(AnyElement::id)
            .filter(|id| !next.contains(*id))
            .map(|id| id.to_string())
            .collect();
        if upserts.is_empty() && removals.is_empty() {
            return Ok(());
        }
        {
            let elements = self.doc().get_or_insert_map(ELEMENTS_ROOT);
            let mut txn = self.doc().transact_mut();
            for (key, json) in upserts {
                elements.try_update(&mut txn, key.as_str(), json);
            }
            for key in &removals {
                elements.remove(&mut txn, key.as_str());
            }
        }
        self.refresh_scene()
    }

    fn materialize(&self) -> Result<Scene, CollabError> {
        let meta = self.doc().get_or_insert_map(META_ROOT);
        let elements = self.doc().get_or_insert_map(ELEMENTS_ROOT);
        let txn = self.doc().transact();
        match meta.get(&txn, VERSION_FIELD) {
            Some(Out::Any(Any::String(version))) if version.as_ref() == DOC_SCHEMA_VERSION => {}
            Some(Out::Any(Any::String(version))) => {
                return Err(CollabError::UnsupportedVersion(version.to_string()));
            }
            Some(_) => {
                return Err(CollabError::Decode(
                    "meta.version is not a string".to_string(),
                ));
            }
            None => {
                return Err(CollabError::Decode("missing meta.version".to_string()));
            }
        }

        let mut payloads = Vec::new();
        for (key, value) in elements.iter(&txn) {
            let json = match value {
                Out::Any(Any::String(json)) => json,
                _ => {
                    return Err(CollabError::Decode(format!(
                        "element {key} is not a JSON string"
                    )));
                }
            };
            let payload: Value = serde_json::from_str(json.as_ref())
                .map_err(|err| CollabError::Decode(err.to_string()))?;
            let id = payload_id(&payload)?;
            if key != id.to_string() {
                return Err(CollabError::KeyMismatch {
                    key: key.to_string(),
                    id,
                });
            }
            payloads.push(payload);
        }
        drop(txn);

        let document = serde_json::json!({
            "version": SCENE_FORMAT_VERSION,
            "elements": payloads,
        });
        let json =
            serde_json::to_string(&document).map_err(|err| CollabError::Decode(err.to_string()))?;
        let mut scene =
            Scene::from_json(&json).map_err(|err| CollabError::Persist(err.to_string()))?;
        clear_selection(&mut scene);
        Ok(scene)
    }
}

impl Default for CollabSession {
    fn default() -> Self {
        Self::new()
    }
}

fn peer_slot_from_client(client_id: u64) -> u32 {
    let mixed = (client_id as u32) ^ ((client_id >> 32) as u32);
    let slot = mixed & MAX_PEER_SLOT;
    if slot == 0 {
        1
    } else {
        slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_slots_stay_inside_21_bits() {
        for client_id in [0, 1, u64::from(u32::MAX), u64::MAX, 1 << 40, (1 << 53) - 1] {
            let slot = peer_slot_from_client(client_id);
            assert!(
                (1..=MAX_PEER_SLOT).contains(&slot),
                "{client_id} derived peer_slot {slot}"
            );
        }
    }
}

fn empty_state_vector() -> StateVector {
    StateVector::from_iter([])
}

fn group_error(err: GroupError) -> CollabError {
    CollabError::Group(err.to_string())
}

/// 把场景在成组时自动分配的 id 换成 [`CollabSession::allocate_id`] 的结果。
///
/// `Scene::group` 只能用自己的 `next_id`。物化后的 `next_id` 是最大 id 加 1，
/// 两端场景相同时会分到同一个组 id。这里改元素 id、`parent` 和组的 `children`。
fn reassign_id(scene: &Scene, from: ElementId, to: ElementId) -> Result<Scene, CollabError> {
    if scene.contains(to) {
        return Err(CollabError::Duplicate(to));
    }
    let mut rebuilt = Scene::new();
    for element in scene.iter() {
        let mut element = element.clone();
        rewrite_id(&mut element, from, to);
        rebuilt.add(element);
    }
    if rebuilt.count() != scene.count() || !rebuilt.contains(to) {
        return Err(CollabError::Decode(format!(
            "failed to reassign group id {from} to {to}"
        )));
    }
    Ok(rebuilt)
}

fn rewrite_id(element: &mut AnyElement, from: ElementId, to: ElementId) {
    let props = element.as_element_mut().props_mut();
    if props.id == from {
        props.id = to;
    }
    if props.parent == Some(from) {
        props.parent = Some(to);
    }
    if let Some(group) = element.as_group_mut() {
        for child in &mut group.children {
            if *child == from {
                *child = to;
            }
        }
    }
}

/// 单个元素的场景 JSON。
///
/// `AnyElement` 的每个种类都走这里，包括 [`AnyElement::Group`]。组的 `children`、
/// 每个元素 `props` 上的 `parent` / `locked`，以及世界 `transform`，都在这份 JSON 里。
/// `selected` 在写入前清掉。
fn element_to_json(element: &AnyElement) -> Result<String, CollabError> {
    let mut cloned = element.clone();
    cloned.as_element_mut().props_mut().selected = false;
    serde_json::to_string(&cloned).map_err(|err| CollabError::Decode(err.to_string()))
}

fn payload_id(value: &Value) -> Result<ElementId, CollabError> {
    value
        .get("data")
        .and_then(|data| data.get("props"))
        .and_then(|props| props.get("id"))
        .and_then(Value::as_u64)
        .ok_or_else(|| CollabError::Decode("element JSON is missing data.props.id".to_string()))
}

fn clear_selection(scene: &mut Scene) {
    let ids: Vec<ElementId> = scene.iter().map(AnyElement::id).collect();
    for id in ids {
        if let Some(element) = scene.get_mut(id) {
            element.as_element_mut().props_mut().selected = false;
        }
    }
}

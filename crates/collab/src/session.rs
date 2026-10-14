//! 一个对等端上的场景文档。
//!
//! 本地增删改走 yrs 事务，物化回 [`Scene`] 时复用场景 JSON（#6）的解码。

use crate::error::CollabError;
use infinitecanvas_engine::{AnyElement, ElementId, Scene, SCENE_FORMAT_VERSION};
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
    /// [`Self::allocate_id`] 的高 32 位。并发分配的两端必须不同。
    peer_slot: u32,
    /// [`Self::allocate_id`] 的低 32 位，从 1 递增。
    local_seq: u32,
    scene: Scene,
}

impl CollabSession {
    /// 用随机的 yrs client id 创建一个空会话。
    ///
    /// client id 和 peer slot 都从该随机 id 推导。要对拍两个连接，用
    /// [`Self::with_client_id`] 显式指定，避免撞车。
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
    /// - `peer_slot`: 放进 [`Self::allocate_id`] 的高 32 位。两端必须不同，否则
    ///   可能分到同一个元素 id，随后按 last-write-wins 收成一个元素。
    ///
    /// # 返回
    /// 空场景会话。
    ///
    /// # 示例
    ///
    /// ```
    /// use infinitecanvas_collab::CollabSession;
    ///
    /// let session = CollabSession::with_client_id(1, 1);
    /// assert_eq!(session.client_id(), 1);
    /// assert_eq!(session.peer_slot(), 1);
    /// assert!(session.scene().is_empty());
    /// ```
    pub fn with_client_id(client_id: u64, peer_slot: u32) -> Self {
        Self::from_doc(Doc::with_client_id(client_id), peer_slot)
    }

    /// yrs 副本 id。
    ///
    /// # 返回
    /// 构造时传入的 client id，或 [`Self::new`] 随机到的 id。
    pub fn client_id(&self) -> u64 {
        self.doc().client_id().get()
    }

    /// 元素 id 的高 32 位。
    ///
    /// # 返回
    /// 构造时的 `peer_slot`。
    pub fn peer_slot(&self) -> u32 {
        self.peer_slot
    }

    /// 分配一个还没在本会话用过的元素 id。
    ///
    /// # 返回
    /// `(peer_slot << 32) | seq`。`seq` 从 1 开始，用尽 `u32` 后回绕。
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
    if mixed == 0 {
        1
    } else {
        mixed
    }
}

fn empty_state_vector() -> StateVector {
    StateVector::from_iter([])
}

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

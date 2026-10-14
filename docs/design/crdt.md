# 场景 CRDT

`infinitecanvas-collab` 把场景元素放进一份 Yjs 兼容文档。绑定层是 Rust 的 [yrs](https://docs.rs/yrs) 0.28，不是浏览器里的 `yjs` 包，也没有链进 `infinitecanvas-wasm`。

```text
调用方
  │ insert / update / delete
  ▼
CollabSession ──▶ Y.Doc
  │                   │  encode_state_v1 / SyncStep1
  │                   ▼
  │              MemoryProvider 或外部字节通道
  │                   │  apply_update / SyncStep2
  ▼                   ▼
Scene ◀── from_json ── Y.Doc
```

现有的 `packages/server` 仍只广播光标、选区和操作。它不读这份文档。

## 为什么是 yrs，以及为什么是地图

场景和 JSON 解码都在 `infinitecanvas-engine`。yrs 和 Yjs 使用同一套 v1 更新字节，`cargo test` 里就能起两个文档，不必先做房间服务。JS 侧以后只要转发 `encode_state_v1` 的字节。

元素放在根 `Y.Map` `elements` 里，键是 id 的十进制字符串，值是一条场景 JSON 元素（`{"type","data"}`）。不同 id 的增删不用抢数组下标。根 `Y.Map` `meta` 只有 `version: "1"`。

值是整段 JSON，而不是每个坐标一个 CRDT 字段。这样载荷和场景文件是同一份解码路径。`text` 和 `group` 都已经在 version 1 里，不必再为它们维护一套字段树。

## 冲突策略

| 并发情况 | 结果 |
| --- | --- |
| 不同 id 的增、删、改 | 都保留。操作可交换 |
| 同一个 id 的两次写入 | 该键 last-write-wins。yrs 用 client id 的资历决定先后，不是墙钟，也不是字段合并。赢的是整份 JSON |
| 同一个 id 一边删除、一边更新 | 仍是该键的 last-write-wins。删除不一定压过更新 |
| 两个端用了同一个 id | 收成一个元素，内容按上一行决定 |

物化时 `next_id` 取已有最大 id 加 1（和 `Scene::from_json` 一样），不单独同步一个计数器。

## 已知限制

- 没有生产级房间、权限、断线重连或历史回放 UI。`MemoryProvider` 只在进程内走 y-sync 握手。
- 光标和选区不进文档。写入前会清掉 `selected`，物化后也是 `false`。撤销栈、脏标记、顶点缓存同样不在文档里。
- 直接改 `Scene` 不会写进 Y.Doc。要走 `CollabSession`。
- `allocate_id` 的形式是 `(peer_slot << 32) | seq`。`seq` 占低 32 位，所以 `peer_slot` 只能用剩下的 21 位（`0..=2_097_151`，常量 `MAX_PEER_SLOT`）。`with_client_id` 会断言这个上限。这样元素 id 始终 `<= 2^53 - 1`，放进 JavaScript `Number` 不会丢精度。两端 slot 相同就可能撞 id。不要用引擎的 `Scene::next_id` 给并发的两端发号。`CollabSession::new` 从 client id 推导 slot 时也会掩到这 21 位。
- 图片像素不在场景 JSON 里，因此也不在 CRDT 里。Text 的 `content`、`font_size`、`color`、`align`、`font_family`、`size` 会同步。`texture_id`、LOD、`revision`、`editing` 是 `#[serde(skip)]`，对端物化后 `texture_id` 为空，按自己的缩放重新光栅化。
- 未知元素种类会让物化失败，和 JSON 导入一样。失败时文档已经吃进更新，`scene()` 仍留着上一份成功快照。`text` 和 `group` 是场景 JSON v1 的种类，整段 JSON 会一起同步。
- 每次本地事务或远端更新成功后，都会把整份 `elements` 地图解码成 `Scene`，代价是 O(n)。按元素增量 observe 可以以后再做。
- 这个 crate 用 yrs 的 `Awareness` 做 y-sync。`Awareness::new` 不覆盖 `wasm32-unknown-unknown`，所以骨架只在宿主进程里编译。

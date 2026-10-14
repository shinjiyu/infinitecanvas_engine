# 项目进度追踪

**更新时间**: 2026-10-14

## Issue #45: 层级快捷键

- [x] `ReorderSelectionCommand`：置顶 / 置底 / 前移 / 后移，同一父节点下的兄弟，一条历史
- [x] 选中组只移动组根；跨父或空选不入栈；锁定（含祖先）跳过，隐藏仍可重排
- [x] `structure_epoch` 前进，顶点缓存整表重排；不把元素标成几何脏
- [x] WASM：`reorder_selection(action)`，`front` / `back` / `forward` / `backward`
- [x] Demo：图层面板四个按钮，以及 `]` / `[` / Ctrl+] / Ctrl+[（或 Cmd）
- [ ] #26 快捷键帮助面板尚未合并，未登记进 `SHORTCUTS`

```text
] / [ / Ctrl+] / Ctrl+[
        │
        ▼
reorder_selection ──► ReorderSelectionCommand ──► arrange_siblings
                                                      ├─ 一条历史
                                                      └─ structure_epoch（顶点缓存重排）
```

## Issue #40: 多选对齐与分布

- [x] `AlignElementsCommand`：左 / 水平居中 / 右 / 顶 / 垂直居中 / 底，参考框是目标世界包围盒的并集
- [x] `DistributeElementsCommand`：水平、垂直均匀间隙，两端不动
- [x] 只改平移；选中组移动组根；标脏；一条撤销记录。单选、空选、不足目标数不入栈
- [x] WASM：`align_selection(mode)`、`distribute_selection(axis)`
- [x] Demo：工具栏和属性面板按钮，多选时才可点。不改拖拽吸附

```text
多选 ──► nudge_targets（可见、未锁定、选中子树的根）
              │
              ├─ AlignElementsCommand ── 贴齐并集边或中心
              └─ DistributeElementsCommand ── 中间元素拉开相等间隙
                        │
                        └─ translate_element ── mark_dirty，一条历史
```

## Issue #43: 方向键微调

- [x] `NudgeElementsCommand`：顶层可见、未锁定；选中组则平移组根。只改平移，标脏并入撤销栈
- [x] 步长 1 世界单位，与缩放无关；Shift ×10。多选一条历史
- [x] 单选 / 多选 / 组、Shift、撤销重做、锁定或空选 no-op
- [x] Demo：非输入框方向键调用 `nudge_selection`；不抢空格平移、撤销、复制粘贴和工具键
- [ ] #26 快捷键帮助面板尚未合并，未登记「方向键微调 / Shift 加速」

```text
方向键 ──► nudge_targets ──► NudgeElementsCommand ──► translate_ids
                                                      ├─ 一条历史
                                                      └─ mark_dirty
```

## Issue #29: 小地图

- [x] 场景包围盒 → 小地图像素，以及像素反算世界坐标（空场景、单元素、极端宽高比）
- [x] 缩略图只画可见叶子的矩形外框；组不重复绘制
- [x] 点击跳转相机中心，拖拽视口框按像素偏移平移
- [x] `geometry_epoch` 变化才重算外框；平移/缩放只更新视口框
- [x] WASM：`scene_bounds`、`set_camera_center`、`minimap_epoch`、`minimap_element_rects`、`minimap_viewport_rect`、`minimap_to_world`
- [x] Demo 右下角小地图，`M` 显示/隐藏。滚轮停在小地图上，不缩放主画布

## 属性面板 (#35)

- [x] `SetStyle`：填充、描边、线宽、不透明度；文本额外字号和颜色
- [x] 执行 / 撤销 / 重做调用 `Scene::mark_dirty`；文本字号、颜色、描边会重新光栅化
- [x] 多选不一致时 `selection_style` 返回 mixed，一次补丁写到全部目标
- [x] 滑块预览只在松手时入栈一条历史
- [x] 样式经场景 JSON 往返；旧 JSON 用 `serde(default)` 导入
- [x] Demo 右侧属性面板，未选中时隐藏

```text
属性面板 ──preview / commit──► SetStyle ──► mark_dirty
                                    │
                                    ├─ 形状：ElementStyle → 顶点缓存
                                    └─ 文本：字号/颜色/描边 → 重新光栅
```

## Issue #33: 复制 / 粘贴 / 克隆

- [x] `Scene::duplicate`：新 ID、样式和变换保留、z 整块高于现有元素
- [x] 组子树一起复制，`parent` / `children` 按新 ID 重映射；子元素克隆仍挂在原组下
- [x] 剪贴板复用 version 1 JSON；`paste_json` 可把包围盒中心对准焦点
- [x] `DuplicateElementsCommand` / `PasteElementsCommand`：撤销删除，重做按原 ID 恢复
- [x] 新元素进入 `Scene::mark_dirty`
- [x] Demo：Ctrl+C / Ctrl+V / Ctrl+D；系统剪贴板失败退回内存，不抛错
- [x] #26 快捷键表尚未合并，三个快捷键写在 `www/main.js`

---

## Issue #4: 文本元素

- [x] Text 元素：内容、字号、颜色、对齐
- [x] Canvas2D / OffscreenCanvas 光栅化，复用 TextureManager
- [x] 按缩放 LOD 重新光栅化
- [x] T 工具单击创建，双击 DOM 编辑
- [x] 命中测试 + 创建/改内容撤销
- [x] Headless Chrome 光栅化：`Hello` / `你` / `好` 都有墨迹，中英文字形不同，1× 与 2× 的世界尺寸一致
- [x] 版本 1 JSON 往返：内容、字号、颜色、对齐、旋转、缩放；光栅字段 `#[serde(skip)]`
- [x] PNG 用 font8x8 点阵画文本（无点阵的字符画方框）
- [x] 双击计时改用 `web_time::Instant`
- [x] 创建、改内容、光栅提交、撤销/重做调用 `Scene::mark_dirty`，顶点缓存在文本变化后更新

---

## M1: 核心渲染引擎 (11/11) - ✅ 已完成

- [x] M1.1: Rust WASM 基础框架
- [x] M1.2: 基础数学库
- [x] M1.3: 视口管理器
- [x] M1.4: Canvas 2D 渲染器（临时实现）
- [x] M1.5: 元素数据结构
- [x] M1.6: 场景容器
- [x] M1.7: 渲染循环
- [x] M1.8: 背景网格
- [x] M1.9: WASM-JS 接口
- [x] M1.10: Web 集成测试
- [x] M1.11: WebGPU 渲染器完整迁移

**WebGPU 迁移进度**:
- [x] 阶段 1: 修复核心数据结构（Color, Vertex, Mat4）
- [x] 阶段 2: 添加顶点生成（Rectangle, Circle）
- [x] 阶段 3: WebGPU 渲染器框架（已测试，60 FPS）
- [x] 阶段 4: 基础 Shader (basic.wgsl)（已测试，矩形和圆形正常渲染）
- [x] 阶段 5: 图片/纹理支持（✅ 已完成）
  - [x] 设计架构文档
  - [x] 修复 wgpu 27 API 兼容性（write_texture）✅ 解题记录已添加
  - [x] TextureManager 完整实现
  - [x] 纹理采样器创建
  - [x] 纹理 BindGroup 布局
  - [x] 纹理 Pipeline 创建
  - [x] 纹理 Shader (textured.wgsl)
  - [x] Vertex 扩展（支持 UV 坐标）
  - [x] Image 元素类型（含顶点生成）
  - [x] AnyElement 和 Scene 更新
  - [x] JS 互操作接口 (upload_texture, add_image)
  - [x] 纹理渲染逻辑集成
  - [x] 测试纹理渲染（✅ 60 FPS，所有测试通过）
- [x] 阶段 6: 测试优化（✅ vertex_offset 修复，Vite 配置优化，测试完全通过）

**状态**: ✅ M1 完成 - WebGPU 渲染引擎（含纹理支持）全部实现并通过测试

---

## M2: 交互系统 (10/10) - ✅ 已完成

- [x] M2.1: 事件系统
- [x] M2.2: 命中测试
- [x] M2.3: 选择管理器
- [x] M2.4: 框选（视觉反馈：选中高亮边框 + 框选矩形）
- [x] M2.5: 元素拖动（选中后拖动移动位置）
- [x] M2.6: 命令系统（Command trait, CommandHistory）
- [x] M2.7: 撤销/重做（Ctrl+Z/Ctrl+Y）
- [x] M2.8: 工具系统（选择/矩形/圆形/平移工具）
- [x] M2.9: 键盘快捷键（V/R/O/H + 空格临时切换）
- [x] M2.10: UI 层集成（get_state, get_selection_bounds, get_element_info）

**状态**: ✅ M2 完成 - 完整的交互系统已实现

---

## 变换手柄 (#7)

- [x] 选中框 8 个缩放手柄 + 1 个旋转手柄（单选跟元素旋转对齐，多选整组变换）
- [x] Shift 缩放保持比例，Shift 旋转按 15° 吸附
- [x] `TransformElementsCommand` 进入 `CommandHistory`，Ctrl+Z 可撤销
- [x] 元素 `rotation` 字段（`ElementProps::rotation`），命中测试走逆变换，覆盖矩形、圆、直线、路径和图片

## 直线 / 自由画笔 (#8)

- [x] `Line`（可选箭头）与 `Path`：沿采样点生成带线宽的三角形带，斜接拐角，过长则斜角
- [x] 命中测试使用点到线段距离（细线另加少量点选余量）
- [x] 工具 `L` / `P`，抬笔后写入撤销栈，可选中并拖动
- [x] WASM：`add_line`、`add_path`、`set_tool("line" | "pen")`

**依赖**: M1 完成

## 对齐与吸附 (#14)

- [x] 拖拽时用选区包围盒对齐其他可见元素的左/右/上/下边缘和水平/垂直中心
- [x] 屏幕像素阈值，除以缩放后得到世界距离；可开关
- [x] 参考线可查询（`axis` + `position`），Demo 画在画布上
- [x] 几何单测覆盖吸附结果、参考线和多选并集

```text
pointer delta ──► 未吸附包围盒 ──► snap_bounds ──► 修正 + 参考线
                                      │
                                      ▼
                              其他可见元素包围盒
```

---

## 场景持久化 (#6) - ✅ 已完成

- [x] 版本化场景 JSON（`version`、`next_id`、`elements`），缺省字段用 serde default，未知字段忽略
- [x] 覆盖矩形、圆、图片、直线（含箭头）、自由路径，以及 `transform.rotation` / `scale`
- [x] WASM `export_json` / `import_json` / `export_png`
- [x] 视口或选中区域 PNG
- [x] Demo 保存/打开与 localStorage 自动保存
- [x] JSON round-trip 与 PNG 单元测试

**依赖**: M1 完成。#7 已合并：`rotation` / `scale` 写在 `Transform` 上并参与往返。Text（#4）以 `type: "text"` 进入同一版本，`parent` / `locked` 一起往返。`dirty` 和文本光栅字段都不写入 JSON。分组（#13）用 `type: group` 写入 `children` 和 `parent`。未知字段仍会被忽略。

---

## 分组与场景树 (#13)

- [x] `Group` 元素，按顺序保存子元素 ID；`ElementProps.parent` / `locked`
- [x] `Scene::group` / `ungroup` / `enter_group` / `leave_group`
- [x] 平移组时子孙世界坐标同步；旋转和均匀缩放用 `Transform::rebase_between`（与 #7 手柄同一条写回路径）
- [x] `Scene::scene_tree()`：根 → 组 → 叶子，含可见性与锁定
- [x] 单测：成组/解散、子元素命中、组变换、场景 JSON 往返（`type: group` + `children` / `parent`）
- [x] 组平移、旋转、缩放和显隐会把子树标脏，顶点缓存只重写这些元素
- [x] 拖拽吸附修正走 `translate_ids`；选中组用叶子包围盒并集，不吸附到组本身或正在拖动的子孙
- [x] Text 可以进组，随组平移/缩放/旋转并标脏；JSON 写出 `parent` / `locked`，光栅字段仍 `serde(skip)`

---

## SVG 导出 (#25) - ✅ 已完成

- [x] `export_svg`：矩形、圆、直线/箭头、自由路径、文本，复用 PNG 的 `elements_in_paint_order`
- [x] 平移 / 缩放 / 旋转、填充、描边、线宽、透明度、z 顺序
- [x] 默认导出全部可见叶子的包围盒；没有叶子时 viewBox 为 `0 0 1 1`
- [x] `selected_only` 导出自身被选中的叶子，以及选中组下的全部可见子孙
- [x] WASM `export_svg(selected_only)`，Demo「导出 SVG」/「选区 SVG」下载 `.svg`
- [x] 图片无像素数据，SVG 中跳过并写注释。文本写成 `<text>` / `<tspan>`
- [x] 组不画几何；叶子保持世界变换。连成一段的组写成不带 transform 的 `<g id="eN">`。隐藏组通过 `is_effectively_visible` 藏住子孙，子孙不计入 viewBox

**依赖**: #6 的元素遍历。#4 已合并，文本一并导出。#13 的可见性复用 `Scene::is_effectively_visible`。选区导出会展开选中组，这一点和 PNG 只看元素自身 `selected` 不同。

---

## CRDT 骨架 (#15)

- [x] `infinitecanvas-collab`：yrs 文档，元素进根 `Y.Map` `elements`（键是 id，值是场景 JSON 的单个元素）
- [x] 本地 `insert` / `update` / `delete` 写成 Yjs 事务，远端 v1 更新物化回 `Scene`
- [x] `MemoryProvider` 做双连接 y-sync；`encode_state_v1` 可回放
- [x] 冲突策略见 `docs/design/crdt.md`（整元素 LWW；选区 / 光标 / 房间服务不在这次）
- [x] Text 经 `MemoryProvider` 同步（内容、字号、颜色），对端 `texture_id` 为空
- [x] `peer_slot` 限制在 21 位，元素 id 不超过 JS 安全整数 `2^53-1`
- [x] Group 经 `MemoryProvider` 同步：`children` / `parent` / `locked`，场景树一致；组的世界变换写到子孙；`ungroup` 后组消失、子元素世界坐标保留

```text
Scene ── 事务 ──▶ Y.Doc elements ── SyncStep1/2 ──▶ 另一份 Y.Doc ──▶ Scene
```

未接到 WASM、`www` 和 `packages/server`。Text 随场景 JSON v1 同步（内容、字号、颜色）；`texture_id` 不进文档，对端物化后为空。`CollabSession::group` / `ungroup` / `set_locked` / `set_world_transform` 把组的 `children`、元素的 `parent` / `locked` 和世界变换写进同一条整段 JSON。组 id 用 `allocate_id`。`peer_slot` 限制在 21 位，元素 id 不超过 JS 安全整数 `2^53-1`。未知种类仍会使物化失败。整场景重物化是 O(n)。

---

## 图层面板 (#27)

- [x] `Scene::move_in_outline`：兄弟换序或移入/移出组，重写 `z_index` 并推进 `structure_epoch`
- [x] `ReorderOutlineCommand` 撤销/重做父指针、子列表和绘制顺序
- [x] 锁定沿祖先生效：命中、点选、框选和拖动都会跳过
- [x] WASM：`reorder_element`、`get_structure_epoch`；`set_element_locked` 取消选中子树
- [x] Demo 图层面板：树列表、选中双向同步、拖拽、显隐/锁定、虚拟滚动
- [x] Rust 单测覆盖显隐、锁定、重排和重入组

## M3: 场景管理 (9/9) - ✅ 已完成

- [x] M3.1: 场景图树结构（#13：Group + 场景树；世界变换由组增量传播）
- [x] M3.2: 空间索引 - QuadTree（基础实现，支持点查询和范围查询）
- [x] M3.3: 视口裁剪（跳过渲染视口外元素）
- [x] M3.4: 批量渲染（顶点合并到单个 draw call，60 FPS @ 1100 元素）
- [x] M3.5: 脏标记（只重生成变化元素；平移/缩放只更新 uniform）
- [x] M3.6: 顶点缓存（持久 GPU buffer + 增量 write_buffer，含直线/路径）
- [x] M3.7: 性能监控（get_performance_stats, frame_count, vertex_count）
- [x] M3.8: 大量元素压力测试（test-stress.html 可切换 1k/10k/100k，输出帧时间）
- [x] M3.9: 虚拟化渲染（✓ 视口裁剪已提供类似功能）

### M3 详细子任务规划

**M3.1 场景图树结构**（#13）
- [x] M3.1.1: `SceneTree` / `Group`
- [x] M3.1.2: 父子关系（成组、解散、进组、出组）
- [x] M3.1.3: 组变换传播到子元素世界坐标（平移、旋转、均匀缩放）
- [x] M3.1.4: 分组 / 解组 API，以及 WASM `group_elements` / `get_scene_tree`

**M3.5 脏标记**
- [x] M3.5.1: 元素 dirty 标记，场景维护待更新 id
- [x] M3.5.2: 只重生成变化元素（矩形/圆/图片/直线/箭头/路径，以及组移动后的子孙；手柄旋转/缩放、多选与撤销）
- [x] M3.5.4: `dirty` 不进 JSON；`import_json` 后整表重建顶点
- [x] M3.5.3: 平移/缩放不重建顶点，只更新投影 uniform

**M3.6 顶点缓存**
- [x] M3.6.1: 持久 CPU/GPU 顶点缓冲，增量 write_buffer
- [x] M3.6.2: 图片纹理顶点同样走缓存

**依赖**: M1 完成

---

## M4: 多人协同 (5/10) - 🔄 进行中

- [x] M4.1: 服务端基础（Node.js + ws，房间管理，用户会话）
- [x] M4.2: 客户端 WebSocket（连接管理器，自动重连，状态UI）
- [ ] M4.3: CRDT 集成 - Yjs（#15 骨架在 `crates/collab`；y-websocket / 房间服务仍未接）
- [x] M4.4: 操作同步（创建元素广播，远程操作应用）
- [x] M4.5: 光标同步（位置广播，远程光标渲染，用户标签）
- [x] M4.6: 选择同步（状态广播，远程选择显示）
- [ ] M4.7: 权限控制
- [ ] M4.8: 冲突提示
- [ ] M4.9: 历史回放 (可选)
- [ ] M4.10: 多人测试

### M4 详细子任务规划

**M4.1 服务端基础**
- M4.1.1: 选择技术栈（Node.js + ws / Rust + tokio-tungstenite）
- M4.1.2: 创建基础 WebSocket 服务器
- M4.1.3: 实现房间（Room）管理
- M4.1.4: 实现用户会话管理

**M4.2 客户端 WebSocket**
- M4.2.1: 创建 WebSocket 连接管理器
- M4.2.2: 实现自动重连逻辑
- M4.2.3: 添加连接状态 UI 指示
- M4.2.4: 实现消息队列和缓冲

**M4.3 CRDT 集成 - Yjs**
- [ ] M4.3.1: 集成 y-websocket 或 y-webrtc
- [x] M4.3.2: 设计 Y.Doc 结构映射 Scene（`elements` 地图 + 场景 JSON 字符串）
- [x] M4.3.3: 实现元素增删的 CRDT 同步（`MemoryProvider`，本地双连接）
- [x] M4.3.4: 实现元素属性的 CRDT 同步（整份元素 JSON 替换，不是字段级合并）

**M4.4 操作同步**
- M4.4.1: 定义操作消息格式（JSON Schema）
- M4.4.2: 实现本地操作广播
- M4.4.3: 实现远程操作应用
- M4.4.4: 处理操作冲突合并

**M4.5 光标同步**
- M4.5.1: 设计光标位置消息
- M4.5.2: 实现光标位置广播（节流处理）
- M4.5.3: 渲染远程用户光标
- M4.5.4: 显示用户名称标签

**M4.6 选择同步**
- M4.6.1: 广播当前选择状态
- M4.6.2: 显示其他用户选中的元素
- M4.6.3: 实现选择锁定（防止同时编辑）

**M4.7 权限控制**
- M4.7.1: 定义权限级别（只读/编辑/管理员）
- M4.7.2: 实现服务端权限验证
- M4.7.3: 客户端 UI 权限限制

**M4.8 冲突提示**
- M4.8.1: 检测编辑冲突
- M4.8.2: 显示冲突提示 UI
- M4.8.3: 提供冲突解决选项

**M4.9 历史回放（可选）**
- M4.9.1: 服务端操作日志持久化
- M4.9.2: 实现时间线回放 UI
- M4.9.3: 支持状态快照恢复

**M4.10 多人测试**
- M4.10.1: 创建多人协同测试页面
- M4.10.2: 测试 2-5 人同时编辑
- M4.10.3: 测试网络延迟和断线重连
- M4.10.4: 性能和稳定性测试

**依赖**: M1, M2 完成

---

## 总体进度

- 已完成任务: 34 / 40（主任务）
- 总体进度: 85%
- M1 核心渲染: ✅ 100% 完成
- M2 交互系统: ✅ 100% 完成
- M3 场景管理: 🔄 89% 完成（8/9）
- M4 多人协同: 🔄 50% 进行中（5/10）

---

## 测试验收指南

### 测试页面

| 页面 | URL | 用途 |
|------|-----|------|
| 框选测试 | http://localhost:5179/test-selection.html | 交互功能测试 |
| 压力测试 | http://localhost:5179/test-stress.html | 性能测试 |
| 纹理测试 | http://localhost:5179/test-texture.html | 图片渲染测试 |
| 协同测试 | http://localhost:5179/test-collaboration.html | 多人协同测试 |
| 协同画布 | http://localhost:5179/test-collab-canvas.html | 完整协同画布 |

### 运行测试

```bash
# 1. 编译 WASM
cd infinitecanvas_engine
./build-wasm.sh

# 2. 启动前端开发服务器
cd www
npm run dev  # 端口 5179

# 3. 启动协同服务器（可选，测试多人协同时需要）
cd packages/server
npm run dev  # 端口 8080

# 4. 打开浏览器访问测试页面
open http://localhost:5179/test-selection.html
open http://localhost:5179/test-collaboration.html  # 多人协同
```

### 验收标准

**M1 核心渲染（已完成 ✅）**
- [x] 矩形和圆形正确渲染
- [x] 图片纹理正确渲染
- [x] 60 FPS 稳定运行
- [x] 视口平移和缩放正常

**M2 交互系统（已完成 ✅）**
- [x] 点击选中元素
- [x] 框选多个元素
- [x] 拖动移动元素
- [x] 撤销/重做功能
- [x] 工具切换（V/R/O/H）
- [x] 空格临时切换手型工具

**M3 场景管理（进行中 🔄）**
- [x] 1000+ 元素 60 FPS
- [x] 视口外元素不渲染
- [x] 性能统计显示
- [ ] 场景图树结构（待定）

**M4 多人协同（待开始 📋）**
- [ ] 多人同时编辑
- [ ] 光标实时同步
- [ ] 操作冲突处理

---

## 修复记录

### 2025-12-21: 基本元素绘制修复

**问题**: 矩形和圆形在画布上不显示，但顶点统计正常。

**原因**: `renderer.rs` 的 `render()` 方法中缺少了 fill_pipeline 和 stroke_pipeline 的 draw 调用，只渲染了纹理元素和选择覆盖层。

**修复**: 在 render_pass 中添加了缺失的 draw 调用。

**验证**: 
- test-selection.html: 5 个元素正常渲染
- test-stress.html: 1100 个元素 @ 60 FPS


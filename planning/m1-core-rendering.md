# M1: 核心渲染引擎

**目标**: 实现 Rust + WASM 的基础无限画布渲染能力

**周期**: 2周  
**依赖**: Rust 工具链

---

## M1.1: Rust WASM 基础框架 (1天)

### 输出
- `crates/wasm/src/lib.rs` - WASM 入口模块
- `crates/wasm/src/canvas.rs` - Canvas 封装
- `crates/engine/src/lib.rs` - 引擎核心模块

### 功能
- WASM 模块初始化
- Canvas 2D Context 获取
- 日志系统集成

### 测试点
- [ ] WASM 编译成功
- [ ] Web 页面加载 WASM 无错误
- [ ] Console 输出初始化日志
- [ ] Canvas Context 获取成功

---

## M1.2: 基础数学库 (1天)

### 输出
- `crates/math/src/vec2.rs` - 2D 向量
- `crates/math/src/transform.rs` - 变换矩阵
- `crates/math/src/rect.rs` - 矩形

### 功能
- Vec2 向量运算（加减乘除、长度、归一化）
- Transform 变换（平移、缩放、旋转）
- Rect 边界框计算

### 测试点
- [ ] 单元测试覆盖所有运算
- [ ] 变换组合正确
- [ ] 边界框相交检测准确

---

## M1.3: 视口管理器 (1天)

### 输出
- `crates/engine/src/viewport.rs` - 视口模块

### 功能
- 世界坐标 ↔ 屏幕坐标转换
- 视口平移
- 视口缩放（以鼠标位置为中心）
- 视口边界计算

### 测试点
- [ ] 坐标转换往返一致
- [ ] 平移后元素位置正确
- [ ] 缩放中心点正确
- [ ] 边界计算准确

---

## M1.4: 元素数据结构 (1天)

### 输出
- `crates/engine/src/element.rs` - 元素定义
- `crates/engine/src/element/rect.rs` - 矩形元素
- `crates/engine/src/element/circle.rs` - 圆形元素
- `crates/engine/src/element/text.rs` - 文本元素

### 功能
- Element trait 定义
- 矩形元素（位置、大小、颜色）
- 圆形元素（位置、半径、颜色）
- 文本元素（位置、内容、字体）

### 测试点
- [ ] 元素序列化/反序列化
- [ ] 边界框计算正确
- [ ] 克隆/复制正常

---

## M1.5: Canvas 2D 渲染器 (2天)

### 输出
- `crates/wasm/src/renderer.rs` - 渲染器模块
- `crates/wasm/src/renderer/rect.rs` - 矩形渲染
- `crates/wasm/src/renderer/circle.rs` - 圆形渲染
- `crates/wasm/src/renderer/text.rs` - 文本渲染

### 功能
- 清空画布
- 应用视口变换
- 绘制矩形（填充+描边）
- 绘制圆形（填充+描边）
- 绘制文本（支持字体大小、颜色）

### 测试点
- [ ] Web 页面显示图形
- [ ] 颜色渲染正确
- [ ] 描边宽度正确
- [ ] 文本对齐正确

---

## M1.6: 场景容器 (1天)

### 输出
- `crates/engine/src/scene.rs` - 场景模块

### 功能
- 添加/删除元素
- 遍历所有元素
- 查找元素（按 ID）
- 元素排序（Z-index）

### 测试点
- [ ] 添加元素成功
- [ ] 删除元素后不再显示
- [ ] ID 查找准确
- [ ] Z-index 渲染顺序正确

---

## M1.7: 渲染循环 (1天)

### 输出
- `crates/wasm/src/loop.rs` - 渲染循环

### 功能
- requestAnimationFrame 集成
- 每帧清空+重绘
- FPS 计数器
- 性能监控

### 测试点
- [ ] 稳定 60 FPS
- [ ] 动画流畅
- [ ] 无内存泄漏
- [ ] CPU 占用合理（< 30%）

---

## M1.8: 背景网格 (0.5天)

### 输出
- `crates/wasm/src/renderer/grid.rs` - 网格渲染

### 功能
- 绘制背景网格（50px 间距）
- 绘制坐标轴（X/Y 轴高亮）
- 根据缩放级别自适应显示/隐藏

### 测试点
- [ ] 网格线对齐
- [ ] 缩放时网格正确
- [ ] 原点标记清晰

---

## M1.9: WASM-JS 接口 (1天)

### 输出
- `crates/wasm/src/api.rs` - JS 接口

### 功能
- `new()` - 创建实例
- `init(canvas)` - 初始化
- `addRect(x, y, w, h, color)` - 添加矩形
- `addCircle(x, y, r, color)` - 添加圆形
- `addText(x, y, text, size, color)` - 添加文本
- `render()` - 渲染一帧

### 测试点
- [ ] JS 调用成功
- [ ] 参数传递正确
- [ ] 返回值准确
- [ ] 错误处理正常

---

## M1.10: Web 集成测试 (1天)

### 输出
- `www/demo.html` - 演示页面
- `www/demo.js` - 演示代码

### 功能
- 初始化画布
- 添加多种图形（矩形、圆形、文本）
- 拖动视口
- 滚轮缩放

### 测试点
- [ ] 页面加载正常
- [ ] 图形显示正确
- [ ] 拖动流畅
- [ ] 缩放流畅
- [ ] 1000+ 元素保持 60 FPS

---

## 验收标准

**功能完整性**:
- ✅ 能绘制矩形、圆形、文本
- ✅ 视口平移、缩放正常
- ✅ 背景网格显示

**性能要求**:
- ✅ 1000 元素 @ 60 FPS
- ✅ 拖动/缩放无卡顿
- ✅ CPU < 30%, 内存 < 50MB

**代码质量**:
- ✅ 所有模块有文档注释
- ✅ 核心逻辑有单元测试
- ✅ 无 Clippy 警告


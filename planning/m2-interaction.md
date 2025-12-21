# M2: 交互系统

**目标**: 支持用户选择、拖动、编辑元素

**周期**: 1周  
**依赖**: M1 完成

---

## M2.1: 事件系统 (1天)

### 输出
- `crates/engine/src/event.rs` - 事件定义
- `crates/wasm/src/input.rs` - 输入处理

### 功能
- 事件类型定义（MouseDown/Move/Up, Wheel, Key）
- JS 事件 → WASM 事件转换
- 事件队列管理（批量处理）
- 坐标转换（屏幕 → 世界）

### 测试点
- [ ] 事件正确传入 WASM
- [ ] 坐标转换准确
- [ ] 事件队列不丢失
- [ ] 批量处理性能好

---

## M2.2: 命中测试 (1天)

### 输出
- `crates/engine/src/hit_test.rs` - 命中测试模块

### 功能
- 点是否在矩形内
- 点是否在圆形内
- 点是否在文本边界内
- 返回命中的元素列表（按 Z-index 排序）

### 测试点
- [ ] 点击元素能正确识别
- [ ] 点击空白返回空
- [ ] 重叠元素返回最上层
- [ ] 边界情况准确

---

## M2.3: 选择管理器 (1天)

### 输出
- `crates/engine/src/selection.rs` - 选择管理

### 功能
- 单选元素
- 多选元素（Ctrl/Cmd + 点击）
- 全选（Ctrl/Cmd + A）
- 取消选择
- 选中状态渲染（高亮边框）

### 测试点
- [ ] 点击选中元素
- [ ] 选中元素有高亮
- [ ] Ctrl 多选生效
- [ ] 全选正确
- [ ] 点击空白取消选择

---

## M2.4: 框选 (1天)

### 输出
- `crates/engine/src/selection_box.rs` - 框选

### 功能
- 拖动绘制选择框
- 选择框内的元素
- 选择框渲染（虚线矩形）
- 与单选/多选兼容

### 测试点
- [ ] 拖动显示选择框
- [ ] 框内元素被选中
- [ ] 虚线样式正确
- [ ] Ctrl + 框选追加选择

---

## M2.5: 元素拖动 (1天)

### 输出
- `crates/engine/src/drag.rs` - 拖动管理

### 功能
- 拖动选中元素移动
- 多个元素同时拖动
- 拖动时实时更新位置
- 拖动结束生成 Command

### 测试点
- [ ] 拖动元素位置改变
- [ ] 多选拖动相对位置不变
- [ ] 拖动流畅（60 FPS）
- [ ] 拖动后可撤销

---

## M2.6: 命令系统 (1天)

### 输出
- `crates/engine/src/command.rs` - 命令模式
- `crates/engine/src/command/move_elements.rs` - 移动命令
- `crates/engine/src/command/add_element.rs` - 添加命令
- `crates/engine/src/command/delete_element.rs` - 删除命令

### 功能
- Command trait（execute/undo/redo）
- 移动元素命令
- 添加元素命令
- 删除元素命令
- 命令历史栈

### 测试点
- [ ] execute 执行成功
- [ ] undo 回退正确
- [ ] redo 重做正确
- [ ] 历史栈容量限制（100条）

---

## M2.7: 撤销/重做 (0.5天)

### 输出
- `crates/engine/src/history.rs` - 历史管理

### 功能
- Ctrl/Cmd + Z 撤销
- Ctrl/Cmd + Shift + Z 重做
- 历史栈管理（限制 100 条）
- 状态更新通知

### 测试点
- [ ] Ctrl+Z 撤销操作
- [ ] Ctrl+Shift+Z 重做
- [ ] 连续撤销正确
- [ ] 超过 100 条自动清理

---

## M2.8: 工具系统 (1天)

### 输出
- `crates/engine/src/tool.rs` - 工具基类
- `crates/engine/src/tool/select.rs` - 选择工具
- `crates/engine/src/tool/hand.rs` - 手型工具（拖动视口）
- `crates/engine/src/tool/rect.rs` - 矩形工具

### 功能
- Tool trait（onMouseDown/Move/Up）
- 选择工具（选择+拖动元素）
- 手型工具（拖动视口）
- 矩形工具（拖动创建矩形）
- 工具切换

### 测试点
- [ ] 工具切换生效
- [ ] 选择工具操作正确
- [ ] 手型工具拖动视口
- [ ] 矩形工具创建图形

---

## M2.9: 键盘快捷键 (0.5天)

### 输出
- `crates/wasm/src/keyboard.rs` - 键盘处理

### 功能
- Delete/Backspace 删除选中元素
- Ctrl/Cmd + A 全选
- Ctrl/Cmd + Z 撤销
- Ctrl/Cmd + Shift + Z 重做
- Space 切换手型工具

### 测试点
- [ ] Delete 删除元素
- [ ] Ctrl+A 全选
- [ ] 快捷键与工具配合正确
- [ ] Space 临时切换手型

---

## M2.10: UI 层集成 (1天)

### 输出
- `www/toolbar.js` - 工具栏组件
- `www/styles/toolbar.css` - 样式

### 功能
- 工具栏（选择、手型、矩形、圆形、文本）
- 工具高亮显示当前工具
- 属性面板（显示选中元素属性）
- 快捷键提示

### 测试点
- [ ] 工具栏显示正确
- [ ] 点击切换工具
- [ ] 选中元素显示属性
- [ ] 属性修改生效

---

## 验收标准

**功能完整性**:
- ✅ 选择、拖动、删除元素
- ✅ 框选多个元素
- ✅ 撤销/重做
- ✅ 快捷键工作正常

**交互质量**:
- ✅ 拖动流畅（60 FPS）
- ✅ 选中反馈及时（< 16ms）
- ✅ 命中测试准确

**代码质量**:
- ✅ 模块有文档注释
- ✅ 命令模式测试覆盖


























# 方向键微调

选中元素后，方向键按固定世界单位平移。步长不随视口缩放变化，用来代替鼠标精拖。

```text
Arrow / Shift+Arrow（焦点不在输入框）
        │
        ▼
nudge_targets：可见、未锁定、选中子树的根
        │
        ▼
NudgeElementsCommand ── translate_ids（只加 translation）
        │
        ├─ 选中组：组根带动子孙，只移动一次
        ├─ mark_dirty（子树）
        └─ 一条 CommandHistory
```

| 按键 | 位移 |
| --- | --- |
| 方向键 | 1 世界单位（`NUDGE_STEP`） |
| Shift + 方向键 | 10 世界单位 |

世界坐标 Y 轴向下：上为 -Y，下为 +Y。多选的全部目标合成一条历史。空选、锁定（含锁定的祖先）或不可见不入栈。撤销和重做只恢复平移，不改旋转和缩放。

拖拽吸附（#14）仍只在指针拖动里生效。方向键不读吸附阈值，也不产生对齐参考线。

Demo 在 `www` 的 `keydown` 里调用 `nudge_selection`。输入框、下拉框和 contenteditable 里的方向键留给光标。空格临时平移、Ctrl/Cmd+Z、以及 V/L/P/R/O/H/T/M 工具键不走这条路径。#26 的快捷键帮助面板还没进 master，因此没有登记到那张表。

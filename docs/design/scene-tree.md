# 分组与场景树

组是场景里的容器节点。叶子仍然各自保存世界变换，渲染不用改顶点生成。

```text
Scene
  ├─ root leaves（parent = None）
  └─ Group
       ├─ Rectangle / Circle / Image / Line / Path
       └─ Group
            └─ …
```

## 数据

| 字段 | 位置 | 作用 |
| --- | --- | --- |
| `Group.children` | 组元素 | 直接子元素 ID，顺序即大纲顺序 |
| `ElementProps.parent` | 每个元素 | 反向指针，根为 `None` |
| `ElementProps.visible` / `locked` | 每个元素 | 大纲上的眼睛和锁 |
| `SceneTree` | 查询结果 | 根 → 组 → 叶子的只读快照 |

子元素的 `transform` 是世界变换。移动、旋转或缩放组时，`Scene` 把同一增量写到每个后代（`Transform::rebase_between`）。#7 的手柄通过 `Scene::apply_world_transforms` 走这条路径。

## API

- `Scene::group` / `ungroup`：成组至少两个同一父节点下的元素；解散后子元素仍在场景中
- `Scene::enter_group` / `leave_group`：进组、出组，世界坐标不变
- `Scene::scene_tree`：层级面板数据
- 删除组会删掉整棵子树；只去掉组节点用 `ungroup`

隐藏组时，子孙的命中和绘制会被跳过，但不会改写它们自己的 `visible`。锁定会沿着祖先链生效。

## 和场景 JSON

`Scene::to_json` 把组写成 `{"type":"group","data":{...}}`。`data.children` 是直接子元素 ID，`data.props.parent` 是父元素。缺省的 `parent` 表示根，缺省的 `locked` 表示未锁定。解散前的世界坐标会随元素自己的 `transform` 一起往返。`dirty` 不写入 JSON。

## 和顶点缓存

组没有三角形。`translate_element`、`set_world_transform` 和 `set_visible` 把子树标脏，缓存只重写这些叶子。成组和解散会改变元素个数，结构版本前进，下一帧整表重排。隐藏的组在重写时不再输出子孙三角形。

## 和拖拽吸附

选中组时，对齐主体是子孙叶子的世界包围盒并集。吸附修正走 `Scene::translate_ids`，子元素世界坐标跟着变。组节点本身、正在拖动的子孙，以及祖先不可见的元素都不会成为吸附目标。文本是普通叶子：可以进组，平移、缩放和旋转与矩形同一条路径，并被标脏后重写纹理四边形。`parent` / `locked` 随场景 JSON 往返；`texture_id`、LOD、`revision`、`editing` 仍不写入文件。

# 图片元素

本地 PNG/JPEG 进场景资源表，再走现有的纹理四边形。同一份字节只解码、只上传一次。

```text
拖入 / Ctrl+V（剪贴板有图片） / add_image_bytes
        │
        ▼
  decode ── 边长 > 4096 ──► 降采样并重编码 PNG
        │
        ▼
  AssetTable ── Image.asset_id ── textured quad
        │              │
        │              └── 命中 / 选中框 / 手柄（默认锁宽高比）
        ▼
  JSON data URL（一份）    PNG 导出直接采样像素
                           SVG `<image href>` 嵌同一份 data URL
```

## 元素

| 字段 | 含义 |
| --- | --- |
| asset_id | 场景资源 id。空表示只引用已经上传的 GPU 纹理 |
| intrinsic_width / intrinsic_height | 资源像素尺寸（降采样之后） |
| size | 显示尺寸，和像素尺寸分开 |
| props.transform | 平移、旋转、缩放 |
| texture_id | GPU 采样键。资源图片里等于 asset_id |
| alpha | 不透明度 0–1。属性面板和 `set_image_alpha` 都写这里。JSON 缺省为 1。不改资源表 |
| shadow | 投影（偏移、模糊、颜色）。关闭时不写入 JSON。画在裁剪后的显示四边形上，不改命中 |
| crop | 归一化窗口 `{x, y, width, height}`，缺省整张图 `0,0,1,1`。与缺省值相同时不写入 JSON。旧文档没有这个字段仍是整张图。不改变换、显示尺寸和资源像素。命中、选中框、顶点和导出都只认这个窗口 |
| blend_mode | 混合模式，缺省 `normal`。为 `normal` 时不写入 JSON |

显示矩形被裁剪时，局部原点仍是整张图的左上角，窗口落在 `crop * size` 上。`uv_rect` 仍是贴到整张显示矩形上的纹理区域；采样时先取 `uv_rect`（缺省整张纹理），再乘上 `crop`。WebGPU 用合成后的 UV。PNG 在命中窗口之后按局部坐标采样原像素。SVG 在窗口小于整张图时把嵌套 `<svg>` 的位置和 `viewBox` 收到这个窗口上，外层变换矩阵不动。

显示尺寸默认等于像素尺寸。粘贴进来时长边超过 512 世界单位会等比缩小，像素仍按原始（或降采样后的）尺寸保存。

## 资源表

引用计数跟着元素增删。降到 0 时像素离开表；撤销命令会把这份资源暂存起来，重做再放回去。GPU 纹理用同一个 id，引用消失后从 `TextureManager` 删掉。

场景 JSON（version 1）增加可省略的 `assets`。每个资源一条 data URL。元素只写 `asset_id`。没有 `assets` 的旧文件仍能打开。

Demo 的 Ctrl+V：剪贴板里有图片文件时走上面的导入；否则粘贴元素 JSON（#33）。复制图片时 `assets[]` 跟着元素走，粘贴和克隆只增加引用，不复制第二份像素。删除组后撤销仍只恢复根节点自己的图片像素。

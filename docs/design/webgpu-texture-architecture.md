# WebGPU 纹理系统架构设计

**日期**: 2025-11-26
**状态**: 设计阶段
**目标**: 为 WebGPU 渲染器添加纹理/图片支持

---

## 1. 系统概述

### 目标功能

1. ✅ 在画布上渲染图片元素（Image Element）
2. ✅ 使用纹理填充矩形/圆形（Texture Fill）
3. ✅ 支持纹理变换（旋转、缩放、UV 映射）
4. ✅ 高性能纹理缓存和批量渲染

### 技术挑战

1. **异步加载**: 图片是异步加载的，需要状态管理
2. **GPU 内存管理**: 纹理占用 GPU 内存，需要缓存策略
3. **UV 坐标**: 顶点需要包含纹理坐标
4. **Shader 复杂度**: 需要分离纹理/非纹理渲染
5. **跨域问题**: 图片可能有 CORS 限制

---

## 2. 数据流

```
┌─────────────┐
│ JS/TS Layer │  loadImage(url)
└──────┬──────┘
       │
       ▼
┌─────────────────────┐
│  ImageLoader (JS)   │  HTMLImageElement 缓存
└──────┬──────────────┘
       │ imageData
       ▼
┌─────────────────────┐
│ TextureManager(Rust)│  wgpu::Texture 缓存
└──────┬──────────────┘
       │ texture_id
       ▼
┌─────────────────────┐
│  Image Element      │  position + size + texture_id
└──────┬──────────────┘
       │
       ▼
┌─────────────────────┐
│  Vertex Generator   │  生成带 UV 坐标的顶点
└──────┬──────────────┘
       │ vertices
       ▼
┌─────────────────────┐
│  WebGPU Renderer    │  texture pipeline + sampler
└──────┬──────────────┘
       │
       ▼
┌─────────────────────┐
│  WGSL Shader        │  texture2D + sampler
└─────────────────────┘
```

---

## 3. 核心组件

### 3.1 Vertex 结构扩展

当前 Vertex (8 bytes):
```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],  // 8 bytes
    color: [f32; 4],     // 16 bytes
}
```

**问题**: 没有 UV 坐标，无法采样纹理。

**解决方案**: 扩展为支持纹理的顶点

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],  // 8 bytes
    color: [f32; 4],     // 16 bytes
    uv: [f32; 2],        // 8 bytes - 纹理坐标
    tex_index: u32,      // 4 bytes - 纹理索引 (0 = no texture)
    _padding: u32,       // 4 bytes - 对齐到 16 字节倍数
}
// Total: 40 bytes (8+16+8+4+4)
```

**顶点布局**:
```rust
wgpu::VertexBufferLayout {
    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &[
        // position
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x2,
        },
        // color
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        // uv
        wgpu::VertexAttribute {
            offset: 24,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x2,
        },
        // tex_index
        wgpu::VertexAttribute {
            offset: 32,
            shader_location: 3,
            format: wgpu::VertexFormat::Uint32,
        },
    ],
}
```

### 3.2 TextureManager (Rust)

```rust
pub struct TextureManager {
    textures: HashMap<String, TextureEntry>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

struct TextureEntry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
    ref_count: usize,
}

impl TextureManager {
    /// 从 ImageData 创建纹理
    pub fn create_texture(
        &mut self, 
        id: String,
        data: &[u8],   // RGBA8 image data
        width: u32,
        height: u32,
    ) -> Result<(), String>;
    
    /// 获取纹理视图
    pub fn get_texture_view(&self, id: &str) -> Option<&wgpu::TextureView>;
    
    /// 删除纹理
    pub fn remove_texture(&mut self, id: &str);
}
```

**纹理创建流程**:
```rust
fn create_texture(data: &[u8], width: u32, height: u32) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Image Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    
    texture
}
```

### 3.3 Sampler (采样器)

```rust
let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    address_mode_u: wgpu::AddressMode::ClampToEdge,
    address_mode_v: wgpu::AddressMode::ClampToEdge,
    address_mode_w: wgpu::AddressMode::ClampToEdge,
    mag_filter: wgpu::FilterMode::Linear,    // 放大时线性插值
    min_filter: wgpu::FilterMode::Linear,    // 缩小时线性插值
    mipmap_filter: wgpu::FilterMode::Nearest,
    ..Default::default()
});
```

**采样模式选择**:
- **Linear**: 适合平滑图片（照片、图标）
- **Nearest**: 适合像素艺术（保持锐利边缘）

### 3.4 BindGroup 布局

**当前布局 (只有 uniform)**:
```
@group(0) @binding(0) - Uniforms (transform matrix)
```

**扩展布局 (添加纹理)**:
```
@group(0) @binding(0) - Uniforms (transform matrix)
@group(0) @binding(1) - Texture (texture2D)
@group(0) @binding(2) - Sampler (sampler)
```

**Rust 实现**:
```rust
let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    entries: &[
        // Uniform buffer
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        // Texture
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: wgpu::TextureViewDimension::D2,
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        },
        // Sampler
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ],
    label: Some("texture_bind_group_layout"),
});
```

### 3.5 Shader 修改

**当前 Shader (basic.wgsl)** - 只支持纯色:
```wgsl
@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.transform * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;  // 直接返回顶点颜色
}
```

**新 Shader (textured.wgsl)** - 支持纹理:
```wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,           // UV 坐标
    @location(3) tex_index: u32,         // 纹理索引
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,           // 传递 UV
    @location(2) @interpolate(flat) tex_index: u32,  // 传递纹理索引
};

struct Uniforms {
    transform: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var t_diffuse: texture_2d<f32>;  // 纹理
@group(0) @binding(2) var s_diffuse: sampler;          // 采样器

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.transform * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    out.uv = model.uv;
    out.tex_index = model.tex_index;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.tex_index == 0u) {
        // 无纹理，使用顶点颜色
        return in.color;
    } else {
        // 有纹理，采样纹理并与顶点颜色混合
        let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
        return tex_color * in.color;  // 支持 tint 着色
    }
}
```

**优化选项 - 使用两个 Pipeline**:

更好的做法是创建两个独立的 Pipeline:
1. **基础 Pipeline**: 用于纯色渲染（当前的 basic.wgsl）
2. **纹理 Pipeline**: 用于纹理渲染（新的 textured.wgsl）

这样可以：
- 避免在 Shader 中分支判断（提升性能）
- 简化 Vertex 结构（基础 Pipeline 不需要 UV）
- 更好的批处理（相同类型元素一起渲染）

### 3.6 Image Element

```rust
// crates/engine/src/element/image.rs
pub struct Image {
    pub props: ElementProps,
    pub size: Vec2,
    pub texture_id: String,  // 纹理 ID
    pub uv_rect: Option<Rect>,  // UV 裁剪区域 (可选)
}

impl Image {
    pub fn new(id: ElementId, position: Vec2, size: Vec2, texture_id: String) -> Self {
        Self {
            props: ElementProps {
                id,
                transform: Transform::new(position, Vec2::ONE, 0.0),
                visible: true,
                z_index: 0,
            },
            size,
            texture_id,
            uv_rect: None,  // 默认使用完整纹理 (0,0) - (1,1)
        }
    }
    
    /// 生成带纹理的顶点（4个顶点 + 6个索引 = 2个三角形）
    pub fn generate_vertices(&self, tex_index: u32) -> Vec<Vertex> {
        let pos = self.props.transform.position;
        let size = self.size;
        
        // 计算 UV 坐标
        let (uv_min, uv_max) = match &self.uv_rect {
            Some(rect) => ((rect.x, rect.y), (rect.x + rect.width, rect.y + rect.height)),
            None => ((0.0, 0.0), (1.0, 1.0)),  // 默认完整纹理
        };
        
        vec![
            // 左上
            Vertex {
                position: [pos.x, pos.y],
                color: [1.0, 1.0, 1.0, 1.0],  // 白色（不着色）
                uv: [uv_min.0, uv_min.1],
                tex_index,
                _padding: 0,
            },
            // 右上
            Vertex {
                position: [pos.x + size.x, pos.y],
                color: [1.0, 1.0, 1.0, 1.0],
                uv: [uv_max.0, uv_min.1],
                tex_index,
                _padding: 0,
            },
            // 右下
            Vertex {
                position: [pos.x + size.x, pos.y + size.y],
                color: [1.0, 1.0, 1.0, 1.0],
                uv: [uv_max.0, uv_max.1],
                tex_index,
                _padding: 0,
            },
            // 左下
            Vertex {
                position: [pos.x, pos.y + size.y],
                color: [1.0, 1.0, 1.0, 1.0],
                uv: [uv_min.0, uv_max.1],
                tex_index,
                _padding: 0,
            },
        ]
    }
}
```

---

## 4. 渲染流程

### 4.1 初始化阶段

```rust
impl WebGPURenderer {
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        // ... 现有初始化代码 ...
        
        // 创建纹理 Pipeline
        let texture_pipeline = create_texture_pipeline(&device, &surface_config);
        
        // 创建采样器
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        
        // 创建纹理管理器
        let texture_manager = TextureManager::new(device.clone(), queue.clone());
        
        Ok(Self {
            // ... 现有字段 ...
            texture_pipeline,
            sampler,
            texture_manager,
        })
    }
}
```

### 4.2 纹理上传阶段

```rust
#[wasm_bindgen]
impl InfiniteCanvas {
    /// 从 JS 接收图片数据并创建 GPU 纹理
    pub fn upload_texture(
        &mut self,
        texture_id: String,
        width: u32,
        height: u32,
        data: Vec<u8>,  // RGBA8 data
    ) -> Result<(), JsValue> {
        self.webgpu_renderer.texture_manager.create_texture(
            texture_id,
            &data,
            width,
            height,
        ).map_err(|e| JsValue::from_str(&e))
    }
}
```

**JS 端**:
```javascript
async function loadImage(canvas, textureId, url) {
    const img = new Image();
    img.crossOrigin = "anonymous";
    
    await new Promise((resolve, reject) => {
        img.onload = resolve;
        img.onerror = reject;
        img.src = url;
    });
    
    // 将图片绘制到临时 canvas 获取像素数据
    const tempCanvas = document.createElement('canvas');
    tempCanvas.width = img.width;
    tempCanvas.height = img.height;
    const ctx = tempCanvas.getContext('2d');
    ctx.drawImage(img, 0, 0);
    
    const imageData = ctx.getImageData(0, 0, img.width, img.height);
    
    // 上传到 WASM
    canvas.upload_texture(textureId, img.width, img.height, imageData.data);
}
```

### 4.3 渲染阶段

```rust
pub fn render(&mut self) -> Result<(), String> {
    // 1. 收集所有顶点，分为纯色和纹理两组
    let (solid_vertices, textured_vertices) = self.collect_vertices_grouped();
    
    // 2. 更新顶点缓冲
    self.update_vertex_buffer(&solid_vertices, &textured_vertices);
    
    // 3. 创建命令编码器
    let mut encoder = self.device.create_command_encoder(&Default::default());
    
    // 4. 获取当前帧
    let output = self.surface.get_current_texture()?;
    let view = output.texture.create_view(&Default::default());
    
    // 5. 创建渲染通道
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.95, g: 0.95, b: 0.95, a: 1.0 }),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        label: Some("render_pass"),
    });
    
    // 6. 渲染纯色元素
    render_pass.set_pipeline(&self.fill_pipeline);
    render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
    render_pass.set_vertex_buffer(0, self.solid_vertex_buffer.slice(..));
    render_pass.draw(0..solid_vertices.len() as u32, 0..1);
    
    // 7. 渲染纹理元素
    if !textured_vertices.is_empty() {
        render_pass.set_pipeline(&self.texture_pipeline);
        render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.textured_vertex_buffer.slice(..));
        render_pass.draw(0..textured_vertices.len() as u32, 0..1);
    }
    
    drop(render_pass);
    
    // 8. 提交命令
    self.queue.submit(Some(encoder.finish()));
    output.present();
    
    Ok(())
}
```

---

## 5. 实现计划

### Phase 1: 基础架构 (1天)
- [ ] 扩展 Vertex 结构添加 UV 和 tex_index
- [ ] 创建 TextureManager
- [ ] 创建 Image 元素
- [ ] 单元测试

### Phase 2: WebGPU 集成 (1天)
- [ ] 创建纹理 Pipeline
- [ ] 创建采样器
- [ ] 编写 textured.wgsl Shader
- [ ] 更新 BindGroup 布局

### Phase 3: JS 互操作 (0.5天)
- [ ] upload_texture() 接口
- [ ] add_image() 接口
- [ ] JS 图片加载工具函数

### Phase 4: 渲染流程 (1天)
- [ ] 分组顶点收集（纯色 vs 纹理）
- [ ] 多 Pipeline 渲染
- [ ] 纹理绑定和切换

### Phase 5: 测试和优化 (1天)
- [ ] 单个图片渲染测试
- [ ] 多个图片渲染测试
- [ ] 性能测试（100+ 图片）
- [ ] 内存泄漏检查

---

## 6. 性能考虑

### 6.1 纹理切换开销

**问题**: 每次切换纹理都需要重新绑定 BindGroup，开销很大。

**解决方案**: 纹理数组 (Texture Array)
```wgsl
@group(0) @binding(1) var t_diffuse: texture_2d_array<f32>;  // 数组纹理
```

这样可以一次绑定多个纹理，通过 `tex_index` 索引访问，无需切换。

### 6.2 批处理

**策略**:
1. 按纹理 ID 排序元素
2. 相同纹理的元素合并为一个 draw call
3. 使用 instanced drawing

### 6.3 Mipmap

对于会缩放的图片，启用 mipmap 可以提升性能和质量：

```rust
let texture = device.create_texture(&wgpu::TextureDescriptor {
    mip_level_count: ((width.max(height) as f32).log2().floor() as u32) + 1,
    // ...
});

// 生成 mipmap
generate_mipmaps(&device, &queue, &texture);
```

---

## 7. 已知限制

1. **纹理大小限制**: 大多数 GPU 限制单个纹理为 8192x8192
2. **纹理数量**: 建议不超过 100 个活跃纹理
3. **内存占用**: RGBA8 纹理: width × height × 4 bytes
4. **WebGL 后端**: 部分功能可能不支持（如纹理数组）

---

## 8. 后续扩展

- [ ] 纹理压缩格式支持 (BC, ASTC)
- [ ] 立方体纹理（环境贴图）
- [ ] 3D 纹理（体积纹理）
- [ ] 渲染到纹理 (Render to Texture)
- [ ] 图片滤镜 (Shader 后处理)

---

## 参考资料

- [WebGPU Spec - Texture](https://www.w3.org/TR/webgpu/#textures)
- [wgpu Texture Example](https://github.com/gfx-rs/wgpu/tree/trunk/examples/texture-arrays)
- [Learn wgpu - Textures](https://sotrh.github.io/learn-wgpu/beginner/tutorial5-textures/)














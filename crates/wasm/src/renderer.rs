//! WebGPU 渲染器
//!
//! 使用 wgpu 实现高性能 GPU 渲染

#[path = "blend_pass.rs"]
mod blend_pass;

use crate::texture_manager::TextureManager;
use infinitecanvas_engine::{
    element::Element,
    grid_vertices,
    math::{Rect, Vec2, Vertex},
    transform_handles::{HandleKind, HandleMetrics, SelectionFrame},
    visible_grid_spacing, GpuVertexWrite, Scene, VertexBufferKind, VertexCache, Viewport,
};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// 裁剪模式下的选中覆盖层。
///
/// `hide_rotate` 为 true 时缩放手柄改的是裁剪窗口，旋转手柄不画。
/// `full_frame` 是未裁的整张图，窗口缩小后用来把边拖回去。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropChrome {
    /// 整张图的外框。已经是整张图时为 `None`。
    pub full_frame: Option<SelectionFrame>,
    /// 不画旋转手柄。
    pub hide_rotate: bool,
}

/// 一帧渲染的 CPU 侧统计。GPU 完成时间不在这里。
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameRenderStats {
    /// 本帧绘制用到的顶点数（含未裁掉的世界空间顶点）
    pub vertex_count: usize,
    /// draw call 次数
    pub draw_calls: usize,
    /// 重新生成顶点的元素数。平移/缩放时为 0。
    pub regenerated_elements: usize,
    /// 本帧写入顶点缓冲的字节数。不含 64 字节 uniform。
    pub gpu_bytes_written: u64,
    /// 几何和高亮都没变，只更新了 uniform。
    pub geometry_unchanged: bool,
}

/// 视口和间距没变时，网格顶点不用重传。
#[derive(Clone, Copy, PartialEq, Eq)]
struct GridCacheKey {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    spacing: u32,
}

impl GridCacheKey {
    fn new(bounds: Rect, spacing: f32) -> Self {
        Self {
            x: bounds.x.to_bits(),
            y: bounds.y.to_bits(),
            width: bounds.width.to_bits(),
            height: bounds.height.to_bits(),
            spacing: spacing.to_bits(),
        }
    }
}

/// WebGPU 渲染器
pub struct WebGPURenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // 基础渲染管线（纯色）
    fill_pipeline: wgpu::RenderPipeline,
    stroke_pipeline: wgpu::RenderPipeline,
    /// 持久填充/粗描边缓冲。只增不减，脏元素按偏移增量写入。
    fill_buffer: wgpu::Buffer,
    fill_capacity: usize,
    /// 持久细描边缓冲。
    stroke_buffer: wgpu::Buffer,
    stroke_capacity: usize,
    /// 持久图片顶点缓冲。
    textured_buffer: wgpu::Buffer,
    textured_capacity: usize,
    /// 选中高亮。几何不变时不重传。
    selection_buffer: wgpu::Buffer,
    selection_capacity: usize,
    /// 背景网格。视口没变时不重传。
    grid_buffer: wgpu::Buffer,
    grid_capacity: usize,
    grid_vertex_count: u32,
    grid_key: Option<GridCacheKey>,
    /// CPU 侧世界空间顶点缓存。平移/缩放不重生成顶点。
    vertex_cache: VertexCache,
    selection_fill: Vec<Vertex>,
    selection_stroke: Vec<Vertex>,
    cached_selection_epoch: u64,
    cached_selection_box: Option<Rect>,
    /// JSON 导入后强制重画选中覆盖层，避免 epoch 碰巧相同而留下旧手柄。
    selection_overlay_dirty: bool,
    /// 手柄框和缩放一起决定选中覆盖层。平移不改变这两项。
    cached_transform_frame: Option<SelectionFrame>,
    /// 裁剪模式的原图外框。窗口变化时覆盖层要重画。
    cached_crop_chrome: Option<CropChrome>,
    cached_selection_zoom: f32,
    last_stats: FrameRenderStats,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // 纹理渲染管线
    texture_pipeline: wgpu::RenderPipeline,
    texture_sampler: wgpu::Sampler,
    texture_manager: TextureManager,

    // 占位纹理（用于无纹理绑定时）
    placeholder_texture: wgpu::Texture,
    placeholder_texture_view: wgpu::TextureView,
    /// 有非 Normal 叶子时才分配。全部为 Normal 时保持空，绘制路径不变。
    blend_targets: Option<blend_pass::BlendTargets>,
}

impl WebGPURenderer {
    /// 初始化 WebGPU 渲染器
    ///
    /// `SurfaceTarget::Canvas` 只在 `wasm32`（非 Emscripten）目标上存在。
    /// 原生 `cargo clippy` / `cargo test` 会走到错误分支，避免引用该变体。
    pub async fn new(
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<Self, JsValue> {
        #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
        {
            Self::new_wasm(canvas, width, height).await
        }
        #[cfg(not(all(target_arch = "wasm32", not(target_os = "emscripten"))))]
        {
            let _ = (canvas, width, height);
            Err(JsValue::from_str(
                "WebGPU canvas surface is only available when targeting wasm32",
            ))
        }
    }

    /// wasm32 上的 WebGPU 初始化实现
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    async fn new_wasm(
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
    ) -> Result<Self, JsValue> {
        log::info!("Initializing WebGPU renderer...");

        // 1. 获取 WebGPU 实例
        let instance = wgpu::Instance::default();

        // 2. 创建 Surface（使用 wgpu::SurfaceTarget::Canvas）
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {:?}", e)))?;

        // 3. 请求适配器
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to find adapter: {:?}", e)))?;

        log::info!("Adapter created: {:?}", adapter.get_info());

        // 4. 请求设备和队列（wgpu 27 API）
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("InfiniteCanvas Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::default(),
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to create device: {:?}", e)))?;

        log::info!("Device and queue created");

        // 5. 配置 Surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);
        log::info!(
            "Surface configured: {}x{}, format: {:?}",
            width,
            height,
            surface_format
        );

        // 6. 创建 Uniform Buffer（用于视口变换矩阵）
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: 64, // Mat4 = 16 * f32 = 64 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 7. 创建 Bind Group Layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // 8. 创建 Bind Group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // 9. 加载 Shader（临时内联，后续会分离）
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/basic.wgsl").into()),
        });

        // 10. 创建填充渲染管线（三角形）
        let fill_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fill Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fill Pipeline"),
            layout: Some(&fill_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
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
                        // uv (需要声明，即使basic.wgsl不使用，以保持正确的stride)
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // 11. 创建描边渲染管线（线条）
        let stroke_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stroke Pipeline"),
            layout: Some(&fill_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // uv (需要声明，即使basic.wgsl不使用，以保持正确的stride)
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // 12. 持久顶点缓冲。容量只在不够时扩大，平移/缩放不重建内容。
        let fill_capacity = 4096;
        let stroke_capacity = 4096;
        let textured_capacity = 1024;
        let selection_capacity = 256;
        let fill_buffer = create_vertex_buffer(&device, "Fill Vertex Buffer", fill_capacity);
        let stroke_buffer = create_vertex_buffer(&device, "Stroke Vertex Buffer", stroke_capacity);
        let textured_buffer =
            create_vertex_buffer(&device, "Textured Vertex Buffer", textured_capacity);
        let selection_buffer =
            create_vertex_buffer(&device, "Selection Vertex Buffer", selection_capacity);
        let grid_capacity = 1024;
        let grid_buffer = create_vertex_buffer(&device, "Grid Vertex Buffer", grid_capacity);

        // 13. 创建纹理采样器
        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear, // 放大时线性插值
            min_filter: wgpu::FilterMode::Linear, // 缩小时线性插值
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // 14. 创建占位纹理（1x1 白色纹理，用于无纹理绑定时）
        let placeholder_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // 上传白色像素到占位纹理
        queue.write_texture(
            placeholder_texture.as_image_copy(),
            &[255u8, 255, 255, 255], // RGBA 白色
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let placeholder_texture_view =
            placeholder_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 15. 创建纹理 BindGroup 布局
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    // Uniform buffer (transform)
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
            });

        // 16. 加载纹理 Shader
        let texture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Texture Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/textured.wgsl").into()),
        });

        // 17. 创建纹理渲染管线
        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Texture Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let texture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Texture Pipeline"),
            layout: Some(&texture_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &texture_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
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
                        // uv (新增)
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &texture_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // 18. 创建纹理管理器
        let device_arc = Arc::new(device);
        let queue_arc = Arc::new(queue);
        let texture_manager = TextureManager::new(device_arc.clone(), queue_arc.clone());

        log::info!("WebGPU renderer initialized successfully (with texture support)");

        Ok(Self {
            device: device_arc,
            queue: queue_arc,
            surface,
            surface_config,
            fill_pipeline,
            stroke_pipeline,
            fill_buffer,
            fill_capacity,
            stroke_buffer,
            stroke_capacity,
            textured_buffer,
            textured_capacity,
            selection_buffer,
            selection_capacity,
            grid_buffer,
            grid_capacity,
            grid_vertex_count: 0,
            grid_key: None,
            vertex_cache: VertexCache::new(),
            selection_fill: Vec::new(),
            selection_stroke: Vec::new(),
            cached_selection_epoch: 0,
            cached_selection_box: None,
            selection_overlay_dirty: false,
            cached_transform_frame: None,
            cached_crop_chrome: None,
            cached_selection_zoom: f32::NAN,
            last_stats: FrameRenderStats::default(),
            uniform_buffer,
            uniform_bind_group,
            texture_pipeline,
            texture_sampler,
            texture_manager,
            placeholder_texture,
            placeholder_texture_view,
            blend_targets: None,
        })
    }

    /// 渲染场景
    ///
    /// 顶点缓存在世界空间。元素没变时只把视口矩阵写入 uniform，不重建顶点。
    /// 直线、箭头和自由画笔路径与矩形填充共用三角形缓冲。
    ///
    /// # 参数
    /// - `scene`: 场景数据（会清除已消化的脏标记）
    /// - `viewport`: 视口。投影矩阵每帧更新；手柄尺寸按缩放换算
    /// - `selection_box`: 框选矩形（如果正在框选）
    /// - `transform_frame`: 选中框。有选中元素时带 8 个缩放手柄和 1 个旋转手柄
    /// - `grid_visible`: 为 false 时不画背景网格。网格顶点不计入场景几何是否变化
    /// - `crop_chrome`: 裁剪模式时藏起旋转手柄，并画出未裁区域的外框
    pub fn render(
        &mut self,
        scene: &mut Scene,
        viewport: &Viewport,
        selection_box: Option<Rect>,
        transform_frame: Option<SelectionFrame>,
        grid_visible: bool,
        crop_chrome: Option<CropChrome>,
    ) -> Result<(), JsValue> {
        let sync = self.vertex_cache.sync(scene);
        let selection_changed = self.sync_selection(
            scene,
            selection_box,
            transform_frame,
            viewport.zoom(),
            crop_chrome,
        );

        let geometry_grew = self.ensure_geometry_capacity();
        let selection_grew = self.ensure_selection_capacity();
        let mut gpu_bytes = 0u64;
        if geometry_grew || sync.full_upload {
            gpu_bytes += self.upload_full_geometry();
        } else if !sync.writes.is_empty() {
            gpu_bytes += self.upload_partial(&sync.writes);
        }
        if selection_changed || selection_grew {
            gpu_bytes += self.upload_selection();
        }
        let (grid_count, grid_bytes) = if grid_visible {
            self.sync_grid(viewport)
        } else {
            (0, 0)
        };
        gpu_bytes += grid_bytes;

        self.write_uniform(viewport);

        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("Failed to acquire next frame: {:?}", e)))?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let fill_count = self.vertex_cache.fill().len() as u32;
        let stroke_count = self.vertex_cache.stroke().len() as u32;
        let selection_fill_count = self.selection_fill.len() as u32;
        let selection_stroke_count = self.selection_stroke.len() as u32;
        let textured_count = self.vertex_cache.textured_draws().len();
        let mut draw_calls = 0usize;
        let blend_composite = self.vertex_cache.needs_blend_composite();
        if blend_composite {
            self.ensure_blend_targets();
        }

        if blend_composite {
            draw_calls += self.draw_blended(
                &mut encoder,
                &view,
                grid_count,
                selection_fill_count,
                selection_stroke_count,
            );
        } else {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if grid_count > 0 {
                render_pass.set_pipeline(&self.stroke_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.grid_buffer.slice(..));
                render_pass.draw(0..grid_count, 0..1);
                draw_calls += 1;
            }

            if fill_count > 0 {
                render_pass.set_pipeline(&self.fill_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.fill_buffer.slice(..));
                render_pass.draw(0..fill_count, 0..1);
                draw_calls += 1;
            }

            if stroke_count > 0 {
                render_pass.set_pipeline(&self.stroke_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.stroke_buffer.slice(..));
                render_pass.draw(0..stroke_count, 0..1);
                draw_calls += 1;
            }

            if textured_count > 0 {
                // 图片先用纯色管线铺一层。文本纹理带透明背景，纯色底会挡住字形，所以跳过。
                render_pass.set_pipeline(&self.fill_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.textured_buffer.slice(..));
                for index in 0..textured_count {
                    let draw = &self.vertex_cache.textured_draws()[index];
                    if !draw.underlay {
                        continue;
                    }
                    let end = draw.vertex_offset + draw.vertex_count;
                    render_pass.draw(draw.vertex_offset..end, 0..1);
                    draw_calls += 1;
                }

                render_pass.set_pipeline(&self.texture_pipeline);
                for index in 0..textured_count {
                    let draw = &self.vertex_cache.textured_draws()[index];
                    let texture_id = draw.texture_id.clone();
                    let range = draw.vertex_offset..draw.vertex_offset + draw.vertex_count;
                    let (texture_view, _placeholder) =
                        match self.texture_manager.get_texture_view(&texture_id) {
                            Some(view) => (view, false),
                            None => {
                                log::warn!(
                                    "[RENDER] Texture '{}' not found. Using placeholder view",
                                    texture_id
                                );
                                (&self.placeholder_texture_view, true)
                            }
                        };
                    let texture_bind_group =
                        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("Texture Bind Group: {}", texture_id)),
                            layout: &self.texture_pipeline.get_bind_group_layout(0),
                            entries: &[
                                wgpu::BindGroupEntry {
                                    binding: 0,
                                    resource: self.uniform_buffer.as_entire_binding(),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 1,
                                    resource: wgpu::BindingResource::TextureView(texture_view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 2,
                                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                                },
                            ],
                        });
                    render_pass.set_bind_group(0, &texture_bind_group, &[]);
                    render_pass.draw(range, 0..1);
                    draw_calls += 1;
                }
            }

            if selection_fill_count > 0 || selection_stroke_count > 0 {
                render_pass.set_vertex_buffer(0, self.selection_buffer.slice(..));
            }
            if selection_fill_count > 0 {
                render_pass.set_pipeline(&self.fill_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.draw(0..selection_fill_count, 0..1);
                draw_calls += 1;
            }
            if selection_stroke_count > 0 {
                render_pass.set_pipeline(&self.stroke_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                let start = selection_fill_count;
                render_pass.draw(start..start + selection_stroke_count, 0..1);
                draw_calls += 1;
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        self.last_stats = FrameRenderStats {
            vertex_count: self.vertex_cache.fill().len()
                + self.vertex_cache.stroke().len()
                + self.vertex_cache.textured().len()
                + self.selection_fill.len()
                + self.selection_stroke.len()
                + grid_count as usize,
            draw_calls,
            regenerated_elements: sync.regenerated,
            gpu_bytes_written: gpu_bytes,
            geometry_unchanged: sync.geometry_unchanged && !selection_changed,
        };
        Ok(())
    }

    /// 上一帧统计
    pub fn last_stats(&self) -> FrameRenderStats {
        self.last_stats
    }

    /// 场景被整体替换后调用。下一帧重传全部顶点，并重画选中覆盖层。
    pub fn invalidate_geometry(&mut self) {
        self.vertex_cache.invalidate();
        self.selection_overlay_dirty = true;
    }

    fn sync_selection(
        &mut self,
        scene: &Scene,
        selection_box: Option<Rect>,
        transform_frame: Option<SelectionFrame>,
        zoom: f32,
        crop_chrome: Option<CropChrome>,
    ) -> bool {
        if !self.selection_overlay_dirty
            && self.cached_selection_epoch == scene.selection_epoch()
            && self.cached_selection_box == selection_box
            && self.cached_transform_frame == transform_frame
            && self.cached_crop_chrome == crop_chrome
            && self.cached_selection_zoom.to_bits() == zoom.to_bits()
        {
            return false;
        }
        self.selection_overlay_dirty = false;
        let (fill, stroke) = self.collect_selection_overlay_vertices(
            scene,
            selection_box,
            transform_frame,
            zoom,
            crop_chrome,
        );
        self.selection_fill = fill;
        self.selection_stroke = stroke;
        self.cached_selection_epoch = scene.selection_epoch();
        self.cached_selection_box = selection_box;
        self.cached_transform_frame = transform_frame;
        self.cached_crop_chrome = crop_chrome;
        self.cached_selection_zoom = zoom;
        true
    }

    fn ensure_geometry_capacity(&mut self) -> bool {
        let fill_len = self.vertex_cache.fill().len();
        let stroke_len = self.vertex_cache.stroke().len();
        let textured_len = self.vertex_cache.textured().len();
        let mut grew = false;
        grew |= grow_buffer(
            &self.device,
            &mut self.fill_buffer,
            &mut self.fill_capacity,
            fill_len,
            "Fill Vertex Buffer",
        );
        grew |= grow_buffer(
            &self.device,
            &mut self.stroke_buffer,
            &mut self.stroke_capacity,
            stroke_len,
            "Stroke Vertex Buffer",
        );
        grew |= grow_buffer(
            &self.device,
            &mut self.textured_buffer,
            &mut self.textured_capacity,
            textured_len,
            "Textured Vertex Buffer",
        );
        grew
    }

    fn ensure_selection_capacity(&mut self) -> bool {
        let needed = self.selection_fill.len() + self.selection_stroke.len();
        grow_buffer(
            &self.device,
            &mut self.selection_buffer,
            &mut self.selection_capacity,
            needed,
            "Selection Vertex Buffer",
        )
    }

    fn upload_full_geometry(&self) -> u64 {
        let mut bytes = 0;
        bytes += write_vertices(&self.queue, &self.fill_buffer, 0, self.vertex_cache.fill());
        bytes += write_vertices(
            &self.queue,
            &self.stroke_buffer,
            0,
            self.vertex_cache.stroke(),
        );
        bytes += write_vertices(
            &self.queue,
            &self.textured_buffer,
            0,
            self.vertex_cache.textured(),
        );
        bytes
    }

    fn upload_partial(&self, writes: &[GpuVertexWrite]) -> u64 {
        let mut bytes = 0;
        for write in writes {
            let (buffer, data) = match write.kind {
                VertexBufferKind::Fill => (&self.fill_buffer, self.vertex_cache.fill()),
                VertexBufferKind::Stroke => (&self.stroke_buffer, self.vertex_cache.stroke()),
                VertexBufferKind::Textured => (&self.textured_buffer, self.vertex_cache.textured()),
            };
            let start = write.vertex_offset as usize;
            let end = start + write.vertex_count as usize;
            bytes += write_vertices(&self.queue, buffer, write.vertex_offset, &data[start..end]);
        }
        bytes
    }

    fn upload_selection(&self) -> u64 {
        let mut bytes =
            write_vertices(&self.queue, &self.selection_buffer, 0, &self.selection_fill);
        bytes += write_vertices(
            &self.queue,
            &self.selection_buffer,
            self.selection_fill.len() as u32,
            &self.selection_stroke,
        );
        bytes
    }

    /// 按当前视口生成背景网格。范围和间距都没变时不写缓冲。
    ///
    /// # 返回
    /// 顶点数量，以及这次写入的字节数。
    fn sync_grid(&mut self, viewport: &Viewport) -> (u32, u64) {
        let spacing = visible_grid_spacing(viewport.zoom());
        let bounds = viewport.bounds();
        let key = GridCacheKey::new(bounds, spacing);
        if self.grid_key == Some(key) {
            return (self.grid_vertex_count, 0);
        }
        let vertices = grid_vertices(bounds, spacing);
        let mut bytes = 0;
        if !vertices.is_empty() {
            let _grew = grow_buffer(
                &self.device,
                &mut self.grid_buffer,
                &mut self.grid_capacity,
                vertices.len(),
                "Grid Vertex Buffer",
            );
            bytes = write_vertices(&self.queue, &self.grid_buffer, 0, &vertices);
        }
        self.grid_vertex_count = vertices.len() as u32;
        self.grid_key = Some(key);
        (self.grid_vertex_count, bytes)
    }

    /// 把视口写成列主序投影矩阵。不触碰顶点缓冲。
    fn write_uniform(&self, viewport: &Viewport) {
        let width = self.surface_config.width as f32;
        let height = self.surface_config.height as f32;
        let zoom = viewport.zoom();
        let center = viewport.center();
        let sx = 2.0 * zoom / width;
        let sy = -2.0 * zoom / height;
        let tx = -center.x * sx;
        let ty = -center.y * sy;
        let projection_matrix: [[f32; 4]; 4] = [
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [tx, ty, 0.0, 1.0],
        ];
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&projection_matrix),
        );
    }

    /// 收集选中高亮覆盖层的顶点
    ///
    /// # 参数
    /// - `scene`: 场景，用于获取选中元素
    /// - `selection_box`: 框选矩形（如果正在框选）
    /// - `transform_frame`: 带手柄的选中框
    /// - `zoom`: 视口缩放，手柄保持屏幕尺寸
    ///
    /// # 返回
    /// (fill_vertices, stroke_vertices) - 填充顶点和描边顶点
    fn collect_selection_overlay_vertices(
        &self,
        scene: &Scene,
        selection_box: Option<Rect>,
        transform_frame: Option<SelectionFrame>,
        zoom: f32,
        crop_chrome: Option<CropChrome>,
    ) -> (Vec<Vertex>, Vec<Vertex>) {
        let mut fill_vertices = Vec::new();
        let mut stroke_vertices = Vec::new();

        // 选中高亮颜色（蓝色）
        let selection_color = [0.25, 0.55, 0.95, 1.0]; // #4090f2
        let selection_fill_color = [0.25, 0.55, 0.95, 0.15]; // 半透明蓝色填充
        let handle_fill = [1.0, 1.0, 1.0, 1.0];
        let crop_color = [0.95, 0.72, 0.25, 1.0];
        let hide_rotate = crop_chrome.is_some_and(|chrome| chrome.hide_rotate);
        let handle_stroke = if hide_rotate {
            crop_color
        } else {
            selection_color
        };

        if let Some(chrome) = crop_chrome {
            if let Some(full) = chrome.full_frame {
                push_frame_stroke(&mut stroke_vertices, full, crop_color);
            }
        }

        if let Some(frame) = transform_frame {
            push_frame_stroke(&mut stroke_vertices, frame, handle_stroke);
            let selected = selected_elements(scene);
            if selected.len() > 1 {
                for element in selected {
                    push_frame_stroke(
                        &mut stroke_vertices,
                        SelectionFrame::from_element(element),
                        selection_color,
                    );
                }
            }
            let metrics = HandleMetrics::from_zoom(zoom);
            for handle in frame.handles(metrics.rotate_gap) {
                match handle.kind {
                    HandleKind::Rotate if hide_rotate => {}
                    HandleKind::Rotate => {
                        push_disc(
                            &mut fill_vertices,
                            handle.position,
                            metrics.draw_half * 1.15,
                            selection_color,
                        );
                        let stem = frame.to_world(Vec2::new(0.0, -frame.half_size.y));
                        push_segment(&mut stroke_vertices, stem, handle.position, selection_color);
                    }
                    HandleKind::Resize(_) => {
                        push_handle_square(
                            &mut fill_vertices,
                            &mut stroke_vertices,
                            frame,
                            handle.position,
                            metrics.draw_half,
                            handle_fill,
                            handle_stroke,
                        );
                    }
                }
            }
        }

        // 框选矩形
        if let Some(rect) = selection_box {
            let fill = self.generate_rect_fill_vertices(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                selection_fill_color,
            );
            fill_vertices.extend(fill);

            let stroke = self.generate_rect_stroke_vertices(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                selection_color,
            );
            stroke_vertices.extend(stroke);
        }

        (fill_vertices, stroke_vertices)
    }

    /// 生成矩形填充顶点（两个三角形）
    fn generate_rect_fill_vertices(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) -> Vec<Vertex> {
        let tl = [x, y];
        let tr = [x + w, y];
        let bl = [x, y + h];
        let br = [x + w, y + h];
        let uv = [0.0, 0.0];

        vec![
            Vertex {
                position: tl,
                color,
                uv,
            },
            Vertex {
                position: tr,
                color,
                uv,
            },
            Vertex {
                position: br,
                color,
                uv,
            },
            Vertex {
                position: tl,
                color,
                uv,
            },
            Vertex {
                position: br,
                color,
                uv,
            },
            Vertex {
                position: bl,
                color,
                uv,
            },
        ]
    }

    /// 生成矩形描边顶点（四条线）
    fn generate_rect_stroke_vertices(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) -> Vec<Vertex> {
        let tl = [x, y];
        let tr = [x + w, y];
        let bl = [x, y + h];
        let br = [x + w, y + h];
        let uv = [0.0, 0.0];

        vec![
            // 上边
            Vertex {
                position: tl,
                color,
                uv,
            },
            Vertex {
                position: tr,
                color,
                uv,
            },
            // 右边
            Vertex {
                position: tr,
                color,
                uv,
            },
            Vertex {
                position: br,
                color,
                uv,
            },
            // 下边
            Vertex {
                position: br,
                color,
                uv,
            },
            Vertex {
                position: bl,
                color,
                uv,
            },
            // 左边
            Vertex {
                position: bl,
                color,
                uv,
            },
            Vertex {
                position: tl,
                color,
                uv,
            },
        ]
    }

    /// 调整 Surface 大小
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.discard_blend_targets();
            log::info!("Surface resized to {}x{}", width, height);
        }
    }

    /// 上传纹理到 GPU
    ///
    /// # 参数
    /// - `texture_id`: 纹理的唯一标识符
    /// - `width`: 图片宽度（像素）
    /// - `height`: 图片高度（像素）
    /// - `data`: RGBA8 图像数据
    ///
    /// # 错误
    /// - 如果纹理 ID 已存在
    /// - 如果数据长度不匹配
    /// - 如果纹理尺寸超过限制
    pub fn upload_texture(
        &mut self,
        texture_id: String,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), String> {
        self.texture_manager
            .create_texture(texture_id, data, width, height)
    }

    /// 获取纹理是否存在
    pub fn has_texture(&self, texture_id: &str) -> bool {
        self.texture_manager.has_texture(texture_id)
    }

    /// 删除纹理
    pub fn remove_texture(&mut self, texture_id: &str) {
        self.texture_manager.remove_texture(texture_id);
    }
}

fn selected_elements(scene: &Scene) -> Vec<&dyn Element> {
    scene
        .elements_sorted()
        .into_iter()
        .filter(|element| {
            let props = element.as_element().props();
            props.selected && props.visible
        })
        .map(|element| element.as_element())
        .collect()
}

fn push_frame_stroke(stroke: &mut Vec<Vertex>, frame: SelectionFrame, color: [f32; 4]) {
    let corners = frame.corners();
    for index in 0..4 {
        push_segment(stroke, corners[index], corners[(index + 1) % 4], color);
    }
}

fn push_handle_square(
    fill: &mut Vec<Vertex>,
    stroke: &mut Vec<Vertex>,
    frame: SelectionFrame,
    world_center: Vec2,
    half: f32,
    fill_color: [f32; 4],
    stroke_color: [f32; 4],
) {
    let local = frame.to_local(world_center);
    let corners = [
        frame.to_world(local + Vec2::new(-half, -half)),
        frame.to_world(local + Vec2::new(half, -half)),
        frame.to_world(local + Vec2::new(half, half)),
        frame.to_world(local + Vec2::new(-half, half)),
    ];
    push_quad_fill(fill, corners, fill_color);
    push_frame_corners(stroke, corners, stroke_color);
}

fn push_frame_corners(stroke: &mut Vec<Vertex>, corners: [Vec2; 4], color: [f32; 4]) {
    for index in 0..4 {
        push_segment(stroke, corners[index], corners[(index + 1) % 4], color);
    }
}

fn push_quad_fill(fill: &mut Vec<Vertex>, corners: [Vec2; 4], color: [f32; 4]) {
    let uv = [0.0, 0.0];
    let vertex = |point: Vec2| Vertex {
        position: [point.x, point.y],
        color,
        uv,
    };
    fill.extend([
        vertex(corners[0]),
        vertex(corners[1]),
        vertex(corners[2]),
        vertex(corners[0]),
        vertex(corners[2]),
        vertex(corners[3]),
    ]);
}

fn push_segment(stroke: &mut Vec<Vertex>, from: Vec2, to: Vec2, color: [f32; 4]) {
    let uv = [0.0, 0.0];
    stroke.push(Vertex {
        position: [from.x, from.y],
        color,
        uv,
    });
    stroke.push(Vertex {
        position: [to.x, to.y],
        color,
        uv,
    });
}

fn push_disc(fill: &mut Vec<Vertex>, center: Vec2, radius: f32, color: [f32; 4]) {
    let uv = [0.0, 0.0];
    let segments = 12;
    for index in 0..segments {
        let a0 = (index as f32) / (segments as f32) * std::f32::consts::TAU;
        let a1 = ((index + 1) as f32) / (segments as f32) * std::f32::consts::TAU;
        fill.push(Vertex {
            position: [center.x, center.y],
            color,
            uv,
        });
        fill.push(Vertex {
            position: [center.x + a0.cos() * radius, center.y + a0.sin() * radius],
            color,
            uv,
        });
        fill.push(Vertex {
            position: [center.x + a1.cos() * radius, center.y + a1.sin() * radius],
            color,
            uv,
        });
    }
}

fn create_vertex_buffer(
    device: &wgpu::Device,
    label: &str,
    vertex_capacity: usize,
) -> wgpu::Buffer {
    let vertices = vertex_capacity.max(1);
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (vertices * std::mem::size_of::<Vertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn grow_buffer(
    device: &wgpu::Device,
    buffer: &mut wgpu::Buffer,
    capacity: &mut usize,
    needed: usize,
    label: &str,
) -> bool {
    if needed <= *capacity {
        return false;
    }
    let new_cap = needed.max(capacity.saturating_mul(2)).max(1024);
    *buffer = create_vertex_buffer(device, label, new_cap);
    *capacity = new_cap;
    true
}

fn write_vertices(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    vertex_offset: u32,
    vertices: &[Vertex],
) -> u64 {
    if vertices.is_empty() {
        return 0;
    }
    let bytes: &[u8] = bytemuck::cast_slice(vertices);
    queue.write_buffer(
        buffer,
        vertex_offset as u64 * std::mem::size_of::<Vertex>() as u64,
        bytes,
    );
    bytes.len() as u64
}

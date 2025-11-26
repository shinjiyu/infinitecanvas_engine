///! WebGPU 渲染器
///!
///! 使用 wgpu 实现高性能 GPU 渲染

use infinitecanvas_engine::{math::Vertex, AnyElement, Scene, Viewport};
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;
use std::sync::Arc;
use crate::texture_manager::TextureManager;

/// WebGPU 渲染器
pub struct WebGPURenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    
    // 基础渲染管线（纯色）
    fill_pipeline: wgpu::RenderPipeline,
    stroke_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_buffer_size: usize,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    
    // 纹理渲染管线
    texture_pipeline: wgpu::RenderPipeline,
    texture_sampler: wgpu::Sampler,
    texture_manager: TextureManager,
    
    // 占位纹理（用于无纹理绑定时）
    placeholder_texture: wgpu::Texture,
    placeholder_texture_view: wgpu::TextureView,
}

impl WebGPURenderer {
    /// 初始化 WebGPU 渲染器
    pub async fn new(canvas: web_sys::HtmlCanvasElement, width: u32, height: u32) -> Result<Self, JsValue> {
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
        log::info!("Surface configured: {}x{}, format: {:?}", width, height, surface_format);

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

        // 12. 创建初始顶点缓冲区（空的，后续动态调整）
        let vertex_buffer_size = 10000; // 初始容量
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (vertex_buffer_size * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 13. 创建纹理采样器
        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,    // 放大时线性插值
            min_filter: wgpu::FilterMode::Linear,    // 缩小时线性插值
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

        let placeholder_texture_view = placeholder_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 15. 创建纹理 BindGroup 布局
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let texture_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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
            vertex_buffer,
            vertex_buffer_size,
            uniform_buffer,
            uniform_bind_group,
            texture_pipeline,
            texture_sampler,
            texture_manager,
            placeholder_texture,
            placeholder_texture_view,
        })
    }

    /// 渲染场景
    pub fn render(&mut self, scene: &Scene, _viewport: &Viewport) -> Result<(), JsValue> {
        // 1. 收集所有顶点
        let (fill_vertices, stroke_vertices) = self.collect_vertices(scene);
        let textured_groups = self.collect_textured_vertices(scene);
        
        // 计算总顶点数
        let textured_vertex_count: usize = textured_groups.iter().map(|(_, v)| v.len()).sum();
        let total_vertices = fill_vertices.len() + stroke_vertices.len() + textured_vertex_count;
        
        log::warn!("🎨 Collected {} fill, {} stroke, {} textured vertices", 
            fill_vertices.len(), stroke_vertices.len(), textured_vertex_count);

        if total_vertices == 0 {
            log::warn!("⚠️ No vertices to render");
            return Ok(());
        }

        // 2. 扩展顶点缓冲区（如果需要）
        if total_vertices > self.vertex_buffer_size {
            self.vertex_buffer_size = (total_vertices * 2).max(1000);
            self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Vertex Buffer"),
                size: (self.vertex_buffer_size * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            log::debug!("Vertex buffer resized to {}", self.vertex_buffer_size);
        }

        // 3. 上传填充顶点
        let mut vertex_offset = 0;
        if !fill_vertices.is_empty() {
            self.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&fill_vertices),
            );
            vertex_offset += fill_vertices.len();
        }

        // 4. 上传描边顶点
        if !stroke_vertices.is_empty() {
            let offset_bytes = (vertex_offset * std::mem::size_of::<Vertex>()) as u64;
            self.queue.write_buffer(
                &self.vertex_buffer,
                offset_bytes,
                bytemuck::cast_slice(&stroke_vertices),
            );
            vertex_offset += stroke_vertices.len();
        }

        // 5. 上传纹理顶点（在渲染通道开始前）
        let base_textured_vertex_offset = vertex_offset;
        let mut textured_vertex_ranges = Vec::new();
        
        for (texture_id, vertices) in &textured_groups {
            if !vertices.is_empty() {
                let offset_bytes = (vertex_offset * std::mem::size_of::<Vertex>()) as u64;
                self.queue.write_buffer(
                    &self.vertex_buffer,
                    offset_bytes,
                    bytemuck::cast_slice(vertices),
                );
                
                let start = vertex_offset as u32;
                let end = start + vertices.len() as u32;
                textured_vertex_ranges.push((texture_id.clone(), start..end));
                
                vertex_offset += vertices.len();
                log::debug!("Uploaded {} vertices for texture '{}' at offset {}", vertices.len(), texture_id, start);
            }
        }

        // 6. 更新 Uniform Buffer（视口变换矩阵）
        let width = self.surface_config.width as f32;
        let height = self.surface_config.height as f32;
        
        // 正交投影矩阵（列优先，WGSL 格式）
        let projection_matrix: [[f32; 4]; 4] = [
            [2.0 / width, 0.0, 0.0, -1.0],          // 第1列：X 变换
            [0.0, -2.0 / height, 0.0, 1.0],         // 第2列：Y 变换（翻转）
            [0.0, 0.0, 1.0, 0.0],                   // 第3列：Z 保持不变
            [0.0, 0.0, 0.0, 1.0],                   // 第4列：齐次坐标
        ];
        
        log::warn!("📐 Projection: {}x{} -> NDC", width, height);
        
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&projection_matrix),
        );

        // 7. 获取当前帧
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("Failed to acquire next frame: {:?}", e)))?;

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // 8. 创建命令编码器
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // 9. 渲染通道
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
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

            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            // 绘制纯色填充
            if !fill_vertices.is_empty() {
                render_pass.set_pipeline(&self.fill_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.draw(0..fill_vertices.len() as u32, 0..1);
                log::debug!("Drew {} fill vertices", fill_vertices.len());
            }

            // 绘制纯色描边
            if !stroke_vertices.is_empty() {
                render_pass.set_pipeline(&self.stroke_pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                let start = fill_vertices.len() as u32;
                let end = start + stroke_vertices.len() as u32;
                render_pass.draw(start..end, 0..1);
                log::debug!("Drew {} stroke vertices", stroke_vertices.len());
            }

            // 绘制纹理元素（所有顶点已上传）
            if !textured_vertex_ranges.is_empty() {
                render_pass.set_pipeline(&self.texture_pipeline);
                
                for (texture_id, range) in &textured_vertex_ranges {
                    // 获取纹理视图
                    let texture_view = self
                        .texture_manager
                        .get_texture_view(texture_id)
                        .unwrap_or(&self.placeholder_texture_view);

                    // 为此纹理创建 BindGroup
                    let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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

                    // 绑定纹理并绘制
                    render_pass.set_bind_group(0, &texture_bind_group, &[]);
                    render_pass.draw(range.clone(), 0..1);
                    log::debug!("Drew {} textured vertices for '{}'", range.end - range.start, texture_id);
                }

                log::info!("Rendered {} texture groups", textured_vertex_ranges.len());
            }
        }

        // 10. 提交命令
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    /// 收集场景中所有元素的顶点
    fn collect_vertices(&self, scene: &Scene) -> (Vec<Vertex>, Vec<Vertex>) {
        let mut fill_vertices = Vec::new();
        let mut stroke_vertices = Vec::new();

        for element in scene.elements_sorted() {
            match element {
                AnyElement::Rectangle(rect) => {
                    fill_vertices.extend(rect.generate_fill_vertices());
                    stroke_vertices.extend(rect.generate_stroke_vertices());
                }
                AnyElement::Circle(circle) => {
                    fill_vertices.extend(circle.generate_fill_vertices());
                    stroke_vertices.extend(circle.generate_stroke_vertices());
                }
                AnyElement::Image(_image) => {
                    // Image 元素使用独立的纹理渲染管线
                }
            }
        }

        (fill_vertices, stroke_vertices)
    }

    /// 收集纹理元素的顶点（按纹理分组）
    ///
    /// # 返回
    /// Vec<(texture_id, vertices)> - 每个纹理的顶点列表
    fn collect_textured_vertices(&self, scene: &Scene) -> Vec<(String, Vec<Vertex>)> {
        use std::collections::HashMap;

        let mut texture_groups: HashMap<String, Vec<Vertex>> = HashMap::new();

        for element in scene.elements_sorted() {
            if let AnyElement::Image(image) = element {
                if !image.props.visible {
                    continue;
                }

                let texture_id = image.texture_id.clone();
                let vertices = image.generate_vertices();

                texture_groups
                    .entry(texture_id)
                    .or_insert_with(Vec::new)
                    .extend(vertices);
            }
        }

        texture_groups.into_iter().collect()
    }

    /// 调整 Surface 大小
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
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


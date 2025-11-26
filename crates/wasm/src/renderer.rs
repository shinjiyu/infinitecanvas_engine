///! WebGPU 渲染器
///!
///! 使用 wgpu 实现高性能 GPU 渲染

use infinitecanvas_engine::{math::Vertex, AnyElement, Scene, Viewport};
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

/// WebGPU 渲染器
pub struct WebGPURenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    fill_pipeline: wgpu::RenderPipeline,
    stroke_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_buffer_size: usize,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
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

        log::info!("WebGPU renderer initialized successfully");

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            fill_pipeline,
            stroke_pipeline,
            vertex_buffer,
            vertex_buffer_size,
            uniform_buffer,
            uniform_bind_group,
        })
    }

    /// 渲染场景
    pub fn render(&mut self, scene: &Scene, _viewport: &Viewport) -> Result<(), JsValue> {
        // 1. 收集所有顶点
        let (fill_vertices, stroke_vertices) = self.collect_vertices(scene);
        let total_vertices = fill_vertices.len() + stroke_vertices.len();
        
        log::warn!("🎨 Collected {} fill vertices, {} stroke vertices", fill_vertices.len(), stroke_vertices.len());

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
        if !fill_vertices.is_empty() {
            self.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&fill_vertices),
            );
        }

        // 4. 上传描边顶点（偏移量）
        if !stroke_vertices.is_empty() {
            let offset = (fill_vertices.len() * std::mem::size_of::<Vertex>()) as u64;
            self.queue.write_buffer(
                &self.vertex_buffer,
                offset,
                bytemuck::cast_slice(&stroke_vertices),
            );
        }

        // 5. 更新 Uniform Buffer（视口变换矩阵）
        // 正交投影矩阵：将像素坐标 [0, width] x [0, height] 转换为 NDC [-1, 1] x [-1, 1]
        let width = self.surface_config.width as f32;
        let height = self.surface_config.height as f32;
        
        // 正交投影矩阵（列优先，WGSL 格式）
        // 标准正交投影：(x, y) -> (2*x/width - 1, 1 - 2*y/height)
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

        // 6. 获取当前帧
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("Failed to acquire next frame: {:?}", e)))?;

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // 7. 创建命令编码器
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // 8. 渲染通道
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

            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

            // 绘制填充
            if !fill_vertices.is_empty() {
                render_pass.set_pipeline(&self.fill_pipeline);
                render_pass.draw(0..fill_vertices.len() as u32, 0..1);
            }

            // 绘制描边
            if !stroke_vertices.is_empty() {
                render_pass.set_pipeline(&self.stroke_pipeline);
                render_pass.draw(
                    fill_vertices.len() as u32..(fill_vertices.len() + stroke_vertices.len()) as u32,
                    0..1,
                );
            }
        }

        // 9. 提交命令
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
            }
        }

        (fill_vertices, stroke_vertices)
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
}


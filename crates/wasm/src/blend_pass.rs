//! 非 Normal 混合时的逐叶子合成。
//!
//! 全部叶子都是 Normal 时渲染器仍走原来的批量绘制。只要有一个可见叶子不是 Normal，
//! 就改到这里：按 z 顺序把每个叶子先画进透明层（填充、描边、纹理一起，普通 alpha），
//! 再按 W3C 公式叠到场景纹理上。这样描边不会拿混合模式去乘自己的填充。
//!
//! 画布清成不透明底色，合成结果也保持 alpha = 1。组没有自己的层，子孙各自混合。
//! 不可见叶子没有 [`PaintSpan`]。选中框在交换链上最后画，不参与混合。

use super::WebGPURenderer;
use infinitecanvas_engine::{BlendMode, PaintSpan};

const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.1,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};

const TRANSPARENT: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

const COMPOSITE_MODES: [u32; 5] = [0, 1, 2, 3, 4];

/// 场景乒乓缓冲、叶子层，以及五种混合和一次 blit 的管线。
pub(crate) struct BlendTargets {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    scenes: [wgpu::Texture; 2],
    layer: wgpu::Texture,
    sampler: wgpu::Sampler,
    composite_layout: wgpu::BindGroupLayout,
    composite_pipelines: [wgpu::RenderPipeline; 5],
    blit_layout: wgpu::BindGroupLayout,
    blit_pipeline: wgpu::RenderPipeline,
}

impl BlendTargets {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        let scenes = [
            color_target(device, "Blend Scene 0", format, width, height),
            color_target(device, "Blend Scene 1", format, width, height),
        ];
        let layer = color_target(device, "Blend Layer", format, width, height);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blend Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blend Composite Layout"),
            entries: &[texture_entry(0), texture_entry(1), sampler_entry(2)],
        });
        let composite_pipelines = std::array::from_fn(|index| {
            let source = format!(
                "const MODE: u32 = {}u;\n{}",
                COMPOSITE_MODES[index],
                include_str!("shaders/composite.wgsl")
            );
            fullscreen_pipeline(
                device,
                &source,
                "Blend Composite",
                &composite_layout,
                format,
                &format!("Blend Composite {index}"),
            )
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blend Blit Layout"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });
        let blit_pipeline = fullscreen_pipeline(
            device,
            include_str!("shaders/blit.wgsl"),
            "Blend Blit",
            &blit_layout,
            format,
            "Blend Blit Pipeline",
        );
        Self {
            width,
            height,
            format,
            scenes,
            layer,
            sampler,
            composite_layout,
            composite_pipelines,
            blit_layout,
            blit_pipeline,
        }
    }

    fn matches(&self, format: wgpu::TextureFormat, width: u32, height: u32) -> bool {
        self.format == format && self.width == width && self.height == height
    }

    fn pipeline_index(mode: BlendMode) -> usize {
        match mode {
            BlendMode::Multiply => 0,
            BlendMode::Screen => 1,
            BlendMode::Overlay => 2,
            BlendMode::Darken => 3,
            BlendMode::Lighten => 4,
            BlendMode::Normal => 0,
        }
    }
}

impl WebGPURenderer {
    /// 画布尺寸变化后丢掉旧的合成目标。下一帧有非 Normal 叶子时重建。
    pub(crate) fn discard_blend_targets(&mut self) {
        self.blend_targets = None;
    }

    /// 保证合成纹理和当前交换链一样大。
    pub(crate) fn ensure_blend_targets(&mut self) {
        let width = self.surface_config.width.max(1);
        let height = self.surface_config.height.max(1);
        let format = self.surface_config.format;
        let fresh = self
            .blend_targets
            .as_ref()
            .is_some_and(|targets| targets.matches(format, width, height));
        if fresh {
            return;
        }
        self.blend_targets = Some(BlendTargets::new(&self.device, format, width, height));
    }

    /// 按绘制顺序合成非 Normal 场景，再把结果和选中框画到交换链。
    ///
    /// # 返回
    /// 本帧 draw call 次数。
    pub(crate) fn draw_blended(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        swapchain: &wgpu::TextureView,
        grid_count: u32,
        selection_fill_count: u32,
        selection_stroke_count: u32,
    ) -> usize {
        let Some(targets) = self.blend_targets.as_ref() else {
            return 0;
        };
        let spans = self.vertex_cache.paint_spans();
        let scene_views = [
            targets.scenes[0].create_view(&wgpu::TextureViewDescriptor::default()),
            targets.scenes[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];
        let layer_view = targets
            .layer
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut draw_calls = 0usize;
        let mut index = 0usize;
        let mut current = 0usize;
        {
            let mut pass = begin_pass(encoder, &scene_views[0], Some(BACKGROUND));
            draw_calls += draw_grid(self, &mut pass, grid_count);
            draw_calls += draw_normal_run(self, &mut pass, &spans, &mut index);
        }
        while index < spans.len() {
            if spans[index].blend == BlendMode::Normal {
                let mut pass = begin_pass(encoder, &scene_views[current], None);
                draw_calls += draw_normal_run(self, &mut pass, &spans, &mut index);
                continue;
            }
            {
                let mut pass = begin_pass(encoder, &layer_view, Some(TRANSPARENT));
                draw_calls += draw_span(self, &mut pass, &spans[index]);
            }
            let mode = spans[index].blend;
            index += 1;
            let next = 1 - current;
            {
                let mut pass = begin_pass(encoder, &scene_views[next], None);
                draw_calls += draw_composite(
                    self,
                    targets,
                    &mut pass,
                    mode,
                    &layer_view,
                    &scene_views[current],
                );
            }
            current = next;
        }
        {
            let mut pass = begin_pass(encoder, swapchain, Some(BACKGROUND));
            draw_calls += draw_blit(self, targets, &mut pass, &scene_views[current]);
            draw_calls += draw_selection(
                self,
                &mut pass,
                selection_fill_count,
                selection_stroke_count,
            );
        }
        draw_calls
    }
}

fn draw_grid(renderer: &WebGPURenderer, pass: &mut wgpu::RenderPass<'_>, grid_count: u32) -> usize {
    if grid_count == 0 {
        return 0;
    }
    pass.set_pipeline(&renderer.stroke_pipeline);
    pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
    pass.set_vertex_buffer(0, renderer.grid_buffer.slice(..));
    pass.draw(0..grid_count, 0..1);
    1
}

fn draw_normal_run(
    renderer: &WebGPURenderer,
    pass: &mut wgpu::RenderPass<'_>,
    spans: &[PaintSpan],
    index: &mut usize,
) -> usize {
    let mut calls = 0;
    while *index < spans.len() && spans[*index].blend == BlendMode::Normal {
        calls += draw_span(renderer, pass, &spans[*index]);
        *index += 1;
    }
    calls
}

fn draw_span(
    renderer: &WebGPURenderer,
    pass: &mut wgpu::RenderPass<'_>,
    span: &PaintSpan,
) -> usize {
    let mut calls = 0;
    if span.fill_count > 0 {
        pass.set_pipeline(&renderer.fill_pipeline);
        pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, renderer.fill_buffer.slice(..));
        let end = span.fill_offset + span.fill_count;
        pass.draw(span.fill_offset..end, 0..1);
        calls += 1;
    }
    if span.stroke_count > 0 {
        pass.set_pipeline(&renderer.stroke_pipeline);
        pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, renderer.stroke_buffer.slice(..));
        let end = span.stroke_offset + span.stroke_count;
        pass.draw(span.stroke_offset..end, 0..1);
        calls += 1;
    }
    if span.tex_count == 0 {
        return calls;
    }
    let end = span.tex_offset + span.tex_count;
    if span.underlay {
        pass.set_pipeline(&renderer.fill_pipeline);
        pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, renderer.textured_buffer.slice(..));
        pass.draw(span.tex_offset..end, 0..1);
        calls += 1;
    }
    let Some(texture_id) = span.texture_id.as_deref() else {
        return calls;
    };
    let texture_view = renderer
        .texture_manager
        .get_texture_view(texture_id)
        .unwrap_or(&renderer.placeholder_texture_view);
    let bind_group = renderer
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blend Leaf Texture"),
            layout: &renderer.texture_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: renderer.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&renderer.texture_sampler),
                },
            ],
        });
    pass.set_pipeline(&renderer.texture_pipeline);
    pass.set_vertex_buffer(0, renderer.textured_buffer.slice(..));
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(span.tex_offset..end, 0..1);
    calls + 1
}

fn draw_composite(
    renderer: &WebGPURenderer,
    targets: &BlendTargets,
    pass: &mut wgpu::RenderPass<'_>,
    mode: BlendMode,
    layer: &wgpu::TextureView,
    scene: &wgpu::TextureView,
) -> usize {
    let pipeline = &targets.composite_pipelines[BlendTargets::pipeline_index(mode)];
    let bind_group = renderer
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blend Composite Bind Group"),
            layout: &targets.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(layer),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&targets.sampler),
                },
            ],
        });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
    1
}

fn draw_blit(
    renderer: &WebGPURenderer,
    targets: &BlendTargets,
    pass: &mut wgpu::RenderPass<'_>,
    scene: &wgpu::TextureView,
) -> usize {
    let bind_group = renderer
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blend Blit Bind Group"),
            layout: &targets.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&targets.sampler),
                },
            ],
        });
    pass.set_pipeline(&targets.blit_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
    1
}

fn draw_selection(
    renderer: &WebGPURenderer,
    pass: &mut wgpu::RenderPass<'_>,
    fill_count: u32,
    stroke_count: u32,
) -> usize {
    if fill_count == 0 && stroke_count == 0 {
        return 0;
    }
    pass.set_vertex_buffer(0, renderer.selection_buffer.slice(..));
    let mut calls = 0;
    if fill_count > 0 {
        pass.set_pipeline(&renderer.fill_pipeline);
        pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
        pass.draw(0..fill_count, 0..1);
        calls += 1;
    }
    if stroke_count > 0 {
        pass.set_pipeline(&renderer.stroke_pipeline);
        pass.set_bind_group(0, &renderer.uniform_bind_group, &[]);
        let start = fill_count;
        pass.draw(start..start + stroke_count, 0..1);
        calls += 1;
    }
    calls
}

fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    clear: Option<wgpu::Color>,
) -> wgpu::RenderPass<'a> {
    let (load, store) = match clear {
        Some(color) => (wgpu::LoadOp::Clear(color), wgpu::StoreOp::Store),
        None => (wgpu::LoadOp::Load, wgpu::StoreOp::Store),
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Blend Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations { load, store },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    })
}

fn color_target(
    device: &wgpu::Device,
    label: &str,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn fullscreen_pipeline(
    device: &wgpu::Device,
    source: &str,
    shader_label: &str,
    layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    pipeline_label: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(shader_label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(pipeline_label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(pipeline_label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
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
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn composite_and_blit_shaders_parse() {
        wgpu::naga::front::wgsl::parse_str(include_str!("shaders/blit.wgsl")).expect("blit shader");
        for mode in 0..5 {
            let source = format!(
                "const MODE: u32 = {mode}u;\n{}",
                include_str!("shaders/composite.wgsl")
            );
            wgpu::naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|err| panic!("composite mode {mode}: {err}"));
        }
    }
}

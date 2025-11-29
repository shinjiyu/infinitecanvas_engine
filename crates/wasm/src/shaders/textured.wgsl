// textured.wgsl - 纹理渲染 Shader
//
// 支持：
// - 纹理采样和渲染
// - 顶点颜色混合（tint）
// - UV 坐标映射
// - 透明度支持

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct Uniforms {
    transform: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var t_diffuse: texture_2d<f32>;

@group(0) @binding(2)
var s_diffuse: sampler;

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.transform * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    out.uv = model.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    return tex_color * in.color;
}














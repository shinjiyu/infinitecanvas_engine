// textured.wgsl - 纹理渲染 Shader
//
// 支持：
// - 纹理采样和渲染
// - 顶点颜色混合（tint）
// - UV 坐标映射
// - 透明度支持
// - 图层模糊：vs_blur / fs_blur 做 5×5 高斯。锐利图片仍走 vs_main / fs_main

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

struct BlurInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) blur_uv: vec2<f32>,
};

struct BlurOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) blur_uv: vec2<f32>,
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

fn gauss(t: f32) -> f32 {
    return exp(-0.5 * t * t);
}

@vertex
fn vs_blur(model: BlurInput) -> BlurOutput {
    var out: BlurOutput;
    out.clip_position = uniforms.transform * vec4<f32>(model.position, 0.0, 1.0);
    out.color = model.color;
    out.uv = model.uv;
    out.blur_uv = model.blur_uv;
    return out;
}

/// 5×5 高斯。采样间距是顶点里的 `blur_uv`（σ，UV 单位）。
///
/// `textureSample` 必须走统一控制流，所以框外点先钳到 0–1 再采样，然后把结果乘成 0。
/// 权重按整个核归一，边缘因此变淡，和 CPU 侧 `kernel_color` 一致。
@fragment
fn fs_blur(in: BlurOutput) -> @location(0) vec4<f32> {
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    for (var j: i32 = -2; j <= 2; j = j + 1) {
        for (var i: i32 = -2; i <= 2; i = i + 1) {
            let w = gauss(f32(i)) * gauss(f32(j));
            weight = weight + w;
            let sample_uv = in.uv + vec2<f32>(f32(i), f32(j)) * in.blur_uv;
            let inside = sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0;
            let clamped = clamp(sample_uv, vec2<f32>(0.0), vec2<f32>(1.0));
            let tap = textureSample(t_diffuse, s_diffuse, clamped);
            sum = sum + select(vec4<f32>(0.0), tap, inside) * w;
        }
    }
    return (sum / weight) * in.color;
}

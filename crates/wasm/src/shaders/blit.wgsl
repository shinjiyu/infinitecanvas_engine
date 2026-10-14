// 把已经合成好的场景纹理原样写到交换链。

struct FullscreenOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0)
var scene_tex: texture_2d<f32>;

@group(0) @binding(1)
var samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullscreenOut;
    out.clip = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(out.clip.x * 0.5 + 0.5, 0.5 - out.clip.y * 0.5);
    return out;
}

@fragment
fn fs_main(in: FullscreenOut) -> @location(0) vec4<f32> {
    return textureSample(scene_tex, samp, in.uv);
}

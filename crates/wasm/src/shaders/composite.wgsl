// 把叶子层（预乘）按 MODE 叠到场景上。
// MODE: 0 multiply, 1 screen, 2 overlay, 3 darken, 4 lighten
// 场景目标保持不透明，所以结果 alpha 写成 1。

struct FullscreenOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0)
var layer_tex: texture_2d<f32>;

@group(0) @binding(1)
var scene_tex: texture_2d<f32>;

@group(0) @binding(2)
var samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> FullscreenOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: FullscreenOut;
    out.clip = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(out.clip.x * 0.5 + 0.5, 0.5 - out.clip.y * 0.5);
    return out;
}

fn overlay_channel(src: f32, dst: f32) -> f32 {
    if (dst <= 0.5) {
        return 2.0 * src * dst;
    }
    return 1.0 - 2.0 * (1.0 - src) * (1.0 - dst);
}

fn blend_rgb(src: vec3<f32>, dst: vec3<f32>) -> vec3<f32> {
    if (MODE == 0u) {
        return src * dst;
    }
    if (MODE == 1u) {
        return vec3<f32>(1.0) - (vec3<f32>(1.0) - src) * (vec3<f32>(1.0) - dst);
    }
    if (MODE == 2u) {
        return vec3<f32>(
            overlay_channel(src.r, dst.r),
            overlay_channel(src.g, dst.g),
            overlay_channel(src.b, dst.b),
        );
    }
    if (MODE == 3u) {
        return min(src, dst);
    }
    return max(src, dst);
}

@fragment
fn fs_main(in: FullscreenOut) -> @location(0) vec4<f32> {
    let premul = textureSample(layer_tex, samp, in.uv);
    let dst = textureSample(scene_tex, samp, in.uv);
    let alpha = premul.a;
    let src = premul.rgb / max(alpha, 1e-5);
    let blended = blend_rgb(src, dst.rgb);
    let out_rgb = blended * alpha + dst.rgb * (1.0 - alpha);
    return vec4<f32>(out_rgb, 1.0);
}

// 基础 Shader - 顶点颜色渲染

// Uniform Buffer: 视口变换矩阵
struct Uniforms {
    transform: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// 顶点输入
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,  // 需要声明以匹配Vertex结构体，但basic shader不使用
}

// 顶点输出 / 片元输入
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// 顶点着色器
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // 应用变换矩阵
    out.clip_position = uniforms.transform * vec4<f32>(in.position, 0.0, 1.0);
    
    // 传递颜色
    out.color = in.color;
    // 注意：uv 在此shader中不使用
    
    return out;
}

// 片元着色器
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}



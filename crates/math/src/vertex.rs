//! GPU 顶点结构
//!
//! 定义用于 WebGPU 渲染的顶点数据
use crate::Color;

/// 2D 顶点（位置 + 颜色 + UV）
///
/// # 内存布局
/// - position: [f32; 2] - 8 字节
/// - color: [f32; 4] - 16 字节
/// - uv: [f32; 2] - 8 字节
/// - 总计: 32 字节
///
/// # 对齐要求
/// - 符合 WebGPU 顶点缓冲区要求
/// - `#[repr(C)]` 确保 C 语言兼容布局
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vertex {
    /// 顶点位置（世界空间坐标）
    pub position: [f32; 2],
    /// 顶点颜色（RGBA）
    pub color: [f32; 4],
    /// 纹理坐标（UV，归一化 0-1）
    pub uv: [f32; 2],
}

impl Vertex {
    /// 创建顶点（非纹理，UV 默认为 0）
    pub fn new(x: f32, y: f32, color: Color) -> Self {
        Self {
            position: [x, y],
            color: color.to_array(),
            uv: [0.0, 0.0],
        }
    }

    /// 创建带纹理坐标的顶点
    pub fn with_uv(x: f32, y: f32, color: Color, u: f32, v: f32) -> Self {
        Self {
            position: [x, y],
            color: color.to_array(),
            uv: [u, v],
        }
    }

    /// 从位置和颜色数组创建（UV 默认为 0）
    pub fn from_arrays(position: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position,
            color,
            uv: [0.0, 0.0],
        }
    }

    /// 从位置、颜色和 UV 数组创建
    pub fn from_arrays_with_uv(position: [f32; 2], color: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            position,
            color,
            uv,
        }
    }
}

// 为 bytemuck 提供 Pod 和 Zeroable 实现（用于 WebGPU）
// 这允许将 Vertex 直接转换为字节数组
unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_size() {
        use std::mem::size_of;
        // 确保顶点大小正确（2 * f32 + 4 * f32 + 2 * f32 = 32 bytes）
        assert_eq!(size_of::<Vertex>(), 32);
    }

    #[test]
    fn test_vertex_creation() {
        let v = Vertex::new(10.0, 20.0, Color::RED);
        assert_eq!(v.position, [10.0, 20.0]);
        assert_eq!(v.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(v.uv, [0.0, 0.0]);
    }

    #[test]
    fn test_vertex_with_uv() {
        let v = Vertex::with_uv(10.0, 20.0, Color::BLUE, 0.5, 0.75);
        assert_eq!(v.position, [10.0, 20.0]);
        assert_eq!(v.color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(v.uv, [0.5, 0.75]);
    }

    #[test]
    fn test_bytemuck_conversion() {
        let v = Vertex::new(1.0, 2.0, Color::GREEN);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 32);
    }
}

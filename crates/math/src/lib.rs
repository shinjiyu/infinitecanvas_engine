//! InfiniteCanvas Math - Mathematical utilities
//!
//! 提供无限画布所需的数学类型和计算工具。
//!
//! # 模块职责
//! - 向量、矩阵运算（基于 glam）
//! - 几何计算（边界框、相交检测）
//! - 坐标变换（屏幕空间 ↔ 世界空间）
//! - 颜色处理（RGB、十六进制）
//!
//! # 核心类型
//! - `Vec2`: 2D 向量（位置、速度）
//! - `Transform`: 2D 变换（平移、旋转、缩放）
//! - `Rect`: 矩形（边界框）
//! - `Color`: RGBA 颜色

mod color;
mod rect;
mod transform;
mod vertex;

// 重导出核心类型
pub use color::Color;
pub use glam::{vec2, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
pub use rect::Rect;
pub use transform::Transform;
pub use vertex::Vertex;

///! 2D 变换
///!
///! 提供平移、缩放、旋转的组合变换

use glam::{Mat3, Mat4, Quat, Vec2};
use serde::{Deserialize, Serialize};

/// 2D 变换
///
/// 封装平移、旋转、缩放操作
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// 平移
    pub translation: Vec2,
    /// 旋转（弧度）
    pub rotation: f32,
    /// 缩放
    pub scale: Vec2,
}

impl Transform {
    /// 创建单位变换
    pub fn identity() -> Self {
        Self {
            translation: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }

    /// 从平移创建
    pub fn from_translation(translation: Vec2) -> Self {
        Self {
            translation,
            ..Self::identity()
        }
    }

    /// 从旋转创建
    pub fn from_rotation(rotation: f32) -> Self {
        Self {
            rotation,
            ..Self::identity()
        }
    }

    /// 从缩放创建
    pub fn from_scale(scale: Vec2) -> Self {
        Self {
            scale,
            ..Self::identity()
        }
    }

    /// 平移
    pub fn translate(&mut self, delta: Vec2) {
        self.translation += delta;
    }

    /// 旋转
    pub fn rotate(&mut self, angle: f32) {
        self.rotation += angle;
    }

    /// 缩放
    pub fn scale_by(&mut self, factor: Vec2) {
        self.scale *= factor;
    }

    /// 转换为 3x3 矩阵
    pub fn to_matrix(&self) -> Mat3 {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();

        Mat3::from_cols_array(&[
            cos * self.scale.x,
            sin * self.scale.x,
            0.0,
            -sin * self.scale.y,
            cos * self.scale.y,
            0.0,
            self.translation.x,
            self.translation.y,
            1.0,
        ])
    }

    /// 转换为 4x4 矩阵（WebGPU 使用）
    ///
    /// 将 2D 变换嵌入到 3D 空间（Z=0 平面）
    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale.extend(1.0),
            Quat::from_rotation_z(self.rotation),
            self.translation.extend(0.0),
        )
    }

    /// 变换点
    pub fn transform_point(&self, point: Vec2) -> Vec2 {
        let matrix = self.to_matrix();
        let homogeneous = matrix.mul_vec3(point.extend(1.0));
        homogeneous.truncate()
    }

    /// 变换向量（忽略平移）
    pub fn transform_vector(&self, vector: Vec2) -> Vec2 {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        Vec2::new(
            (cos * vector.x - sin * vector.y) * self.scale.x,
            (sin * vector.x + cos * vector.y) * self.scale.y,
        )
    }

    /// 逆变换点
    pub fn inverse_transform_point(&self, point: Vec2) -> Vec2 {
        let translated = point - self.translation;
        let cos = (-self.rotation).cos();
        let sin = (-self.rotation).sin();
        Vec2::new(
            (cos * translated.x - sin * translated.y) / self.scale.x,
            (sin * translated.x + cos * translated.y) / self.scale.y,
        )
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let transform = Transform::identity();
        let point = Vec2::new(10.0, 20.0);
        assert_eq!(transform.transform_point(point), point);
    }

    #[test]
    fn test_translation() {
        let transform = Transform::from_translation(Vec2::new(100.0, 200.0));
        let point = Vec2::new(10.0, 20.0);
        let result = transform.transform_point(point);
        assert_eq!(result, Vec2::new(110.0, 220.0));
    }

    #[test]
    fn test_scale() {
        let transform = Transform::from_scale(Vec2::splat(2.0));
        let point = Vec2::new(10.0, 20.0);
        let result = transform.transform_point(point);
        assert_eq!(result, Vec2::new(20.0, 40.0));
    }

    #[test]
    fn test_inverse() {
        let transform = Transform::from_translation(Vec2::new(100.0, 200.0));
        let point = Vec2::new(10.0, 20.0);
        let transformed = transform.transform_point(point);
        let back = transform.inverse_transform_point(transformed);
        assert!((back - point).length() < 0.001);
    }
}

//! 2D 变换
//!
//! 提供平移、缩放、旋转的组合变换
use glam::{Mat3, Mat4, Quat, Vec2};
use serde::{Deserialize, Serialize};

fn default_scale() -> Vec2 {
    Vec2::ONE
}

/// 2D 变换
///
/// 封装平移、旋转、缩放操作。
/// JSON 中缺省的 `translation` 为 0，`rotation` 为 0，`scale` 为 1，
/// 未知字段会被忽略，便于以后增加变换属性而不升版本号。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    /// 平移
    #[serde(default)]
    pub translation: Vec2,
    /// 旋转（弧度）
    #[serde(default)]
    pub rotation: f32,
    /// 缩放
    #[serde(default = "default_scale")]
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

    /// 从 3x3 矩阵分解回平移、旋转、缩放。
    ///
    /// 矩阵需要是 `T * R * S`（与 [`Transform::to_matrix`] 相同）。带剪切的矩阵会丢掉剪切，只保留最接近的旋转和轴向缩放。
    pub fn from_mat3(matrix: Mat3) -> Self {
        let column_x = Vec2::new(matrix.x_axis.x, matrix.x_axis.y);
        let column_y = Vec2::new(matrix.y_axis.x, matrix.y_axis.y);
        let (rotation, scale) = decompose_basis(column_x, column_y);
        Self {
            translation: Vec2::new(matrix.z_axis.x, matrix.z_axis.y),
            rotation,
            scale,
        }
    }

    /// 缩放是否远离 0，矩阵可逆。
    pub fn is_invertible(self) -> bool {
        self.scale.x.is_finite()
            && self.scale.y.is_finite()
            && self.translation.x.is_finite()
            && self.translation.y.is_finite()
            && self.rotation.is_finite()
            && self.scale.x.abs() > 1e-6
            && self.scale.y.abs() > 1e-6
    }

    /// 先应用 `self`，再应用 `outer`（矩阵 `outer * self`）。
    pub fn then(self, outer: Transform) -> Self {
        Self::from_mat3(outer.to_matrix() * self.to_matrix())
    }

    /// 父变换从 `from` 变为 `to` 时，子元素的世界变换。
    ///
    /// 世界点的映射是 `to * inverse(from)`。组平移、旋转、均匀缩放都走这条路径，
    /// 这样子元素不用单独存局部坐标，现有顶点生成仍然读世界变换。
    ///
    /// # 示例
    /// ```
    /// use infinitecanvas_math::{Transform, Vec2};
    ///
    /// let from = Transform::from_translation(Vec2::new(10.0, 0.0));
    /// let to = Transform::from_translation(Vec2::new(25.0, 4.0));
    /// let child = Transform::from_translation(Vec2::new(12.0, 3.0));
    /// let moved = child.rebase_between(from, to);
    /// assert!((moved.translation - Vec2::new(27.0, 7.0)).length() < 1e-4);
    /// ```
    pub fn rebase_between(self, from: Transform, to: Transform) -> Self {
        if !from.is_invertible() {
            return self;
        }
        let delta = to.to_matrix() * from.to_matrix().inverse();
        Self::from_mat3(delta * self.to_matrix())
    }
}

/// 从矩阵的两列分解旋转和轴向缩放。
fn decompose_basis(column_x: Vec2, column_y: Vec2) -> (f32, Vec2) {
    let scale_x = column_x.length();
    if scale_x < 1e-6 {
        return (0.0, Vec2::new(1e-3, column_y.length().max(1e-3)));
    }
    let rotation = column_x.y.atan2(column_x.x);
    let scale_y_len = column_y.length().max(1e-4);
    let (sin, cos) = rotation.sin_cos();
    let expected_y = Vec2::new(-sin, cos) * scale_y_len;
    let scale_y = if column_y.dot(expected_y) < 0.0 {
        -scale_y_len
    } else {
        scale_y_len
    };
    (rotation, Vec2::new(scale_x.max(1e-4), scale_y))
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

    #[test]
    fn rebase_between_translates_child() {
        let from = Transform::from_translation(Vec2::new(10.0, 0.0));
        let to = Transform::from_translation(Vec2::new(25.0, 4.0));
        let child = Transform::from_translation(Vec2::new(12.0, 3.0));
        let moved = child.rebase_between(from, to);
        assert!((moved.translation - Vec2::new(27.0, 7.0)).length() < 1e-4);
        assert!(moved.rotation.abs() < 1e-4);
        assert!((moved.scale - Vec2::ONE).length() < 1e-4);
    }

    #[test]
    fn rebase_between_rotates_child_around_parent_origin() {
        let from = Transform::from_translation(Vec2::new(50.0, 0.0));
        let mut to = from;
        to.rotation = std::f32::consts::FRAC_PI_2;
        let child = Transform::from_translation(Vec2::new(100.0, 0.0));
        let moved = child.rebase_between(from, to);
        assert!((moved.translation - Vec2::new(50.0, 50.0)).length() < 1e-3);
        assert!((moved.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn matrix_round_trip_keeps_trs() {
        let transform = Transform {
            translation: Vec2::new(-12.0, 8.0),
            rotation: 0.4,
            scale: Vec2::new(2.0, 0.5),
        };
        let back = Transform::from_mat3(transform.to_matrix());
        assert!((back.translation - transform.translation).length() < 1e-4);
        assert!((back.rotation - transform.rotation).abs() < 1e-4);
        assert!((back.scale - transform.scale).length() < 1e-4);
    }
}

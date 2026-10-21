//! 图层高斯模糊。
//!
//! 每个叶子一份 `blur_radius`（f32，缺省 0）。0 表示关闭，旧 JSON 没有这个字段也能读入。
//! 小于等于 0 或非有限值在绘制和采样时收成 0，序列化时省略。
//!
//! v1 不用离屏可分离卷积：
//!
//! - 均匀颜色的形状（填充、实线描边、直线、路径）把高斯模糊收成有符号距离上的 CDF
//!   （`erfc` 的有理近似，标准库的 `f32::erfc` 仍不稳定）。顶点沿外法线从 -3σ 铺到 +3σ，圈数固定。σ 大于形状内径时内圈会交叉，
//!   星形更明显。虚线在模糊打开时收成同一条软带，间隙不再断开。
//!   渐变和模糊同时打开时，软边用包围盒中心采到的一个颜色，不卷积整段渐变；模糊回到 0 后渐变顶点恢复。改模糊不会清掉渐变字段。
//!   布尔运算得到的填充路径若带孔，实体是内缩 3σ 后的耳切三角（孔保留），软边单独贴在每条环上，孔不会被扇形填实。
//! - 图片和文本在纹理着色器里做 5×5 高斯，采样间距是 σ。四边形向外扩 3σ，
//!   框外纹素按透明处理。UV 裁剪的图片仍按 0–1 裁掉框外样本，裁剪框内的邻接纹素可能渗进光晕。
//!
//! 命中测试和 [`crate::Element::local_bounds`] 用未模糊的轴对齐包围盒。
//! 光晕上的点选不中。PNG 会把光晕画出来；SVG 用 `feGaussianBlur` 近似，不保证和顶点环一致。
//!
//! 不透明度和可见性沿用元素自己的 alpha 与 [`crate::Scene::is_effectively_visible`]。
//! 模糊不改变 z 顺序：形状的软边写在自己的填充缓冲里，跟在投影后面；文本和图片的纹理仍在全部填充之后。

use super::shadow::{
    capsule_outline, composite_over, fan_fill, feather_strip, open_ring, outward_normals,
};
use infinitecanvas_math::{Color, Transform, Vec2, Vertex};

/// 高斯衰减收到大约这个倍数之外就可以当成 0。顶点和 PNG 包围盒都用它。
pub(crate) const BLUR_EXTENT: f32 = 3.0;

/// 纹理核的半径（采样点是 `-RADIUS..=RADIUS`）。间距为 1σ，覆盖大约 ±2σ。
pub(crate) const KERNEL_RADIUS: i32 = 2;

/// 形状轮廓的羽化圈数。圈数固定，σ 变大只把环推远。
const SILHOUETTE_RINGS: usize = 6;

/// 非有限或负数收成 0。
pub(crate) fn sanitize_blur(radius: f32) -> f32 {
    if radius.is_finite() && radius > 0.0 {
        radius
    } else {
        0.0
    }
}

/// 半径大到需要换软边路径。
pub(crate) fn blur_active(radius: f32) -> bool {
    sanitize_blur(radius) > 1.0e-4
}

/// 把元素上存的半径收成非负有限值。场景加载时调用。
pub(crate) fn sanitize_element(element: &mut crate::scene::AnyElement) {
    use crate::scene::AnyElement;
    match element {
        AnyElement::Rectangle(rect) => {
            rect.style.blur_radius = sanitize_blur(rect.style.blur_radius);
        }
        AnyElement::Circle(circle) => {
            circle.style.blur_radius = sanitize_blur(circle.style.blur_radius);
        }
        AnyElement::Polygon(polygon) => {
            polygon.style.blur_radius = sanitize_blur(polygon.style.blur_radius);
        }
        AnyElement::Line(line) => {
            line.style.blur_radius = sanitize_blur(line.style.blur_radius);
        }
        AnyElement::Path(path) => {
            path.style.blur_radius = sanitize_blur(path.style.blur_radius);
        }
        AnyElement::Text(text) => {
            text.blur_radius = sanitize_blur(text.blur_radius);
        }
        AnyElement::Image(image) => {
            image.blur_radius = sanitize_blur(image.blur_radius);
        }
        AnyElement::Frame(frame) => {
            frame.style.blur_radius = sanitize_blur(frame.style.blur_radius);
        }
        AnyElement::Group(_) => {}
    }
}

/// `P(N(0, σ) <= x)`。σ 接近 0 时是阶跃。
fn gaussian_cdf(x: f32, sigma: f32) -> f32 {
    let sigma = sanitize_blur(sigma);
    if !x.is_finite() {
        return 0.0;
    }
    if sigma <= 1.0e-4 {
        return if x >= 0.0 { 1.0 } else { 0.0 };
    }
    0.5 * approx_erfc(-x / (sigma * std::f32::consts::SQRT_2))
}

/// `erfc` 的 Abramowitz–Stegun 7.1.26 近似。最大绝对误差大约 1.5×10⁻⁷。
///
/// `f32::erfc` 还在 `float_erf` 特性后面，所以这里自己算。
fn approx_erfc(x: f32) -> f32 {
    if !x.is_finite() {
        return if x.is_sign_negative() { 2.0 } else { 0.0 };
    }
    if x < 0.0 {
        return (2.0 - approx_erfc(-x)).clamp(0.0, 2.0);
    }
    let x = f64::from(x);
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    (poly * (-x * x).exp()).clamp(0.0, 2.0) as f32
}

/// 填充区域（有符号距离 ≤ 0）被高斯模糊之后的覆盖率。
///
/// 距离为正表示在形状外。σ 为 0 时内部是 1，外部是 0。边界上约为 0.5。
pub(crate) fn edge_coverage(signed_distance: f32, sigma: f32) -> f32 {
    if !signed_distance.is_finite() {
        return 0.0;
    }
    gaussian_cdf(-signed_distance, sigma).clamp(0.0, 1.0)
}

/// 以轮廓为中心、半宽 `half_width` 的描边带被高斯模糊之后的覆盖率。
pub(crate) fn band_coverage(signed_distance: f32, half_width: f32, sigma: f32) -> f32 {
    if !signed_distance.is_finite() {
        return 0.0;
    }
    let half = if half_width.is_finite() {
        half_width.max(0.0)
    } else {
        0.0
    };
    let hi = gaussian_cdf(half - signed_distance, sigma);
    let lo = gaussian_cdf(-half - signed_distance, sigma);
    (hi - lo).clamp(0.0, 1.0)
}

fn shade(color: Color, coverage: f32) -> Color {
    Color::new(
        color.r,
        color.g,
        color.b,
        (color.a * coverage.clamp(0.0, 1.0)).clamp(0.0, 1.0),
    )
}

/// 覆盖率乘进 alpha。填充路径的内缩实体用它，避免和羽化环叠色。
pub(crate) fn shade_coverage(color: Color, coverage: f32) -> Color {
    shade(color, coverage)
}

/// 沿外法线偏移闭合轮廓。正距离向外。`normal_sign` 为 -1 时朝多边形内部，孔用这个方向扩边。
pub(crate) fn offset_outline_signed(
    outline: &[Vec2],
    distance: f32,
    normal_sign: f32,
) -> Vec<Vec2> {
    let ring = open_ring(outline);
    if ring.len() < 3 || !distance.is_finite() || !normal_sign.is_finite() {
        return Vec::new();
    }
    let normals = outward_normals(&ring);
    offset_ring(&ring, &normals, distance * normal_sign)
}

/// 只铺轮廓软边，不填内部。`normal_sign` 为 1 时高覆盖率在多边形内部；孔传 -1，避免把孔填实。
pub(crate) fn gaussian_edge_feather_signed(
    outline: &[Vec2],
    color: Color,
    sigma: f32,
    transform: &Transform,
    normal_sign: f32,
) -> Vec<Vertex> {
    let Some((radii, rings)) = silhouette_rings(outline, sigma, normal_sign) else {
        return Vec::new();
    };
    let mut vertices = Vec::new();
    for index in 0..SILHOUETTE_RINGS {
        vertices.extend(feather_strip(
            &rings[index],
            &rings[index + 1],
            shade(color, edge_coverage(radii[index], sigma)),
            shade(color, edge_coverage(radii[index + 1], sigma)),
            transform,
        ));
    }
    vertices
}

fn silhouette_rings(
    outline: &[Vec2],
    sigma: f32,
    normal_sign: f32,
) -> Option<(Vec<f32>, Vec<Vec<Vec2>>)> {
    let sigma = sanitize_blur(sigma);
    if sigma <= 1.0e-4 || !normal_sign.is_finite() {
        return None;
    }
    let ring = open_ring(outline);
    if ring.len() < 3 {
        return None;
    }
    let normals = outward_normals(&ring);
    let mut radii = Vec::with_capacity(SILHOUETTE_RINGS + 1);
    for index in 0..=SILHOUETTE_RINGS {
        let t = index as f32 / SILHOUETTE_RINGS as f32;
        radii.push((-BLUR_EXTENT + 2.0 * BLUR_EXTENT * t) * sigma);
    }
    let rings = radii
        .iter()
        .copied()
        .map(|distance| offset_ring(&ring, &normals, distance * normal_sign))
        .collect();
    Some((radii, rings))
}

/// 闭合轮廓的软填充。轮廓可以首尾重复。σ 接近 0 时返回空，调用方走原来的硬边。
pub(crate) fn gaussian_fill_vertices(
    outline: &[Vec2],
    color: Color,
    sigma: f32,
    transform: &Transform,
) -> Vec<Vertex> {
    let Some((radii, rings)) = silhouette_rings(outline, sigma, 1.0) else {
        return Vec::new();
    };
    let mut vertices = fan_fill(
        &rings[0],
        shade(color, edge_coverage(radii[0], sigma)),
        transform,
    );
    for index in 0..SILHOUETTE_RINGS {
        vertices.extend(feather_strip(
            &rings[index],
            &rings[index + 1],
            shade(color, edge_coverage(radii[index], sigma)),
            shade(color, edge_coverage(radii[index + 1], sigma)),
            transform,
        ));
    }
    vertices
}

/// 沿闭合轮廓的软描边。半宽是线宽的一半。σ 接近 0 时返回空。
pub(crate) fn gaussian_stroke_vertices(
    outline: &[Vec2],
    half_width: f32,
    color: Color,
    sigma: f32,
    transform: &Transform,
) -> Vec<Vertex> {
    let sigma = sanitize_blur(sigma);
    if sigma <= 1.0e-4 || !half_width.is_finite() || half_width < 0.0 {
        return Vec::new();
    }
    let ring = open_ring(outline);
    if ring.len() < 3 {
        return Vec::new();
    }
    let normals = outward_normals(&ring);
    let reach = half_width + BLUR_EXTENT * sigma;
    let mut radii = Vec::with_capacity(SILHOUETTE_RINGS + 1);
    for index in 0..=SILHOUETTE_RINGS {
        let t = index as f32 / SILHOUETTE_RINGS as f32;
        radii.push(-reach + 2.0 * reach * t);
    }
    let rings: Vec<Vec<Vec2>> = radii
        .iter()
        .copied()
        .map(|distance| offset_ring(&ring, &normals, distance))
        .collect();
    let mut vertices = Vec::new();
    for index in 0..SILHOUETTE_RINGS {
        vertices.extend(feather_strip(
            &rings[index],
            &rings[index + 1],
            shade(color, band_coverage(radii[index], half_width, sigma)),
            shade(color, band_coverage(radii[index + 1], half_width, sigma)),
            transform,
        ));
    }
    vertices
}

/// 开口笔画的胶囊剪影，再交给 [`gaussian_fill_vertices`]。
pub(crate) fn gaussian_capsule_vertices(
    centerline: &[Vec2],
    half_width: f32,
    color: Color,
    sigma: f32,
    transform: &Transform,
) -> Vec<Vertex> {
    let outline = capsule_outline(centerline, half_width);
    gaussian_fill_vertices(&outline, color, sigma, transform)
}

fn offset_ring(ring: &[Vec2], normals: &[Vec2], distance: f32) -> Vec<Vec2> {
    ring.iter()
        .zip(normals)
        .map(|(point, normal)| *point + *normal * distance)
        .collect()
}

/// 填充盖住内部，描边再盖在上面。颜色的 alpha 已经含元素不透明度。
///
/// σ 接近 0 时不要调用，硬边采样和阶跃会在边界上差一个像素。
pub(crate) fn blurred_paint(
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_half: f32,
    signed_distance: f32,
    sigma: f32,
) -> Option<Color> {
    let mut dst = Color::TRANSPARENT;
    let mut painted = false;
    if let Some(fill) = fill {
        let coverage = edge_coverage(signed_distance, sigma);
        if coverage > 1.0e-4 && fill.a > 0.0 {
            dst = shade(fill, coverage);
            painted = true;
        }
    }
    if let Some(stroke) = stroke {
        let coverage = band_coverage(signed_distance, stroke_half, sigma);
        if coverage > 1.0e-4 && stroke.a > 0.0 {
            let src = shade(stroke, coverage);
            dst = if painted {
                composite_over(src, dst)
            } else {
                src
            };
            painted = true;
        }
    }
    if painted && dst.a > 1.0e-4 {
        Some(dst)
    } else {
        None
    }
}

/// 剪影内部为负的有符号距离，模糊成一种颜色。直线和路径的笔画已经含在距离里。
pub(crate) fn blurred_mask(color: Color, signed_distance: f32, sigma: f32) -> Option<Color> {
    let coverage = edge_coverage(signed_distance, sigma);
    if coverage <= 1.0e-4 || color.a <= 0.0 {
        None
    } else {
        Some(shade(color, coverage))
    }
}

fn tap_weight(offset: i32) -> f32 {
    let t = offset as f32;
    (-0.5 * t * t).exp()
}

/// 5×5 高斯。`sample(i, j)` 为 `None` 时该点透明，和着色器把框外纹素当成 0 一样。
///
/// 权重按整个核归一，不按命中的点重归一，所以边缘会变淡。
pub(crate) fn kernel_color(mut sample: impl FnMut(i32, i32) -> Option<Color>) -> Option<Color> {
    let mut sum = [0.0_f32; 4];
    let mut weight = 0.0_f32;
    for j in -KERNEL_RADIUS..=KERNEL_RADIUS {
        for i in -KERNEL_RADIUS..=KERNEL_RADIUS {
            let w = tap_weight(i) * tap_weight(j);
            weight += w;
            let Some(color) = sample(i, j) else {
                continue;
            };
            sum[0] += color.r.clamp(0.0, 1.0) * w;
            sum[1] += color.g.clamp(0.0, 1.0) * w;
            sum[2] += color.b.clamp(0.0, 1.0) * w;
            sum[3] += color.a.clamp(0.0, 1.0) * w;
        }
    }
    if weight <= 1.0e-6 || sum[3] <= 1.0e-4 {
        return None;
    }
    Some(Color::new(
        sum[0] / weight,
        sum[1] / weight,
        sum[2] / weight,
        sum[3] / weight,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Vec2> {
        vec![
            Vec2::new(-10.0, -10.0),
            Vec2::new(10.0, -10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(-10.0, 10.0),
        ]
    }

    #[test]
    fn edge_coverage_is_half_on_the_boundary_and_solid_inside() {
        assert!((edge_coverage(0.0, 4.0) - 0.5).abs() < 1.0e-4);
        assert!(edge_coverage(-40.0, 4.0) > 0.99);
        assert!(edge_coverage(40.0, 4.0) < 1.0e-4);
        assert_eq!(edge_coverage(-1.0, 0.0), 1.0);
        assert_eq!(edge_coverage(1.0, 0.0), 0.0);
        assert_eq!(sanitize_blur(-3.0), 0.0);
        assert!(!blur_active(f32::NAN));
    }

    #[test]
    fn silhouette_rings_extend_past_the_hard_edge_and_fade() {
        let transform = Transform::identity();
        let color = Color::new(1.0, 0.0, 0.0, 1.0);
        let vertices = gaussian_fill_vertices(&square(), color, 4.0, &transform);
        assert!(vertices.len() > 12);
        let max_alpha = vertices
            .iter()
            .map(|vertex| vertex.color[3])
            .fold(0.0_f32, f32::max);
        let min_alpha = vertices
            .iter()
            .map(|vertex| vertex.color[3])
            .fold(1.0_f32, f32::min);
        assert!(max_alpha > 0.9);
        assert!(min_alpha < 0.05);
        assert!(vertices.iter().any(|vertex| vertex.position[0] > 12.0));
        assert!(gaussian_fill_vertices(&square(), color, 0.0, &transform).is_empty());
    }
}

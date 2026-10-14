//! Hit test utilities for picking elements in the scene.

use infinitecanvas_math::Vec2;

use crate::{ElementId, Scene};

/// Configuration for hit testing.
#[derive(Debug, Clone)]
pub struct HitTestOptions {
    /// Maximum number of results to return.
    pub max_results: usize,
    /// Whether to include invisible elements.
    pub include_invisible: bool,
}

impl Default for HitTestOptions {
    fn default() -> Self {
        Self {
            max_results: usize::MAX,
            include_invisible: false,
        }
    }
}

/// Result of a single hit test match.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    pub id: ElementId,
    pub z_index: i32,
    pub element_type: &'static str,
}

/// Hit test the scene at a world-space point. Results are returned from top-most element.
pub fn hit_test_point(scene: &Scene, point: Vec2, options: &HitTestOptions) -> Vec<HitTestResult> {
    if scene.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut elements = scene.elements_sorted();
    elements.reverse(); // highest z-index first

    for element in elements {
        if !options.include_invisible && !element.as_element().props().visible {
            continue;
        }

        if element.as_element().contains_point(point) {
            results.push(HitTestResult {
                id: element.id(),
                z_index: element.as_element().props().z_index,
                element_type: element.as_element().type_name(),
            });

            if results.len() >= options.max_results {
                break;
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Element, Rectangle};
    use crate::Scene;
    use infinitecanvas_math::Vec2;

    #[test]
    fn returns_top_most_first() {
        let mut scene = Scene::new();
        let mut rect_low = Rectangle::new(1, Vec2::ZERO, Vec2::new(100.0, 100.0));
        rect_low.props_mut().z_index = 1;
        scene.add_rect(rect_low);

        let mut rect_high = Rectangle::new(2, Vec2::ZERO, Vec2::new(100.0, 100.0));
        rect_high.props_mut().z_index = 10;
        scene.add_rect(rect_high);

        let options = HitTestOptions {
            max_results: 1,
            include_invisible: false,
        };
        let hits = hit_test_point(&scene, Vec2::ZERO, &options);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 2);
    }

    #[test]
    fn respects_visibility_and_limit() {
        let mut scene = Scene::new();
        let mut visible = Rectangle::new(1, Vec2::ZERO, Vec2::new(50.0, 50.0));
        visible.props_mut().z_index = 5;
        scene.add_rect(visible);

        let mut hidden = Rectangle::new(2, Vec2::ZERO, Vec2::new(50.0, 50.0));
        hidden.props_mut().visible = false;
        hidden.props_mut().z_index = 20;
        scene.add_rect(hidden);

        // Default options skip invisible elements
        let hits = hit_test_point(&scene, Vec2::ZERO, &HitTestOptions::default());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 1);

        // Include invisible elements and cap limit
        let hits = hit_test_point(
            &scene,
            Vec2::ZERO,
            &HitTestOptions {
                include_invisible: true,
                max_results: 2,
            },
        );
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, 2); // invisible but higher z-index
    }

    #[test]
    fn rotated_rectangle_hit_uses_oriented_bounds() {
        let mut rect = Rectangle::new(1, Vec2::ZERO, Vec2::new(100.0, 20.0));
        rect.set_rotation(std::f32::consts::FRAC_PI_4);
        // 沿长轴仍在矩形内。
        let along = Vec2::new(28.28, 28.28);
        assert!(rect.contains_point(along));
        assert_eq!(
            hit_test_point(
                &{
                    let mut scene = Scene::new();
                    scene.add_rect(rect.clone());
                    scene
                },
                along,
                &HitTestOptions::default(),
            )
            .len(),
            1
        );
        // 轴对齐包围盒的角落在旋转后的矩形外面。
        assert!(!rect.contains_point(Vec2::new(42.0, 42.0)));
        assert!(hit_test_point(
            &{
                let mut scene = Scene::new();
                scene.add_rect(rect);
                scene
            },
            Vec2::new(42.0, 42.0),
            &HitTestOptions::default(),
        )
        .is_empty());
    }

    #[test]
    fn rotated_circle_hit_follows_scaled_axes() {
        use crate::element::Circle;
        let mut circle = Circle::new(1, Vec2::ZERO, 10.0);
        circle.props_mut().transform.scale = Vec2::new(3.0, 1.0);
        circle.set_rotation(std::f32::consts::FRAC_PI_2);
        let mut scene = Scene::new();
        scene.add_circle(circle);
        // 90° 之后，原先的 X 拉伸转到 Y。
        assert_eq!(
            hit_test_point(&scene, Vec2::new(0.0, 25.0), &HitTestOptions::default()).len(),
            1
        );
        assert!(
            hit_test_point(&scene, Vec2::new(25.0, 0.0), &HitTestOptions::default()).is_empty()
        );
    }

    #[test]
    fn rotated_line_and_path_follow_transform() {
        use crate::element::{ElementStyle, Line, Path};
        use infinitecanvas_math::Color;

        let mut line = Line::from_world(
            1,
            Vec2::ZERO,
            Vec2::new(100.0, 0.0),
            ElementStyle::stroke_only(Color::BLACK, 4.0),
            false,
        );
        line.set_rotation(std::f32::consts::FRAC_PI_2);
        assert!(line.contains_point(Vec2::new(0.0, 50.0)));
        assert!(!line.contains_point(Vec2::new(50.0, 0.0)));

        let mut path = Path::from_world_points(
            2,
            &[Vec2::ZERO, Vec2::new(80.0, 0.0)],
            ElementStyle::stroke_only(Color::BLACK, 4.0),
        );
        path.set_rotation(std::f32::consts::FRAC_PI_2);
        assert!(path.contains_point(Vec2::new(0.0, 40.0)));
        assert!(!path.contains_point(Vec2::new(40.0, 0.0)));

        let mut scene = Scene::new();
        scene.add_line(line);
        scene.add_path(path);
        let hits = hit_test_point(&scene, Vec2::new(0.0, 40.0), &HitTestOptions::default());
        assert!(hits.iter().any(|hit| hit.id == 2));
        assert!(
            hit_test_point(&scene, Vec2::new(40.0, 0.0), &HitTestOptions::default()).is_empty()
        );
    }
}

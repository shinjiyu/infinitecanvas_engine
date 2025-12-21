//! Hit test utilities for picking elements in the scene.

use infinitecanvas_math::Vec2;

use crate::{element::Element, ElementId, Scene};

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
pub fn hit_test_point(
    scene: &Scene,
    point: Vec2,
    options: &HitTestOptions,
) -> Vec<HitTestResult> {
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
    use crate::{element::Rectangle, Scene};
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
}

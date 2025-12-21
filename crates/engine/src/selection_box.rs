//! Selection box state used for drag-to-select interactions.

use infinitecanvas_math::{Rect, Vec2};

#[derive(Debug, Clone)]
pub struct SelectionBox {
    active: bool,
    pointer_id: Option<i32>,
    start: Vec2,
    current: Vec2,
    additive: bool,
}

impl SelectionBox {
    pub fn new() -> Self {
        Self {
            active: false,
            pointer_id: None,
            start: Vec2::ZERO,
            current: Vec2::ZERO,
            additive: false,
        }
    }

    pub fn start(&mut self, pointer_id: i32, start: Vec2, additive: bool) {
        self.active = true;
        self.pointer_id = Some(pointer_id);
        self.start = start;
        self.current = start;
        self.additive = additive;
    }

    pub fn update(&mut self, pointer_id: i32, current: Vec2) {
        if self.is_pointer(pointer_id) {
            self.current = current;
        }
    }

    pub fn finish(&mut self, pointer_id: i32) -> Option<(Rect, bool)> {
        if !self.is_pointer(pointer_id) {
            return None;
        }

        let rect = self.bounds();
        let additive = self.additive;
        self.cancel();

        rect.map(|r| (r, additive))
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.pointer_id = None;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn bounds(&self) -> Option<Rect> {
        if !self.active {
            return None;
        }

        let min = Vec2::new(self.start.x.min(self.current.x), self.start.y.min(self.current.y));
        let max = Vec2::new(self.start.x.max(self.current.x), self.start.y.max(self.current.y));
        Some(Rect::from_corners(min, max))
    }

    fn is_pointer(&self, pointer_id: i32) -> bool {
        self.active && self.pointer_id == Some(pointer_id)
    }
}

impl Default for SelectionBox {
    fn default() -> Self {
        Self::new()
    }
}

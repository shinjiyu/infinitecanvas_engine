//! Input event definitions and queue utilities.

use std::collections::VecDeque;

use infinitecanvas_math::Vec2;

/// Modifier key state snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const fn new(shift: bool, ctrl: bool, alt: bool, meta: bool) -> Self {
        Self {
            shift,
            ctrl,
            alt,
            meta,
        }
    }

    pub const fn none() -> Self {
        Self::new(false, false, false, false)
    }
}

/// Pointer button pressed/released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Aux(u16),
}

/// Pointer input payload.
#[derive(Debug, Clone, Copy)]
pub struct PointerEvent {
    pub pointer_id: i32,
    pub screen_position: Vec2,
    pub world_position: Vec2,
    pub screen_delta: Vec2,
    pub world_delta: Vec2,
    pub button: Option<PointerButton>,
    pub buttons_mask: u16,
    pub modifiers: Modifiers,
}

/// Wheel delta interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelDeltaMode {
    Pixel,
    Line,
    Page,
}

/// Wheel input payload.
#[derive(Debug, Clone, Copy)]
pub struct WheelEvent {
    pub delta: Vec2,
    pub mode: WheelDeltaMode,
    pub modifiers: Modifiers,
}

/// Keyboard event state.
#[derive(Debug, Clone)]
pub struct KeyboardEvent {
    pub key: String,
    pub code: String,
    pub is_repeat: bool,
    pub modifiers: Modifiers,
}

/// High level input event.
#[derive(Debug, Clone)]
pub enum Event {
    PointerDown(PointerEvent),
    PointerMove(PointerEvent),
    PointerUp(PointerEvent),
    Wheel(WheelEvent),
    KeyDown(KeyboardEvent),
    KeyUp(KeyboardEvent),
}

/// FIFO queue for input events.
#[derive(Default)]
pub struct EventQueue {
    events: VecDeque<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    pub fn push(&mut self, event: Event) {
        self.events.push_back(event);
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn drain(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }
}

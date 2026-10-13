use infinitecanvas_engine::{
    event::{
        Event, KeyboardEvent, Modifiers, PointerButton, PointerEvent, WheelDeltaMode, WheelEvent,
    },
    math::Vec2,
    Viewport,
};

/// Supported pointer event kinds from JS.
pub enum PointerEventKind {
    Down,
    Move,
    Up,
}

impl std::str::FromStr for PointerEventKind {
    type Err = ();

    fn from_str(kind: &str) -> Result<Self, Self::Err> {
        match kind {
            "down" => Ok(Self::Down),
            "move" => Ok(Self::Move),
            "up" => Ok(Self::Up),
            _ => Err(()),
        }
    }
}

impl PointerEventKind {
    pub fn into_event(self, payload: PointerEvent) -> Event {
        match self {
            Self::Down => Event::PointerDown(payload),
            Self::Move => Event::PointerMove(payload),
            Self::Up => Event::PointerUp(payload),
        }
    }
}

pub fn pointer_button_from_i16(button: i16) -> Option<PointerButton> {
    match button {
        0 => Some(PointerButton::Primary),
        1 => Some(PointerButton::Middle),
        2 => Some(PointerButton::Secondary),
        value if value >= 0 => Some(PointerButton::Aux(value as u16)),
        _ => None,
    }
}

pub fn modifiers_from_flags(shift: bool, ctrl: bool, alt: bool, meta: bool) -> Modifiers {
    Modifiers::new(shift, ctrl, alt, meta)
}

pub fn pointer_event_from_js(
    kind: PointerEventKind,
    pointer_id: i32,
    screen_pos: Vec2,
    screen_delta: Vec2,
    button: i16,
    buttons_mask: u16,
    modifiers: Modifiers,
    viewport: &Viewport,
) -> Event {
    let world_position = viewport.screen_to_world(screen_pos);
    let zoom = viewport.zoom();
    let zoom_safe = if zoom.abs() < f32::EPSILON { 1.0 } else { zoom };
    let world_delta = screen_delta / zoom_safe;

    let payload = PointerEvent {
        pointer_id,
        screen_position: screen_pos,
        world_position,
        screen_delta,
        world_delta,
        button: pointer_button_from_i16(button),
        buttons_mask,
        modifiers,
    };

    kind.into_event(payload)
}

pub fn wheel_event_from_js(delta: Vec2, mode: u32, modifiers: Modifiers) -> Event {
    let mode = match mode {
        0 => WheelDeltaMode::Pixel,
        1 => WheelDeltaMode::Line,
        2 => WheelDeltaMode::Page,
        _ => WheelDeltaMode::Pixel,
    };

    Event::Wheel(WheelEvent {
        delta,
        mode,
        modifiers,
    })
}

pub enum KeyboardEventKind {
    Down,
    Up,
}

impl std::str::FromStr for KeyboardEventKind {
    type Err = ();

    fn from_str(kind: &str) -> Result<Self, Self::Err> {
        match kind {
            "down" => Ok(Self::Down),
            "up" => Ok(Self::Up),
            _ => Err(()),
        }
    }
}

impl KeyboardEventKind {
    pub fn into_event(self, payload: KeyboardEvent) -> Event {
        match self {
            Self::Down => Event::KeyDown(payload),
            Self::Up => Event::KeyUp(payload),
        }
    }
}

pub fn keyboard_event_from_js(
    kind: KeyboardEventKind,
    key: String,
    code: String,
    is_repeat: bool,
    modifiers: Modifiers,
) -> Event {
    let payload = KeyboardEvent {
        key,
        code,
        is_repeat,
        modifiers,
    };

    kind.into_event(payload)
}

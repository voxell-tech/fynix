use fynix::Fynix;
use fynix_hit_test::HitTest;
use fynix_interactions::{
    ButtonRole, ClassifyButton, PointerRecognizer,
};
use vello::kurbo::Point;
use winit::event::{ElementState, MouseButton, Touch, TouchPhase};

/// The mouse pointer id. Touch ids are shifted by one so none can
/// alias it.
const MOUSE: u64 = 0;

/// A winit mouse button the recognizer can classify.
///
/// The orphan rule forbids implementing [`ClassifyButton`] on
/// `winit::MouseButton` directly, so it is wrapped here.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Button(MouseButton);

impl ClassifyButton for Button {
    fn role(self) -> ButtonRole {
        match self.0 {
            MouseButton::Left => ButtonRole::Primary,
            MouseButton::Right => ButtonRole::Secondary,
            MouseButton::Middle => ButtonRole::Middle,
            _ => ButtonRole::Other,
        }
    }
}

/// Translates winit pointer events into recognizer calls.
///
/// A thin adapter over [`PointerRecognizer`]: this is the part that
/// would move into a future `fynix_winit` crate.
#[derive(Default)]
pub struct WinitInput {
    rec: PointerRecognizer<u64, Button>,
}

impl WinitInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_cursor_moved(
        &mut self,
        fynix: &mut Fynix,
        hit: &HitTest,
        pos: Point,
    ) {
        self.rec.pointer_moved(fynix, hit, pos);
    }

    pub fn on_mouse_button(
        &mut self,
        fynix: &mut Fynix,
        hit: &HitTest,
        state: ElementState,
        button: MouseButton,
        cursor: Point,
    ) {
        let button = Button(button);
        match state {
            ElementState::Pressed => {
                self.rec.pointer_down(hit, MOUSE, button, cursor);
            }
            ElementState::Released => {
                self.rec
                    .pointer_up(fynix, hit, MOUSE, button, cursor);
            }
        }
    }

    pub fn on_touch(
        &mut self,
        fynix: &mut Fynix,
        hit: &HitTest,
        touch: Touch,
    ) {
        let id = touch.id.wrapping_add(1);
        let pos = Point::new(touch.location.x, touch.location.y);
        // Touch contacts have no distinct button, so they act as the
        // primary button.
        let button = Button(MouseButton::Left);
        match touch.phase {
            TouchPhase::Started => {
                self.rec.pointer_down(hit, id, button, pos);
            }
            TouchPhase::Moved => {
                self.rec.pointer_moved(fynix, hit, pos);
            }
            TouchPhase::Ended => {
                self.rec.pointer_up(fynix, hit, id, button, pos);
            }
            TouchPhase::Cancelled => {
                self.rec.cancel(id);
            }
        }
    }
}

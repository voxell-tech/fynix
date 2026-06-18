use core::hash::Hash;

use fynix::Fynix;
use fynix::element::ElementId;
use fynix_hit_test::{Hit, HitTest};
use hashbrown::HashMap;
use spatree::kurbo::Point;

use super::identity::{ButtonRole, PointerId};
use super::interaction::{
    Drag, DragEnd, DragStart, Hover, PointerEnter, PointerLeave,
    PrimaryClick, RawClick, SecondaryClick,
};

/// Tuning for the recognizer's click-vs-drag decision.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Max distance (px) a press may travel and still count as a
    /// click rather than a drag.
    pub click_slop: f64,
    /// Distance (px) a press must travel before it becomes a drag.
    pub drag_threshold: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            click_slop: 4.0,
            drag_threshold: 8.0,
        }
    }
}

/// An in-progress press: tracked from button-down until release or
/// cancel.
#[derive(Debug, Clone, Copy)]
struct Press {
    target: ElementId,
    button: ButtonRole,
    /// Where the press began, in absolute coordinates.
    press_global: Point,
    /// Press position relative to the target's origin.
    press_local: Point,
    /// Pointer position at the previous drag event, for per-move
    /// deltas.
    last_global: Point,
    /// Whether the press has crossed the drag threshold.
    dragging: bool,
}

/// Per-pointer recognizer state.
#[derive(Debug, Clone, Copy, Default)]
struct PointerState {
    /// The element the pointer is currently over, while not pressed.
    hovering: Option<ElementId>,
    /// The active press, if a button is held.
    press: Option<Press>,
}

/// A per-pointer gesture state machine driven by the `down`, `moved`,
/// `up`, and `cancel` methods, dispatching the crate's interaction
/// types through `fynix`.
///
/// State is keyed by the whole [`PointerId`], so devices and fingers
/// never collide. A press is either a click or a drag: crossing the
/// drag threshold starts a drag (suppressing the click) and captures
/// the pointer to the press target.
pub struct PointerRecognizer<Device, Pointer> {
    config: Config,
    pointers: HashMap<PointerId<Device, Pointer>, PointerState>,
}

impl<Device, Pointer> PointerRecognizer<Device, Pointer>
where
    Device: Copy + Eq + Hash + 'static,
    Pointer: Copy + Eq + Hash + 'static,
{
    /// Creates a recognizer with the given tuning.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            pointers: HashMap::new(),
        }
    }

    /// Records a button-down. A press starts only on a hit target,
    /// and is held until release or cancel. `role` is the pressed
    /// button's classified role, and `hit` is the element resolved
    /// under `pos`.
    pub fn down(
        &mut self,
        id: PointerId<Device, Pointer>,
        role: ButtonRole,
        pos: Point,
        hit: Option<Hit>,
    ) {
        let Some(hit) = hit else {
            return;
        };
        let state = self.pointers.entry(id).or_default();
        state.press = Some(Press {
            target: hit.id,
            button: role,
            press_global: pos,
            press_local: hit.local,
            last_global: pos,
            dragging: false,
        });
    }

    /// Records a pointer move. While pressed: cross the threshold
    /// into a drag, then emit a `Drag` per move. While not pressed:
    /// emit hover/enter/leave. `hit` is the element resolved under
    /// `pos`.
    pub fn moved<W: 'static>(
        &mut self,
        id: PointerId<Device, Pointer>,
        pos: Point,
        hit: Option<Hit>,
        fynix: &mut Fynix<W>,
        world: &mut W,
        hit_test: &HitTest,
    ) {
        let state = self.pointers.entry(id).or_default();

        if let Some(press) = state.press.as_mut() {
            let threshold = self.config.drag_threshold;
            if !press.dragging {
                let travel = (pos - press.press_global).hypot2();
                if travel <= threshold * threshold {
                    return;
                }
                press.dragging = true;
                press.last_global = pos;
                dispatch_drag(
                    fynix,
                    world,
                    press.target,
                    DragStart {
                        id,
                        start: press.press_global,
                        local: press.press_local,
                    },
                );
                return;
            }

            let delta = pos - press.last_global;
            let local =
                press.press_local + (pos - press.press_global);
            press.last_global = pos;
            dispatch_drag(
                fynix,
                world,
                press.target,
                Drag { id, local, delta },
            );
            return;
        }

        // Not pressed: hover semantics on the topmost element.
        let new_top = hit.map(|h| h.id);
        if new_top != state.hovering {
            if let Some(old) = state.hovering {
                fynix.dispatch(&old, PointerLeave { id }, world);
            }
            if let (Some(top), Some(hit)) = (new_top, hit) {
                fynix.dispatch(
                    &top,
                    PointerEnter {
                        id,
                        local: hit.local,
                    },
                    world,
                );
            }
            state.hovering = new_top;
        }
        if let Some(hit) = hit {
            fynix.dispatch_bubbling(
                &hit.id,
                Hover {
                    id,
                    local: hit.local,
                },
                world,
                |e| hit_test.contains(e, pos),
            );
        }
    }

    /// Records a button-up: end a drag, or fire a click if the press
    /// stayed on its target within click slop. `hit` is the element
    /// resolved under `pos`.
    pub fn up<W: 'static>(
        &mut self,
        id: PointerId<Device, Pointer>,
        pos: Point,
        hit: Option<Hit>,
        fynix: &mut Fynix<W>,
        world: &mut W,
        hit_test: &HitTest,
    ) {
        let Some(state) = self.pointers.get_mut(&id) else {
            return;
        };
        let Some(press) = state.press.take() else {
            return;
        };
        // After release the pointer hovers wherever it landed.
        state.hovering = hit.map(|h| h.id);

        if press.dragging {
            let local =
                press.press_local + (pos - press.press_global);
            dispatch_drag(
                fynix,
                world,
                press.target,
                DragEnd { id, local },
            );
            return;
        }

        // Click candidate: released on the press target within slop.
        let Some(hit) = hit else {
            return;
        };
        if hit.id != press.target {
            return;
        }
        let travel = (pos - press.press_global).hypot2();
        if travel > self.config.click_slop * self.config.click_slop {
            return;
        }

        let gate = |e: &ElementId| hit_test.contains(e, pos);
        fynix.dispatch_bubbling(
            &press.target,
            RawClick {
                id,
                local: hit.local,
            },
            world,
            gate,
        );
        match press.button {
            ButtonRole::Primary => {
                fynix.dispatch_bubbling(
                    &press.target,
                    PrimaryClick {
                        id,
                        local: hit.local,
                    },
                    world,
                    gate,
                );
            }
            ButtonRole::Secondary => {
                fynix.dispatch_bubbling(
                    &press.target,
                    SecondaryClick {
                        id,
                        local: hit.local,
                    },
                    world,
                    gate,
                );
            }
            ButtonRole::Middle | ButtonRole::Other => {}
        }
    }

    /// Aborts a pending click and ends any active drag so handlers
    /// can clean up.
    pub fn cancel<W: 'static>(
        &mut self,
        id: PointerId<Device, Pointer>,
        fynix: &mut Fynix<W>,
        world: &mut W,
    ) {
        let Some(state) = self.pointers.get_mut(&id) else {
            return;
        };
        if let Some(press) = state.press.take()
            && press.dragging
        {
            let local = press.press_local
                + (press.last_global - press.press_global);
            dispatch_drag(
                fynix,
                world,
                press.target,
                DragEnd { id, local },
            );
        }
    }
}

/// Dispatches a drag interaction to the captured target, ungated:
/// the pointer may roam outside the target's bounds mid-drag, so
/// bubbling is not stopped by the hit-test edge.
fn dispatch_drag<I: 'static + Copy, W: 'static>(
    fynix: &mut Fynix<W>,
    world: &mut W,
    target: ElementId,
    interaction: I,
) {
    fynix.dispatch_bubbling(&target, interaction, world, |_| true);
}

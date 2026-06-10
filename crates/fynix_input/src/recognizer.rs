use alloc::vec::Vec;

use fynix::Fynix;
use fynix::element::ElementId;

use crate::hit_test::HitTest;
use crate::interaction::{Click, PointerEnter, PointerLeave};
use crate::raw_input::{
    PointerButton, PointerId, RawInput, RawInputKind,
};

/// Maximum distance, in absolute pixels, a pointer may travel between
/// press and release while still counting as a click rather than a
/// drag.
const CLICK_SLOP: f64 = 10.0;

/// Recognizes click gestures from raw pointer input.
///
/// Tracks each in-flight button press and, on release, emits a
/// [`Click`] to the element under the pointer when the press and
/// release landed on the same element within a small slop distance.
///
/// Hit-testing and dispatch are supplied by the caller: the backend
/// builds a [`HitTest`] after layout and hands it, along with the
/// [`Fynix`] context, to [`Self::handle`].
#[derive(Default)]
pub struct PointerRecognizer {
    /// One entry per button currently held down. Few buttons are
    /// ever down at once, so a linear scan is cheaper than a
    /// map.
    presses: Vec<Press>,
    /// The topmost hit-tested element the mouse pointer is currently
    /// over, tracked to emit enter/leave when it changes.
    hovered: Option<ElementId>,
}

struct Press {
    pointer: PointerId,
    button: PointerButton,
    x: f64,
    y: f64,
    /// The element under the press, if any. The release must resolve
    /// to this same element for a click.
    target: Option<ElementId>,
}

impl PointerRecognizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one raw event through the recognizer, dispatching any
    /// interaction it produces into `fynix`.
    pub fn handle(
        &mut self,
        fynix: &mut Fynix,
        hit_test: &HitTest,
        input: &RawInput,
    ) {
        match input.kind {
            RawInputKind::PointerDown {
                pointer,
                button,
                x,
                y,
            } => {
                let target = hit_test.query(x, y).map(|hit| hit.id);
                self.presses.push(Press {
                    pointer,
                    button,
                    x,
                    y,
                    target,
                });
            }
            RawInputKind::PointerUp {
                pointer,
                button,
                x,
                y,
            } => {
                let Some(press) = self.take_press(pointer, button)
                else {
                    return;
                };
                let Some(target) = press.target else {
                    return;
                };

                let Some(hit) = hit_test.query(x, y) else {
                    return;
                };
                if hit.id != target || moved_past_slop(&press, x, y) {
                    return;
                }

                fynix.dispatch_bubbling::<Click>(
                    &hit.id,
                    Click {
                        pointer,
                        button,
                        local_x: hit.local_x,
                        local_y: hit.local_y,
                    },
                    // Only bubble to ancestors still under the
                    // release point.
                    |id| hit_test.contains(id, x, y),
                );
            }
            RawInputKind::PointerMoved { pointer, x, y } => {
                let target = hit_test.query(x, y).map(|hit| hit.id);
                if target == self.hovered {
                    return;
                }

                if let Some(left) = self.hovered {
                    // The pointer has moved off `left`, so the
                    // hit-test no longer covers it: always deliver.
                    fynix.dispatch_bubbling::<PointerLeave>(
                        &left,
                        PointerLeave { pointer },
                        |_| true,
                    );
                }
                if let Some(entered) = target {
                    fynix.dispatch_bubbling::<PointerEnter>(
                        &entered,
                        PointerEnter { pointer },
                        |id| hit_test.contains(id, x, y),
                    );
                }

                self.hovered = target;
            }
        }
    }

    /// Removes and returns the press matching `pointer` and `button`,
    /// if one is in flight.
    fn take_press(
        &mut self,
        pointer: PointerId,
        button: PointerButton,
    ) -> Option<Press> {
        let i = self.presses.iter().position(|press| {
            press.pointer == pointer && press.button == button
        })?;
        Some(self.presses.swap_remove(i))
    }
}

/// Returns `true` when the release at `(x, y)` is further than
/// [`CLICK_SLOP`] from where the press began.
fn moved_past_slop(press: &Press, x: f64, y: f64) -> bool {
    let dx = x - press.x;
    let dy = y - press.y;
    dx * dx + dy * dy > CLICK_SLOP * CLICK_SLOP
}

#[cfg(test)]
mod tests {
    use fynix::Fynix;
    use fynix::element::layout::ElementNodes;
    use fynix::element::{Element, ElementBuild, ElementId};
    use fynix::init::Init;
    use fynix::prelude::{Constraint, Size, Vec2};

    use super::*;
    use crate::is_hit_target;

    /// Builds the hit-test index, indexing only elements with a click
    /// handler (the production `include` predicate).
    fn hit_test(fynix: &Fynix, root: &ElementId) -> HitTest {
        HitTest::build(&fynix.elements, root, |id| {
            is_hit_target(&fynix.interactions, id)
        })
    }

    #[derive(Init, Element)]
    struct Tile;

    impl ElementBuild for Tile {
        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            _nodes: &mut ElementNodes,
        ) -> Size {
            constraint.min
        }
    }

    #[derive(Debug, PartialEq)]
    struct Clicked;

    /// Builds a single `Tile` with a click handler, positioned at
    /// `(10, 10)` sized `100 x 50` in absolute space.
    fn tile_at(fynix: &mut Fynix) -> ElementId {
        let mut world = ();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Tile>()
                .on::<Click>(|_, events| events.push(Clicked))
                .id()
        };

        let meta = fynix.elements.metas.get_mut(&id).unwrap();
        meta.node.world_translation = Vec2::new(10.0, 10.0);
        meta.node.size = Size::new(100.0, 50.0);
        id
    }

    fn down(x: f64, y: f64) -> RawInput {
        RawInput {
            time: 0,
            kind: RawInputKind::PointerDown {
                pointer: PointerId::MOUSE,
                button: PointerButton::Primary,
                x,
                y,
            },
        }
    }

    fn up(x: f64, y: f64) -> RawInput {
        RawInput {
            time: 0,
            kind: RawInputKind::PointerUp {
                pointer: PointerId::MOUSE,
                button: PointerButton::Primary,
                x,
                y,
            },
        }
    }

    fn moved(x: f64, y: f64) -> RawInput {
        RawInput {
            time: 0,
            kind: RawInputKind::PointerMoved {
                pointer: PointerId::MOUSE,
                x,
                y,
            },
        }
    }

    fn click_count(fynix: &Fynix) -> usize {
        fynix.events.iter::<Clicked>().count()
    }

    #[test]
    fn press_and_release_inside_dispatches_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = PointerRecognizer::new();

        rec.handle(&mut fynix, &hit, &down(20.0, 20.0));
        rec.handle(&mut fynix, &hit, &up(25.0, 25.0));

        assert_eq!(click_count(&fynix), 1);
    }

    #[test]
    fn release_outside_element_does_not_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = PointerRecognizer::new();

        rec.handle(&mut fynix, &hit, &down(20.0, 20.0));
        // (200, 200) misses the tile entirely.
        rec.handle(&mut fynix, &hit, &up(200.0, 200.0));

        assert_eq!(click_count(&fynix), 0);
    }

    #[test]
    fn drag_past_slop_does_not_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = PointerRecognizer::new();

        rec.handle(&mut fynix, &hit, &down(20.0, 20.0));
        // Still inside the tile, but 12px away (> CLICK_SLOP).
        rec.handle(&mut fynix, &hit, &up(32.0, 20.0));

        assert_eq!(click_count(&fynix), 0);
    }

    #[test]
    fn press_outside_then_release_inside_does_not_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = PointerRecognizer::new();

        // Press misses everything, so there is no target.
        rec.handle(&mut fynix, &hit, &down(200.0, 200.0));
        rec.handle(&mut fynix, &hit, &up(20.0, 20.0));

        assert_eq!(click_count(&fynix), 0);
    }

    #[derive(Debug, PartialEq)]
    struct Entered;

    #[derive(Debug, PartialEq)]
    struct Left;

    #[test]
    fn pointer_move_emits_enter_then_leave() {
        let mut fynix = Fynix::new();
        let mut world = ();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Tile>()
                .on::<PointerEnter>(|_, events| events.push(Entered))
                .on::<PointerLeave>(|_, events| events.push(Left))
                .id()
        };
        let meta = fynix.elements.metas.get_mut(&id).unwrap();
        meta.node.world_translation = Vec2::new(10.0, 10.0);
        meta.node.size = Size::new(100.0, 50.0);

        let hit = hit_test(&fynix, &id);
        let mut rec = PointerRecognizer::new();

        // Move onto the tile: one enter, no leave.
        rec.handle(&mut fynix, &hit, &moved(20.0, 20.0));
        assert_eq!(fynix.events.iter::<Entered>().count(), 1);
        assert_eq!(fynix.events.iter::<Left>().count(), 0);

        // Move within the tile: no further enter/leave.
        rec.handle(&mut fynix, &hit, &moved(30.0, 30.0));
        assert_eq!(fynix.events.iter::<Entered>().count(), 1);

        // Move off the tile: one leave.
        rec.handle(&mut fynix, &hit, &moved(200.0, 200.0));
        assert_eq!(fynix.events.iter::<Left>().count(), 1);
    }
}

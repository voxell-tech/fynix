//! Pointer interactions: button classification and the recognizer
//! that turns semantic pointer actions into [`Click`]-style events.
//!
//! [Easter Egg](https://youtu.be/vsD70GAtVac?si=PRQqJq7WZwtY3iMT)

use alloc::vec::Vec;

use fynix::Fynix;
use fynix::element::ElementId;
use fynix_hit_test::HitTest;
use spatree::kurbo::Point;

use crate::interaction::{
    Click, PointerEnter, PointerLeave, SecondaryClick,
};

/// The semantic role of a pointer button.
///
/// Backends classify their native buttons into one of these through
/// [`ClassifyButton`], so the recognizer can pick which interaction
/// to emit without ever naming the native button type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonRole {
    Primary,
    Secondary,
    Middle,
    Other,
}

/// A native pointer button the recognizer can classify.
///
/// The recognizer stays generic over the backend's own button type
/// and only asks it for a [`ButtonRole`], so primary versus secondary
/// is decided once, in the backend, rather than leaking into every
/// interaction handler.
///
/// Implement this for a newtype wrapping the backend button (the
/// orphan rule forbids implementing it on the foreign type directly).
pub trait ClassifyButton: Copy + Eq {
    fn role(self) -> ButtonRole;
}

/// Maximum distance, in absolute pixels, a pointer may travel between
/// press and release while still counting as a click rather than a
/// drag.
const CLICK_SLOP: f64 = 10.0;

/// Recognizes pointer gestures from semantic pointer actions.
///
/// Generic over the backend's own pointer id and button types: `Id`
/// only needs to be comparable (to correlate a release with its
/// press), and `Btn` is classified into a [`ButtonRole`] to choose
/// which interaction to emit. Backends translate native events into
/// calls on [`Self::pointer_down`], [`Self::pointer_up`], and
/// [`Self::pointer_moved`], so no raw-input envelope is constructed.
///
/// Hit-testing and dispatch are supplied by the caller: the backend
/// builds a [`HitTest`] after layout and hands it, along with the
/// [`Fynix`] context, to each method.
pub struct PointerRecognizer<Id, Btn> {
    /// One entry per button currently held down. Few buttons are
    /// ever down at once, so a linear scan is cheaper than a
    /// map.
    presses: Vec<Press<Id, Btn>>,
    /// The topmost hit-tested element the pointer is currently over,
    /// tracked to emit enter/leave when it changes.
    hovered: Option<ElementId>,
}

struct Press<Id, Btn> {
    pointer: Id,
    button: Btn,
    pos: Point,
    /// The element under the press, if any. The release must resolve
    /// to this same element for a click.
    target: Option<ElementId>,
}

impl<Id, Btn> Default for PointerRecognizer<Id, Btn> {
    fn default() -> Self {
        Self {
            presses: Vec::new(),
            hovered: None,
        }
    }
}

impl<Id, Btn> PointerRecognizer<Id, Btn>
where
    Id: Copy + Eq,
    Btn: ClassifyButton,
{
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a button going down. Nothing is dispatched yet; the
    /// element under the press is captured so the matching release
    /// can confirm the click landed on the same element.
    pub fn pointer_down(
        &mut self,
        hit_test: &HitTest,
        pointer: Id,
        button: Btn,
        pos: Point,
    ) {
        let target = hit_test.query(pos).map(|hit| hit.id);
        self.presses.push(Press {
            pointer,
            button,
            pos,
            target,
        });
    }

    /// Resolves a button going up against its press, dispatching the
    /// interaction depending on its [`ButtonRole`].
    pub fn pointer_up(
        &mut self,
        fynix: &mut Fynix,
        hit_test: &HitTest,
        pointer: Id,
        button: Btn,
        pos: Point,
    ) {
        let Some(press) = self.take_press(pointer, button) else {
            return;
        };
        let Some(target) = press.target else {
            return;
        };

        let Some(hit) = hit_test.query(pos) else {
            return;
        };
        if hit.id != target || moved_past_slop(&press, pos) {
            return;
        }

        // Only bubble to ancestors still under the release point.
        let gate = |id: &ElementId| hit_test.contains(id, pos);
        match button.role() {
            ButtonRole::Primary => {
                fynix.dispatch_bubbling::<Click>(
                    &hit.id,
                    Click { local: hit.local },
                    gate,
                );
            }
            ButtonRole::Secondary => {
                fynix.dispatch_bubbling::<SecondaryClick>(
                    &hit.id,
                    SecondaryClick { local: hit.local },
                    gate,
                );
            }
            ButtonRole::Middle | ButtonRole::Other => {}
        }
    }

    /// Updates the hovered element, emitting [`PointerLeave`] for the
    /// element left and [`PointerEnter`] for the one entered when the
    /// topmost element under the pointer changes.
    pub fn pointer_moved(
        &mut self,
        fynix: &mut Fynix,
        hit_test: &HitTest,
        pos: Point,
    ) {
        let target = hit_test.query(pos).map(|hit| hit.id);
        if target == self.hovered {
            return;
        }

        if let Some(left) = self.hovered {
            // The pointer has moved off `left`, so the hit-test no
            // longer covers it: always deliver.
            fynix.dispatch_bubbling::<PointerLeave>(
                &left,
                PointerLeave,
                |_| true,
            );
        }
        if let Some(entered) = target {
            fynix.dispatch_bubbling::<PointerEnter>(
                &entered,
                PointerEnter,
                |id| hit_test.contains(id, pos),
            );
        }

        self.hovered = target;
    }

    /// Discards any in-flight press for `pointer`, used when the
    /// backend reports the pointer was cancelled (e.g. a touch was
    /// interrupted) so no spurious click fires later.
    ///
    /// A cancelled pointer cannot move off an element on its own, so
    /// any hover it established is dropped here with a matching
    /// [`PointerLeave`]. Hover is tracked as a single element rather
    /// than per pointer, so the leave fires for whatever is currently
    /// hovered.
    pub fn cancel(&mut self, fynix: &mut Fynix, pointer: Id) {
        self.presses.retain(|press| press.pointer != pointer);

        if let Some(left) = self.hovered.take() {
            fynix.dispatch_bubbling::<PointerLeave>(
                &left,
                PointerLeave,
                |_| true,
            );
        }
    }

    /// Removes and returns the press matching `pointer` and `button`,
    /// if one is in flight.
    fn take_press(
        &mut self,
        pointer: Id,
        button: Btn,
    ) -> Option<Press<Id, Btn>> {
        let i = self.presses.iter().position(|press| {
            press.pointer == pointer && press.button == button
        })?;
        Some(self.presses.swap_remove(i))
    }
}

/// Returns `true` when the release is further than [`CLICK_SLOP`]
/// from where the press began.
fn moved_past_slop<Id, Btn>(
    press: &Press<Id, Btn>,
    pos: Point,
) -> bool {
    let offset = pos - press.pos;
    offset.length_squared() > CLICK_SLOP * CLICK_SLOP
}

#[cfg(test)]
mod tests {
    use fynix::element::layout::ElementNodes;
    use fynix::prelude::*;

    use super::*;
    use crate::is_hit_target;

    /// A test button whose role is whatever it is constructed with.
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Btn(ButtonRole);

    impl ClassifyButton for Btn {
        fn role(self) -> ButtonRole {
            self.0
        }
    }

    const PRIMARY: Btn = Btn(ButtonRole::Primary);
    const SECONDARY: Btn = Btn(ButtonRole::Secondary);

    type Rec = PointerRecognizer<u64, Btn>;

    /// Builds the hit-test index, indexing only elements with a
    /// pointer handler (the production `include` predicate).
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

    #[derive(Debug, PartialEq)]
    struct SecondaryClicked;

    /// Builds a single `Tile` with click handlers, positioned at
    /// `(10, 10)` sized `100 x 50` in absolute space.
    fn tile_at(fynix: &mut Fynix) -> ElementId {
        let mut world = ();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Tile>()
                .on::<Click>(|_, events| events.push(Clicked))
                .on::<SecondaryClick>(|_, events| {
                    events.push(SecondaryClicked)
                })
                .id()
        };

        let node = fynix.elements.table.node_mut(&id).unwrap();
        node.world_translation = Vec2::new(10.0, 10.0);
        node.size = Size::new(100.0, 50.0);
        id
    }

    fn click_count(fynix: &Fynix) -> usize {
        fynix.events.iter::<Clicked>().count()
    }

    #[test]
    fn press_and_release_inside_dispatches_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        rec.pointer_down(&hit, 0, PRIMARY, Point::new(20.0, 20.0));
        rec.pointer_up(
            &mut fynix,
            &hit,
            0,
            PRIMARY,
            Point::new(25.0, 25.0),
        );

        assert_eq!(click_count(&fynix), 1);
    }

    #[test]
    fn secondary_button_dispatches_secondary_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        rec.pointer_down(&hit, 0, SECONDARY, Point::new(20.0, 20.0));
        rec.pointer_up(
            &mut fynix,
            &hit,
            0,
            SECONDARY,
            Point::new(25.0, 25.0),
        );

        assert_eq!(click_count(&fynix), 0);
        assert_eq!(
            fynix.events.iter::<SecondaryClicked>().count(),
            1
        );
    }

    #[test]
    fn release_outside_element_does_not_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        rec.pointer_down(&hit, 0, PRIMARY, Point::new(20.0, 20.0));
        // (200, 200) misses the tile entirely.
        rec.pointer_up(
            &mut fynix,
            &hit,
            0,
            PRIMARY,
            Point::new(200.0, 200.0),
        );

        assert_eq!(click_count(&fynix), 0);
    }

    #[test]
    fn drag_past_slop_does_not_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        rec.pointer_down(&hit, 0, PRIMARY, Point::new(20.0, 20.0));
        // Still inside the tile, but 12px away (> CLICK_SLOP).
        rec.pointer_up(
            &mut fynix,
            &hit,
            0,
            PRIMARY,
            Point::new(32.0, 20.0),
        );

        assert_eq!(click_count(&fynix), 0);
    }

    #[test]
    fn press_outside_then_release_inside_does_not_click() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        // Press misses everything, so there is no target.
        rec.pointer_down(&hit, 0, PRIMARY, Point::new(200.0, 200.0));
        rec.pointer_up(
            &mut fynix,
            &hit,
            0,
            PRIMARY,
            Point::new(20.0, 20.0),
        );

        assert_eq!(click_count(&fynix), 0);
    }

    #[test]
    fn cancel_drops_pending_press() {
        let mut fynix = Fynix::new();
        let id = tile_at(&mut fynix);
        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        rec.pointer_down(&hit, 0, PRIMARY, Point::new(20.0, 20.0));
        rec.cancel(&mut fynix, 0);
        rec.pointer_up(
            &mut fynix,
            &hit,
            0,
            PRIMARY,
            Point::new(25.0, 25.0),
        );

        assert_eq!(click_count(&fynix), 0);
    }

    #[derive(Debug, PartialEq)]
    struct Entered;

    #[derive(Debug, PartialEq)]
    struct Left;

    #[test]
    fn cancel_emits_leave_for_hovered() {
        let mut fynix = Fynix::new();
        let mut world = ();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Tile>()
                .on::<PointerEnter>(|_, events| events.push(Entered))
                .on::<PointerLeave>(|_, events| events.push(Left))
                .id()
        };
        let node = fynix.elements.table.node_mut(&id).unwrap();
        node.world_translation = Vec2::new(10.0, 10.0);
        node.size = Size::new(100.0, 50.0);

        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        // Hover onto the tile, then cancel: the hover must be undone
        // with a leave even though the pointer never moved off.
        rec.pointer_moved(&mut fynix, &hit, Point::new(20.0, 20.0));
        assert_eq!(fynix.events.iter::<Entered>().count(), 1);
        assert_eq!(fynix.events.iter::<Left>().count(), 0);

        rec.cancel(&mut fynix, 0);
        assert_eq!(fynix.events.iter::<Left>().count(), 1);
    }

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
        let node = fynix.elements.table.node_mut(&id).unwrap();
        node.world_translation = Vec2::new(10.0, 10.0);
        node.size = Size::new(100.0, 50.0);

        let hit = hit_test(&fynix, &id);
        let mut rec = Rec::new();

        // Move onto the tile: one enter, no leave.
        rec.pointer_moved(&mut fynix, &hit, Point::new(20.0, 20.0));
        assert_eq!(fynix.events.iter::<Entered>().count(), 1);
        assert_eq!(fynix.events.iter::<Left>().count(), 0);

        // Move within the tile: no further enter/leave.
        rec.pointer_moved(&mut fynix, &hit, Point::new(30.0, 30.0));
        assert_eq!(fynix.events.iter::<Entered>().count(), 1);

        // Move off the tile: one leave.
        rec.pointer_moved(&mut fynix, &hit, Point::new(200.0, 200.0));
        assert_eq!(fynix.events.iter::<Left>().count(), 1);
    }
}

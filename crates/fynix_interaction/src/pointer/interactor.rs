use core::hash::Hash;

use fynix::Fynix;
use fynix::element::ElementId;
use fynix::element::storage::Elements;
use fynix_hit_test::HitTest;
use spatree::kurbo::Point;

use super::identity::{ClassifyButton, PointerId};
use super::recognizer::{Config, PointerRecognizer};
use crate::pointer::HitTarget;

/// Drives pointer interaction for a subtree: owns the hit-test and
/// the recognizer so the backend feeds input through `down`/`up`/
/// `moved`/`cancel` and never touches the spatial index directly.
///
/// Call [`Self::invalidate`] after each layout pass; the hit-test is
/// then rebuilt lazily on the next event.
pub struct Interactor<Device, Pointer, Btn> {
    recognizer: PointerRecognizer<Device, Pointer>,
    classify: ClassifyButton<Btn>,
    hit_test: Option<HitTest>,
    root: ElementId,
}

impl<Device, Pointer, Btn> Interactor<Device, Pointer, Btn>
where
    Device: Copy + Eq + Hash + 'static,
    Pointer: Copy + Eq + Hash + 'static,
{
    /// Creates an interactor over the subtree rooted at `root`.
    ///
    /// `classify` maps the backend's native button type to a
    /// [`ButtonRole`], used to route clicks into the role-specific
    /// interactions.
    ///
    /// [`ButtonRole`]: crate::pointer::identity::ButtonRole
    pub fn new(
        root: ElementId,
        config: Config,
        classify: ClassifyButton<Btn>,
    ) -> Self {
        Self {
            recognizer: PointerRecognizer::new(config),
            classify,
            hit_test: None,
            root,
        }
    }

    /// Marks the hit-test stale; it is rebuilt on the next event.
    /// Call this after every layout pass.
    pub fn invalidate(&mut self) {
        self.hit_test = None;
    }

    /// Feeds a button-down at `pos`.
    pub fn down<W: 'static>(
        &mut self,
        fynix: &mut Fynix<W>,
        id: PointerId<Device, Pointer>,
        button: Btn,
        pos: Point,
    ) {
        let role = (self.classify)(button);
        self.ensure_hit_test(fynix);
        let hit_test = self.hit_test.as_ref().unwrap();
        let hit = hit_test.query(pos);
        self.recognizer.down(id, role, pos, hit);
    }

    /// Feeds a button-up at `pos`.
    pub fn up<W: 'static>(
        &mut self,
        fynix: &mut Fynix<W>,
        world: &mut W,
        id: PointerId<Device, Pointer>,
        pos: Point,
    ) {
        self.ensure_hit_test(fynix);
        let hit_test = self.hit_test.as_ref().unwrap();
        let hit = hit_test.query(pos);
        self.recognizer.up(id, pos, hit, fynix, world, hit_test);
    }

    /// Feeds a pointer move to `pos`.
    pub fn moved<W: 'static>(
        &mut self,
        fynix: &mut Fynix<W>,
        world: &mut W,
        id: PointerId<Device, Pointer>,
        pos: Point,
    ) {
        self.ensure_hit_test(fynix);
        let hit_test = self.hit_test.as_ref().unwrap();
        let hit = hit_test.query(pos);
        self.recognizer.moved(id, pos, hit, fynix, world, hit_test);
    }

    /// Feeds a cancellation, aborting any press or drag.
    pub fn cancel<W: 'static>(
        &mut self,
        fynix: &mut Fynix<W>,
        world: &mut W,
        id: PointerId<Device, Pointer>,
    ) {
        self.recognizer.cancel(id, fynix, world);
    }

    /// Rebuilds the hit-test if it is stale, indexing only the
    /// elements carrying one of the crate's pointer handlers.
    fn ensure_hit_test<W: 'static>(&mut self, fynix: &Fynix<W>) {
        if self.hit_test.is_none() {
            self.hit_test = Some(HitTest::build(
                fynix.elements(),
                &self.root,
                |id| is_hit_target::<W>(fynix.elements(), id),
            ));
        }
    }
}

/// An element is a hit target once an observer has marked it with
/// [`HitTarget`], so the index check is a single component lookup.
fn is_hit_target<W: 'static>(
    elements: &Elements<W>,
    id: &ElementId,
) -> bool {
    elements.table.contains::<HitTarget>(id)
}

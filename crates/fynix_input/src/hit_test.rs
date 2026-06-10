use alloc::vec::Vec;

use fynix::element::ElementId;
use fynix::element::storage::Elements;
use spatree::Spatree;
use spatree::kurbo::{Point, Rect};

/// The element resolved under a pointer position, plus the position
/// expressed relative to that element's origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The topmost element whose absolute rect contains the point.
    pub id: ElementId,
    /// Pointer position relative to the hit element's origin
    /// (`world_translation`).
    pub local_x: f64,
    pub local_y: f64,
}

/// A spatial index over an element subtree's absolute rects, used to
/// resolve a pointer position to the element underneath it.
///
/// Built once from a laid-out tree via [`Self::build`], then queried
/// per pointer event. Rects are pushed in paint order (parents before
/// children), so a later push is an element painted on top;
/// [`Self::query`] resolves overlaps in favour of that topmost
/// element.
pub struct HitTest {
    tree: Spatree,
    /// One entry per pushed rect, indexed by its `RectId`: the
    /// element and its absolute rect (for deriving the local
    /// point and for per-element containment tests).
    entries: Vec<Entry>,
}

#[derive(Clone, Copy)]
struct Entry {
    id: ElementId,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Entry {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
}

impl HitTest {
    /// Builds a hit-test index over the subtree rooted at `root`,
    /// indexing only the elements `include` accepts.
    ///
    /// Most of a scene is not interactive, so only elements that can
    /// receive a hit-tested interaction (those with a registered
    /// handler, see [`crate::is_hit_target`]) need to be in the
    /// spatree. The whole subtree is still walked, to reach
    /// interactive descendants, but non-indexed elements add nothing
    /// to the tree. Paint order is preserved among indexed elements,
    /// so the topmost still wins.
    ///
    /// Layout must be complete so each element's `world_translation`
    /// and `size` are current.
    pub fn build(
        elements: &Elements,
        root: &ElementId,
        include: impl Fn(&ElementId) -> bool,
    ) -> Self {
        let mut tree = Spatree::new();
        let mut entries = Vec::new();

        elements.visit_paint_order(root, |id, meta| {
            if !include(id) {
                return;
            }

            let origin = meta.node.world_translation;
            let size = meta.node.size;

            let x0 = origin.x as f64;
            let y0 = origin.y as f64;
            let x1 = x0 + size.width as f64;
            let y1 = y0 + size.height as f64;
            tree.push_rect(Rect::new(x0, y0, x1, y1));
            entries.push(Entry {
                id: *id,
                x0,
                y0,
                x1,
                y1,
            });
        });

        tree.build(|rect| rect.center());

        Self { tree, entries }
    }

    /// Returns the topmost element whose absolute rect contains
    /// `(x, y)`, or `None` if the point misses every element.
    pub fn query(&self, x: f64, y: f64) -> Option<Hit> {
        let rect_id = self.tree.query_point_single(
            Point::new(x, y),
            // Later push order paints on top, so the larger id wins.
            #[inline(always)]
            |a, b| if a > b { a } else { b },
        )?;

        let entry = self.entries.get(rect_id.into_inner())?;
        Some(Hit {
            id: entry.id,
            local_x: x - entry.x0,
            local_y: y - entry.y0,
        })
    }

    /// Returns `true` if `id`'s absolute rect contains `(x, y)`.
    ///
    /// Used as the bubbling gate: an ancestor handles a pointer
    /// interaction only while the pointer is still within its bounds.
    pub fn contains(&self, id: &ElementId, x: f64, y: f64) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.id == *id && entry.contains(x, y))
    }
}

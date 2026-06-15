//! Spatial hit-testing for fynix.
//!
//! Resolves a pointer position to the element underneath it, using a
//! spatial index built once after layout and queried per event.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use fynix::element::ElementId;
use fynix::element::storage::Elements;
use hashbrown::HashMap;
use spatree::kurbo::{Point, Rect};
use spatree::{RectId, Spatree};

/// The element resolved under a pointer position, plus the position
/// expressed relative to that element's origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The topmost element whose absolute rect contains the point.
    pub id: ElementId,
    /// Pointer position relative to the hit element's origin.
    pub local: Point,
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
    /// Push-order mapping from [`RectId`] to [`ElementId`], used by
    /// [`Self::query`] to recover the element after the spatree
    /// resolves a point.
    ids: Vec<ElementId>,
    /// Reverse mapping from `ElementId` to `RectId`, used by
    /// [`Self::contains`] for O(1) bounds lookup.
    id_map: HashMap<ElementId, RectId>,
}

impl HitTest {
    /// Builds a hit-test index over the subtree rooted at `root`,
    /// indexing only the elements `include` accepts.
    ///
    /// Most of a scene is not interactive, so only elements that can
    /// receive a hit-tested interaction need to be in the spatree.
    /// The whole subtree is still walked to reach interactive
    /// descendants, but non-indexed elements add nothing to the tree.
    /// Paint order is preserved among indexed elements, so the
    /// topmost still wins.
    ///
    /// Layout must be complete so each element's `world_translation`
    /// and `size` are current.
    pub fn build(
        elements: &Elements,
        root: &ElementId,
        include: impl Fn(&ElementId) -> bool,
    ) -> Self {
        let mut tree = Spatree::new();
        let mut ids = Vec::new();
        let mut id_map = HashMap::new();

        elements.visit_paint_order(root, |id, node| {
            if !include(id) {
                return;
            }

            let origin = node.world_translation;
            let size = node.size;

            let x0 = origin.x as f64;
            let y0 = origin.y as f64;
            let x1 = x0 + size.width as f64;
            let y1 = y0 + size.height as f64;
            let rect_id = tree.push_rect(Rect::new(x0, y0, x1, y1));
            ids.push(*id);
            id_map.insert(*id, rect_id);
        });

        tree.build(|rect| rect.center());

        Self { tree, ids, id_map }
    }

    /// Returns the topmost element whose absolute rect contains
    /// `point`, or `None` if the point misses every element.
    pub fn query(&self, point: Point) -> Option<Hit> {
        let rect_id = self.tree.query_point_single(
            point,
            // Later push order paints on top, so the larger id wins.
            #[inline(always)]
            |a, b| if a > b { a } else { b },
        )?;

        let id = *self.ids.get(*rect_id)?;
        let rect = self.tree.get_rect(rect_id)?;
        Some(Hit {
            id,
            local: Point::new(point.x - rect.x0, point.y - rect.y0),
        })
    }

    /// Returns `true` if `id`'s absolute rect contains `point`.
    ///
    /// Used as the bubbling gate: an ancestor handles a pointer
    /// interaction only while the pointer is still within its bounds.
    pub fn contains(&self, id: &ElementId, point: Point) -> bool {
        self.id_map
            .get(id)
            .and_then(|id| self.tree.get_rect(*id))
            .is_some_and(|rect| rect.contains(point))
    }
}

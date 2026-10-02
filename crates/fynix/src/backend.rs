//! What views are built into.

use core::hash::Hash;

/// A world of nodes. Views above the elements are generic over it;
/// each element is written once per backend.
pub trait Backend: 'static {
    type World: 'static;
    type Node: Copy + Eq + Hash + Send + Sync + 'static;

    /// A new, empty node under `parent`, or at the root.
    fn spawn(
        world: &mut Self::World,
        parent: Option<Self::Node>,
    ) -> Self::Node;

    /// Removes `node` and every node under it.
    fn despawn(world: &mut Self::World, node: Self::Node);

    /// Sets the order of the children of `parent`, which `children`
    /// lists whole.
    fn reorder(
        world: &mut Self::World,
        parent: Self::Node,
        children: &[Self::Node],
    );

    /// A hook run when an element is mounted on `node`, for telling
    /// [`Mounted::unmount`](crate::Mounted::unmount) once it is gone.
    fn on_mount(_world: &mut Self::World, _node: Self::Node) {}

    /// Puts `node` in its leaving state: the root of a view a
    /// structural view has dropped, kept while it animates out. It
    /// should stop taking input.
    fn leave(_world: &mut Self::World, _node: Self::Node) {}

    /// Keeps `node`, the root of a view a structural view has just
    /// built, out of the layout and out of sight, with its natural
    /// size measurable once a layout has run, and stops it taking
    /// input. It is held until [`collapse`](Self::collapse) is first
    /// called on it, and let go by [`release`](Self::release).
    fn hold(_world: &mut Self::World, _node: Self::Node) {}

    /// Sets the space `node` takes in its parent's layout, `progress`
    /// of the way from its natural size to nothing. The first call
    /// measures the natural size, and puts a held node in the layout.
    /// A leaving view is called with 0 first and 1 last, then
    /// despawned. An entering one is called with 1 first and 0 last,
    /// then released.
    fn collapse(
        _world: &mut Self::World,
        _node: Self::Node,
        _progress: f32,
    ) {
    }

    /// Makes `node` whole again after [`hold`](Self::hold): in the
    /// layout as it was built, taking input, and no longer entering,
    /// so what it animates in from starts to move. Without a
    /// `collapse` before it, as under reduced motion, it was never
    /// in the layout.
    fn release(_world: &mut Self::World, _node: Self::Node) {}
}

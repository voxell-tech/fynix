//! What a view is, and the kinds there are: elements, composites built
//! out of other views, and wrappers around any view.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use motiongfx_interp::interpolation::InterpFn;

use crate::backend::Backend;
use crate::cx::Cx;
use crate::layer::Live;
use crate::transition::Tween;
use crate::visual::{Visual, VisualMut};

/// Something that can be built under a node, with the theme `T`.
pub trait View<B: Backend, T> {
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node;
}

/// A view set rules can restyle: every prop can be left unset, and a
/// value left unset can be filled from below.
pub trait Styled: Sized + Send + Sync + 'static {
    /// Every prop unset.
    fn unset() -> Self;

    /// This, with each prop it left unset taken from `below`.
    fn over(self, below: Self) -> Self;
}

/// A view whose props can be put on top of it for a while and taken
/// off again, without cloning any of them: what a state rule needs.
///
/// Props are numbered in declaration order, one bit each. The
/// [`styled!`](crate::styled) macro writes this and [`Styled`] from a
/// list of fields.
pub trait Layered: Styled {
    /// The bits of the props this sets.
    fn set_mask(&self) -> u64;

    /// Swaps the props whose bits are in `mask` with `other`'s.
    /// Swapping again with the same mask undoes it.
    fn swap_props(&mut self, other: &mut Self, mask: u64);
}

/// A field of a [`Styled`] view: a [`Prop`](crate::Prop), or anything
/// else a rule can leave unset.
pub trait Settable {
    fn empty() -> Self;

    fn is_set(&self) -> bool;

    /// This, or `below` when this was left unset.
    fn or(self, below: Self) -> Self;
}

impl<W, T> Settable for crate::Prop<W, T> {
    fn empty() -> Self {
        Self::Unset
    }

    fn is_set(&self) -> bool {
        !self.is_unset()
    }

    fn or(self, below: Self) -> Self {
        crate::Prop::or(self, below)
    }
}

impl<T> Settable for Option<T> {
    fn empty() -> Self {
        None
    }

    fn is_set(&self) -> bool {
        self.is_some()
    }

    fn or(self, below: Self) -> Self {
        Option::or(self, below)
    }
}

/// [`Styled`] and [`Layered`] for a struct whose fields are all
/// [`Settable`]: `styled!(Label { text, size, tone })`.
#[macro_export]
macro_rules! styled {
    ($view:ty { $($field:ident),* $(,)? }) => {
        impl $crate::Styled for $view {
            fn unset() -> Self {
                Self { $($field: $crate::Settable::empty()),* }
            }

            fn over(self, below: Self) -> Self {
                Self {
                    $($field: $crate::Settable::or(
                        self.$field,
                        below.$field,
                    )),*
                }
            }
        }

        impl $crate::Layered for $view {
            fn set_mask(&self) -> u64 {
                let mut mask = 0;
                let mut bit = 1;
                $(
                    if $crate::Settable::is_set(&self.$field) {
                        mask |= bit;
                    }
                    bit <<= 1;
                )*
                let _ = bit;
                mask
            }

            fn swap_props(&mut self, other: &mut Self, mask: u64) {
                let mut bit = 1;
                $(
                    if mask & bit != 0 {
                        ::core::mem::swap(
                            &mut self.$field,
                            &mut other.$field,
                        );
                    }
                    bit <<= 1;
                )*
                let _ = (bit, other);
            }
        }
    };
}

/// A view that is one node of its own, with no views under it.
///
/// Its props are resolved against the rules in force, read into a
/// [`Snapshot`](Self::Snapshot) of plain values, and written onto the
/// node. A live element stays mounted, and is read and written again
/// whenever it reports a change.
pub trait Element<B: Backend, T>: Layered {
    /// Every prop's value at one moment, with the theme's defaults
    /// filled in.
    type Snapshot: Clone + PartialEq + Send + Sync + 'static;

    /// What the node needs besides what [`write`](Self::write) keeps
    /// up to date.
    fn prepare(world: &mut B::World, node: B::Node);

    fn snapshot(&self, world: &B::World, theme: &T)
    -> Self::Snapshot;

    fn write(
        snapshot: &Self::Snapshot,
        world: &mut B::World,
        node: B::Node,
    );

    /// Whether anything it holds can change after the build.
    fn is_live(&self) -> bool;

    /// Whether anything it holds may have changed since the last
    /// call. Every prop's check runs each call, as each one keeps
    /// its own memory of the last.
    fn changed(&mut self, world: &B::World) -> bool;

    /// A hook run right after the element is mounted on `node`.
    fn on_mounted(&self, _world: &mut B::World, _node: B::Node) {}

    /// This, with the rules in force applied.
    fn resolve(self, cx: &Cx<'_, B, T>) -> Self
    where
        T: 'static,
    {
        cx.resolve(self)
    }

    /// Edits a fresh snapshot of `node` before it is written, from
    /// what only the node knows.
    fn adjust(
        &self,
        _snapshot: &mut Self::Snapshot,
        _world: &B::World,
        _node: B::Node,
        _theme: &T,
    ) {
    }

    /// How the written values travel to a new snapshot, whatever the
    /// rules say. `None` leaves it to a transition rule.
    fn tween(&self, _theme: &T) -> Option<Tween<Self::Snapshot>> {
        None
    }

    /// How two snapshots blend, for a transition rule to travel with.
    /// `None` snaps whatever the rules say.
    fn interp() -> Option<InterpFn<Self::Snapshot>> {
        None
    }

    /// Its [`Visual`] props, for rules for every kind of element to
    /// reach. `None` for an element without them.
    fn visual(&mut self) -> Option<VisualMut<'_, B::World>> {
        None
    }
}

impl<B: Backend, T: 'static, E: Element<B, T>> View<B, T> for E {
    fn build(mut self, cx: &mut Cx<'_, B, T>) -> B::Node {
        let call = self.set_mask();
        let layers = cx.layers::<E>();
        let (visual_call, visual_layers) = match self.visual() {
            Some(visual) => {
                (visual.set_mask(), cx.layers::<Visual<B::World>>())
            }
            None => (0, Vec::new()),
        };
        let curve = cx.curve();
        let mut element = E::resolve(self, cx);
        if let Some(mut visual) = element.visual() {
            // After the element's own rules, which are more specific.
            visual.fill(cx.resolve(Visual::unset()));
        }
        let node = cx.spawn();
        let mut live = Live {
            layers,
            call,
            visual_layers,
            visual_call,
            tween: element.tween(cx.theme()).or_else(|| {
                Some(Tween {
                    curve: curve?,
                    interp: E::interp()?,
                })
            }),
        };
        E::prepare(cx.world, node);
        // Before the snapshot, which a state set on watching can
        // change.
        live.watch(cx.world, node);
        let snapshot =
            live.snapshot(&mut element, cx.world, node, cx.theme());
        E::write(&snapshot, cx.world, node);
        // One with a transition is kept too, so it can animate out.
        if element.is_live()
            || live.is_layered()
            || live.tween.is_some()
        {
            cx.mount(node, element, snapshot, live);
        }
        node
    }
}

/// Views built one after another under the same node: a tuple of
/// views, or a `Vec` of one kind.
pub trait ViewSeq<B: Backend, T> {
    fn build_each(self, cx: &mut Cx<'_, B, T>) -> Vec<B::Node>;
}

impl<B: Backend, T, V: View<B, T>> ViewSeq<B, T> for Vec<V> {
    fn build_each(self, cx: &mut Cx<'_, B, T>) -> Vec<B::Node> {
        self.into_iter().map(|view| view.build(cx)).collect()
    }
}

macro_rules! view_seq {
    ($($view:ident),*) => {
        impl<B: Backend, T, $($view: View<B, T>),*> ViewSeq<B, T>
            for ($($view,)*)
        {
            #[allow(non_snake_case, unused_variables)]
            fn build_each(self, cx: &mut Cx<'_, B, T>) -> Vec<B::Node> {
                let ($($view,)*) = self;
                vec![$($view.build(cx)),*]
            }
        }
    };
}

view_seq!();
view_seq!(V1);
view_seq!(V1, V2);
view_seq!(V1, V2, V3);
view_seq!(V1, V2, V3, V4);
view_seq!(V1, V2, V3, V4, V5);
view_seq!(V1, V2, V3, V4, V5, V6);
view_seq!(V1, V2, V3, V4, V5, V6, V7);
view_seq!(V1, V2, V3, V4, V5, V6, V7, V8);
view_seq!(V1, V2, V3, V4, V5, V6, V7, V8, V9);
view_seq!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10);
view_seq!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11);
view_seq!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12);

/// A view's build, its type erased.
type BuildFn<B, T> =
    dyn for<'a> FnOnce(&mut Cx<'a, B, T>) -> <B as Backend>::Node;

/// Any view, its type erased: for storing views of different kinds
/// together, or keeping a deep view's type from growing.
pub struct AnyView<B: Backend, T>(Box<BuildFn<B, T>>);

impl<B: Backend, T> AnyView<B, T> {
    /// A view built by `build`, for one written in place.
    pub fn new(
        build: impl for<'a> FnOnce(&mut Cx<'a, B, T>) -> B::Node + 'static,
    ) -> Self {
        Self(Box::new(build))
    }
}

impl<B: Backend, T> View<B, T> for AnyView<B, T> {
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        (self.0)(cx)
    }
}

/// What any view can be turned into.
pub trait ViewExt<B: Backend, T>:
    View<B, T> + Sized + 'static
{
    /// This view, its type erased.
    fn boxed(self) -> AnyView<B, T> {
        AnyView::new(move |cx| self.build(cx))
    }
}

impl<B: Backend, T, V: View<B, T> + 'static> ViewExt<B, T> for V {}

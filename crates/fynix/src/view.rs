//! What a view is, and the kinds there are: elements, composites
//! built out of other views, and wrappers around any view.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::any::TypeId;
use core::ops::Deref;

use typarena::type_pool::PoolKey;

use crate::backend::Backend;
use crate::cx::Cx;
use crate::layer::Live;
use crate::mounted::Tick;
use crate::transition::Travel;
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
/// Its props are resolved against the rules in force, and each is
/// written onto the node on its own, through its
/// [`Patch`](crate::Patch). A live element stays mounted, and only
/// the props that changed are read and written again.
///
/// Props are numbered in declaration order, one bit each, as
/// [`Layered`] numbers them. `#[element]` writes this from the
/// struct.
pub trait Element<B: Backend, T>: Layered {
    /// Every prop's [`Slot`](crate::Slot): what is written, and where
    /// it is heading.
    type Shown: Default + Send + Sync + 'static;

    /// What the node needs before any prop is written.
    fn prepare(world: &mut B::World, node: B::Node);

    /// Re-reads the props whose bits are in `dirty`, heads each for
    /// its new value, as `travel` says if it blends, and moves every
    /// prop still travelling on by `tick`. Returns whether any still
    /// is.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &self,
        shown: &mut Self::Shown,
        dirty: u64,
        world: &mut B::World,
        node: B::Node,
        theme: &T,
        tick: Tick,
        travel: Option<Travel>,
    ) -> bool;

    /// Whether anything it holds can change after the build.
    fn is_live(&self) -> bool;

    /// The bits of the props that may have changed since the last
    /// call. Every prop's check runs each call, as each one keeps
    /// its own memory of the last.
    fn changed(&mut self, world: &B::World) -> u64;

    /// A hook run right after the element is mounted on `node`.
    fn on_mounted(&self, _world: &mut B::World, _node: B::Node) {}

    /// This, with the rules in force applied.
    fn resolve(self, cx: &Cx<'_, B, T>) -> Self
    where
        T: 'static,
    {
        cx.resolve(self)
    }

    /// Its [`Visual`] props, for rules for every kind of element to
    /// reach. `None` for an element without them.
    fn visual(&mut self) -> Option<VisualMut<'_, B::World>> {
        None
    }

    /// The bits of its [`Visual`] props, opacity then scale.
    fn visual_bits() -> [u64; 2] {
        [0, 0]
    }
}

impl<B: Backend, T: 'static, E: Element<B, T>> View<B, T> for E {
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        cx.build_element(self)
    }
}

/// An element a show rule matched, as the rule is handed it: with the
/// set rules in force applied.
pub struct Matched<E> {
    element: E,
    /// The props its call site set.
    call: u64,
    /// The [`Visual`] props its call site set.
    visual_call: u64,
    /// The show rules it went through, which it skips when built.
    pub(crate) done: Vec<PoolKey>,
}

impl<E> Matched<E> {
    /// `element` with the set rules in force applied.
    pub(crate) fn resolve<B: Backend, T: 'static>(
        mut element: E,
        cx: &Cx<'_, B, T>,
    ) -> Self
    where
        E: Element<B, T>,
    {
        let call = element.set_mask();
        let visual_call =
            element.visual().map_or(0, |visual| visual.set_mask());
        let mut element = E::resolve(element, cx);
        if let Some(mut visual) = element.visual() {
            // After the element's own rules, which are more specific.
            visual.fill(cx.resolve(Visual::unset()));
        }
        Self {
            element,
            call,
            visual_call,
            done: Vec::new(),
        }
    }

    /// The bits of the props its call site set, numbered as
    /// [`Layered`] numbers them. The rest are unset or set by rules.
    pub fn call_mask(&self) -> u64 {
        self.call
    }
}

impl<E> Deref for Matched<E> {
    type Target = E;

    fn deref(&self) -> &E {
        &self.element
    }
}

impl<B: Backend, T: 'static, E: Element<B, T>> View<B, T>
    for Matched<E>
{
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        match cx.apply_show(self) {
            Ok(node) => node,
            Err(matched) => matched.mount(cx),
        }
    }
}

impl<E> Matched<E> {
    /// Builds the element on a node of its own.
    fn mount<B: Backend, T: 'static>(
        self,
        cx: &mut Cx<'_, B, T>,
    ) -> B::Node
    where
        E: Element<B, T>,
    {
        let Self {
            mut element,
            call,
            visual_call,
            ..
        } = self;
        let layers = cx.layers::<E>();
        let visual_layers = match element.visual() {
            Some(_) => cx.layers::<Visual<B::World>>(),
            None => Vec::new(),
        };
        let travel = cx.travel();
        let node = cx.spawn_for(Some(TypeId::of::<E>()));
        let mut live = Live {
            layers,
            call,
            visual_layers,
            visual_call,
            travel,
        };
        E::prepare(cx.world, node);
        // Before the first write, which a state set on watching can
        // change.
        live.watch(cx.world, node);
        let mut shown = E::Shown::default();
        live.update(
            &mut element,
            &mut shown,
            u64::MAX,
            cx.world,
            node,
            cx.theme(),
            Tick::default(),
        );
        // One with a transition is kept too, so it can animate out.
        if element.is_live()
            || live.is_layered()
            || live.travel.is_some()
        {
            cx.mount(node, element, shown, live);
        }
        node
    }
}

/// Views built one after another under the same node: a tuple of
/// views (up to 16), or a `Vec` of one kind.
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
view_seq!(V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13);
view_seq!(
    V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14
);
view_seq!(
    V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15
);
view_seq!(
    V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15,
    V16
);

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

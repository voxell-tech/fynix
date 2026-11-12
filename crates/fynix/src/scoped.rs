//! Views built under rules of their own: a block of rules, the same
//! block waiting on a state, and a transition.

use core::marker::PhantomData;

use crate::backend::Backend;
use crate::cx::Cx;
use crate::rules::Condition;
use crate::transition::Curve;
use crate::view::View;

/// A view built in a scope where `rules` ran first.
pub struct Rules<V, F> {
    view: V,
    rules: F,
}

/// A view whose `rules` hold while its root is in the state `C`.
pub struct When<V, F, C> {
    view: V,
    rules: F,
    state: PhantomData<fn() -> C>,
}

/// A view whose elements travel to new values over a curve `pick`
/// takes from the theme.
pub struct Transition<V, P> {
    view: V,
    pick: P,
}

impl<B, T, V, F> View<B, T> for Rules<V, F>
where
    B: Backend,
    T: 'static,
    V: View<B, T>,
    F: FnOnce(&mut Cx<'_, B, T>),
{
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        cx.scope(|cx| {
            (self.rules)(cx);
            self.view.build(cx)
        })
    }
}

impl<B, T, V, F, C> View<B, T> for When<V, F, C>
where
    B: Backend,
    T: 'static,
    V: View<B, T>,
    F: FnOnce(&mut Cx<'_, B, T>),
    C: Condition<B>,
{
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        cx.scope(|cx| {
            cx.when::<C>(self.rules);
            self.view.build(cx)
        })
    }
}

impl<B, T, V, P> View<B, T> for Transition<V, P>
where
    B: Backend,
    T: 'static,
    V: View<B, T>,
    P: FnOnce(&T) -> Curve,
{
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        cx.scope(|cx| {
            cx.transition(self.pick);
            self.view.build(cx)
        })
    }
}

/// What any view can be built under.
pub trait ScopedExt: Sized {
    /// This view, built in a scope where `rules` ran first. Rules
    /// marked [`root`](Cx::root) reach its root node alone.
    fn rules<F>(self, rules: F) -> Rules<Self, F> {
        Rules { view: self, rules }
    }

    /// This view, with `rules` holding while its root node is in the
    /// state `C`. See [`Cx::when`].
    fn when_in<C, F>(self, rules: F) -> When<Self, F, C> {
        When {
            view: self,
            rules,
            state: PhantomData,
        }
    }

    /// This view, with every element in it travelling to new values
    /// over the curve `pick` takes from the theme, such as
    /// `T::interact`.
    fn transition<P>(self, pick: P) -> Transition<Self, P> {
        Transition { view: self, pick }
    }
}

impl<V> ScopedExt for V {}

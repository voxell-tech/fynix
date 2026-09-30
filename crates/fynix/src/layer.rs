//! State rules as they reach one element: props put on top of it while
//! a state holds, and taken off when it stops.

use alloc::vec::Vec;

use crate::backend::Backend;
use crate::rules::When;
use crate::transition::{Curve, Tween};
use crate::view::{Element, Layered};
use crate::visual::Visual;

/// The props a state rule sets on an element, and the state they wait
/// on.
pub(crate) struct Layer<B: Backend, E> {
    /// Built by the rule from an unset view, so only the props it
    /// touches are set.
    pub view: E,
    pub when: When<B>,
    /// The node the state is read on. `None` is the element's own.
    pub on: Option<B::Node>,
    /// Whether the rule was written on the element itself, so it beats
    /// the call site. One written on an ancestor only fills what the
    /// call site left unset.
    pub own: bool,
}

impl<B: Backend, E> Layer<B, E> {
    /// The node the state is read on, for an element on `node`.
    pub fn on(&self, node: B::Node) -> B::Node {
        self.on.unwrap_or(node)
    }
}

impl<B: Backend, E: Layered> Layer<B, E> {
    /// The bits of the props to swap in for an element on `node` whose
    /// call site set `call`, or `None` while the state does not hold.
    fn mask(
        &self,
        world: &B::World,
        node: B::Node,
        call: u64,
    ) -> Option<u64> {
        let holds = (self.when.holds)(world, self.on(node));
        let allowed = if self.own { u64::MAX } else { !call };
        holds.then(|| self.view.set_mask() & allowed)
    }
}

/// What a mounted element keeps beside its props.
pub(crate) struct Live<B: Backend, E, S> {
    /// Weakest first.
    pub layers: Vec<Layer<B, E>>,
    /// The props the call site set.
    pub call: u64,
    /// Layers from rules for every kind of element, weakest first.
    pub visual_layers: Vec<Layer<B, Visual<B::World>>>,
    /// The [`Visual`] props the call site set.
    pub visual_call: u64,
    pub tween: Option<Tween<S>>,
}

impl<B: Backend, E, S> Live<B, E, S> {
    /// Whether any state rule reaches the element.
    pub fn is_layered(&self) -> bool {
        !self.layers.is_empty() || !self.visual_layers.is_empty()
    }

    /// Asks the backend to report every node a state rule is read on.
    pub fn watch(&self, world: &mut B::World, node: B::Node) {
        let whens =
            self.layers.iter().map(|layer| (layer.when, layer.on));
        let visual = self
            .visual_layers
            .iter()
            .map(|layer| (layer.when, layer.on));
        for (when, on) in whens.chain(visual) {
            (when.watch)(world, on.unwrap_or(node));
        }
    }

    /// The nodes other than the element's own that its state rules
    /// are read on.
    pub fn read_on(
        &self,
        node: B::Node,
    ) -> impl Iterator<Item = B::Node> {
        let ons = self.layers.iter().map(|layer| layer.on);
        let visual = self.visual_layers.iter().map(|layer| layer.on);
        ons.chain(visual).flatten().filter(move |&on| on != node)
    }

    /// The snapshot of `element` with every layer whose state holds
    /// put on top, each layer taken off again after. Layers for every
    /// kind of element go first, so the element's own kind wins.
    pub fn snapshot<T>(
        &mut self,
        element: &mut E,
        world: &B::World,
        node: B::Node,
        theme: &T,
    ) -> S
    where
        E: Element<B, T, Snapshot = S>,
    {
        let mut visual_swapped = Vec::new();
        for (index, layer) in
            self.visual_layers.iter_mut().enumerate()
        {
            let Some(mask) =
                layer.mask(world, node, self.visual_call)
            else {
                continue;
            };
            if let Some(mut visual) = element.visual() {
                visual.swap_props(&mut layer.view, mask);
                visual_swapped.push((index, mask));
            }
        }
        let mut swapped = Vec::new();
        for (index, layer) in self.layers.iter_mut().enumerate() {
            let Some(mask) = layer.mask(world, node, self.call)
            else {
                continue;
            };
            element.swap_props(&mut layer.view, mask);
            swapped.push((index, mask));
        }

        let mut snapshot = element.snapshot(world, theme);
        element.adjust(&mut snapshot, world, node, theme);

        for (index, mask) in swapped.into_iter().rev() {
            element.swap_props(&mut self.layers[index].view, mask);
        }
        for (index, mask) in visual_swapped.into_iter().rev() {
            if let Some(mut visual) = element.visual() {
                visual.swap_props(
                    &mut self.visual_layers[index].view,
                    mask,
                );
            }
        }
        snapshot
    }

    /// Whether a bound prop in any layer may have changed. Every
    /// check runs, as each keeps its own memory.
    pub fn changed<T>(&mut self, world: &B::World) -> bool
    where
        E: Element<B, T, Snapshot = S>,
    {
        let own =
            self.layers.iter_mut().fold(false, |changed, layer| {
                layer.view.changed(world) | changed
            });
        self.visual_layers.iter_mut().fold(own, |changed, layer| {
            layer.view.changed(world) | changed
        })
    }

    /// The curve the element travels over, if it does.
    pub fn curve(&self) -> Option<Curve> {
        self.tween.map(|tween| tween.curve)
    }
}

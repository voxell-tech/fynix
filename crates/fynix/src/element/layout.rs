use imaging::record::Scene;
use rectree::{Constraint, RectNode, RectNodes, Rectree, Size};

use crate::element::ElementId;
use crate::element::meta::{ElementMetas, ElementTypeMetas};
use crate::element::table::ElementTable;
use crate::resource::Resources;

/// Immutable view of the element tree used to implement
/// [`Rectree`].
pub struct ElementTree<'a> {
    pub(super) elements: &'a ElementTable,
    pub(super) type_metas: &'a ElementTypeMetas,
}

impl<'a> Rectree for ElementTree<'a> {
    type Id = ElementId;
    type Nodes = ElementNodes<'a>;

    fn for_each_child(
        &self,
        id: &ElementId,
        nodes: &mut Self::Nodes,
        mut f: impl FnMut(&ElementId, &mut Self::Nodes),
    ) {
        if let Some(type_meta) = nodes
            .metas
            .get(id)
            .and_then(|m| self.type_metas.get_slot(m.slot))
        {
            (type_meta.children_fn)(
                self.elements,
                id,
                &mut |child| f(child, nodes),
            );
        }
    }

    fn constrain(
        &self,
        id: &ElementId,
        nodes: &Self::Nodes,
        parent: Constraint,
    ) -> Constraint {
        nodes
            .metas
            .get(id)
            .and_then(|m| self.type_metas.get_slot(m.slot))
            .map(|m| {
                m.get_dyn(self.elements, id)
                    .map(|e| e.constrain(parent))
                    .unwrap_or(parent)
            })
            .unwrap_or(parent)
    }

    fn build(
        &self,
        id: &ElementId,
        constraint: Constraint,
        nodes: &mut Self::Nodes,
    ) -> Size {
        nodes
            .metas
            .get(id)
            .and_then(|m| self.type_metas.get_slot(m.slot))
            .map(|m| {
                m.get_dyn(self.elements, id)
                    .map(|e| e.build(id, constraint, nodes))
                    .unwrap_or_default()
            })
            .unwrap_or(Size::ZERO)
    }
}

pub struct ElementNodes<'a> {
    pub(crate) metas: &'a mut ElementMetas,
    pub(crate) resources: &'a mut Resources,
}

impl ElementNodes<'_> {
    pub fn get_resource<R: 'static>(&self) -> Option<&R> {
        self.resources.get()
    }

    pub fn get_resource_mut<R: 'static>(&mut self) -> Option<&mut R> {
        self.resources.get_mut()
    }

    pub fn cache_scene(
        &mut self,
        id: &ElementId,
        scene: Scene,
    ) -> bool {
        if let Some(meta) = self.metas.get_mut(id) {
            meta.cached_scene = Some(scene);
            return true;
        }

        false
    }
}

// TODO: Hide this implementation to the `build` fn. Maybe
// add a `ElementNodesBuilder` wrapper struct.
impl RectNodes for ElementNodes<'_> {
    type Id = ElementId;

    fn get_node(
        &self,
        id: &ElementId,
    ) -> Option<&RectNode<ElementId>> {
        self.metas.get(id).map(|m| &m.node)
    }

    fn get_node_mut(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut RectNode<ElementId>> {
        self.metas.get_mut(id).map(|m| &mut m.node)
    }
}

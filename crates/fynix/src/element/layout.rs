use imaging::record::Scene;
use rectree::{Constraint, RectNode, RectNodes, Rectree, Size};
use typarena::type_pool::TypePool;

use crate::element::ElementId;
use crate::element::table::ElementTable;
use crate::element::type_meta::ElementTypeMetas;
use crate::resource::Resources;

/// Immutable view of the element tree used to implement [`Rectree`].
pub struct ElementTree<'a> {
    pub(super) elements: &'a TypePool,
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
        if let Some(type_meta) =
            self.type_metas.get_column(id.col_id())
        {
            (type_meta.for_each_child_fn)(
                self.elements,
                id,
                &mut |child| f(child, nodes),
            );
        }
    }

    fn constrain(
        &self,
        id: &ElementId,
        _nodes: &Self::Nodes,
        parent: Constraint,
    ) -> Constraint {
        self.type_metas
            .get_column(id.col_id())
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
        self.type_metas
            .get_column(id.col_id())
            .map(|m| {
                m.get_dyn(self.elements, id)
                    .map(|e| e.build(id, constraint, nodes))
                    .unwrap_or_default()
            })
            .unwrap_or(Size::ZERO)
    }
}

pub struct ElementNodes<'a> {
    pub(crate) table: &'a mut ElementTable,
    pub(crate) resources: &'a mut Resources,
}

impl ElementNodes<'_> {
    pub fn get_resource<R: 'static>(&self) -> Option<&R> {
        self.resources.get()
    }

    pub fn get_resource_mut<R: 'static>(&mut self) -> Option<&mut R> {
        self.resources.get_mut()
    }

    pub fn cache_scene(&mut self, id: &ElementId, scene: Scene) {
        self.table.set_scene(id, scene);
    }
}

impl RectNodes for ElementNodes<'_> {
    type Id = ElementId;

    fn get_node(&self, id: &ElementId) -> Option<&ElementNode> {
        self.table.node(id)
    }

    fn get_node_mut(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut ElementNode> {
        self.table.node_mut(id)
    }
}

pub(crate) type ElementNode = RectNode<ElementId>;

#![no_std]

extern crate alloc;

use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};

use crate::registry::FieldRegistries;
use crate::style::StyleChain;

pub mod any_wrapper;
pub mod element;
pub mod field_index;
pub mod registry;
pub mod setter;
pub mod style;
pub mod type_table;

pub struct Fynix {
    pub tree: Rectree,
    pub registries: FieldRegistries,
    pub rules: StyleChain,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            tree: Rectree::new(),
            registries: FieldRegistries::default(),
            rules: StyleChain::default(),
        }
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}

impl Fynix {
    // pub fn set_rule(&mut self, id: RuleId) -> RuleSetBuilder<'_> {
    //     self.rules.edit(id, &mut self.registries)
    // }

    // pub fn apply<S: 'static>(&self, id: &RuleId, target: &mut S) {
    //     self.rules.apply_styles(id, target, &self.registries);
    // }

    // /// Apply rules from this scope node and all its ancestors with leaf-first
    // /// semantics: the closest scope wins, and each field is written exactly once.
    // pub(crate) fn apply_inherited<S: 'static>(
    //     &self,
    //     id: RuleId,
    //     target: &mut S,
    // ) {
    //     self.apply_inherited_inner(id, target, &mut HashSet::new());
    // }

    // fn apply_inherited_inner<S: 'static>(
    //     &self,
    //     id: RuleId,
    //     target: &mut S,
    //     visited: &mut HashSet<UntypedField>,
    // ) {
    //     self.rules.apply_styles_skipping(
    //         &id,
    //         target,
    //         &self.registries,
    //         visited,
    //     );
    //     if let Some(parent) = self.tree.get(&id.node_id).parent() {
    //         self.apply_inherited_inner(
    //             RuleId {
    //                 node_id: parent,
    //                 rank: id.rank,
    //             },
    //             target,
    //             visited,
    //         );
    //     }
    // }
}

pub struct FynixCtx<'a, W> {
    pub fynix: &'a mut Fynix,
    pub world: &'a mut W,
}

impl LayoutWorld for Fynix {
    fn get_solver(&self, _id: &NodeId) -> &dyn LayoutSolver {
        todo!()
    }
}

#![no_std]

extern crate alloc;

use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};

use crate::registry::FieldRegistries;
use crate::rule_set::RuleSet;

pub mod any_wrapper;
pub mod element;
pub mod field_index;
pub mod registry;
pub mod rule_set;
pub mod setter;
pub mod type_table;

pub struct Fynix {
    pub tree: Rectree,
    pub registries: FieldRegistries<NodeId>,
    pub rules: RuleSet<NodeId>,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            tree: Rectree::new(),
            registries: FieldRegistries::default(),
            rules: RuleSet::default(),
        }
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutWorld for Fynix {
    fn get_solver(&self, _id: &NodeId) -> &dyn LayoutSolver {
        todo!()
    }
}

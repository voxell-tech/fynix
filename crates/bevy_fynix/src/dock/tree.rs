//! The dock layout as plain data: a binary tree of splits and tabbed
//! leaves.

use bevy::ecs::resource::Resource;
use bevy::platform::collections::HashMap;

/// Whether a leaf draws a tab bar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DockAreaStyle {
    #[default]
    TabBar,
    /// No tab bar: the window provides its own header.
    Headless,
}

/// A handle to a node of a [`DockTree`]. Ids are never reused.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub u64);

/// A handle to one tab of a [`DockTree`]. Two tabs of the same window
/// kind have different ids. Ids are never reused.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct TabId(pub u64);

impl TabId {
    /// The id of a tab not yet inserted into a tree, which the tree
    /// replaces with a fresh one.
    pub(crate) const PENDING: TabId = TabId(0);
}

/// Which way a split divides its two children.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SplitAxis {
    /// `a` is on the left, `b` is on the right.
    Horizontal,
    /// `a` is on the top, `b` is on the bottom.
    Vertical,
}

/// A side of a leaf that a window is dropped on.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub fn axis(self) -> SplitAxis {
        match self {
            Edge::Top | Edge::Bottom => SplitAxis::Vertical,
            Edge::Left | Edge::Right => SplitAxis::Horizontal,
        }
    }

    /// Whether a window dropped on this edge goes into the first
    /// child of the split.
    pub fn puts_new_in_a(self) -> bool {
        matches!(self, Edge::Top | Edge::Left)
    }
}

/// One tab of a [`DockLeaf`]: a window kind and its own id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockTabEntry {
    pub window_id: String,
    pub id: TabId,
}

/// An area hosting tabbed windows.
#[derive(Clone, Debug)]
pub struct DockLeaf {
    pub area_id: String,
    pub style: DockAreaStyle,
    /// Tabs in display order.
    pub windows: Vec<DockTabEntry>,
    /// The tab shown. `None` for an empty leaf.
    pub active: Option<TabId>,
    /// Whether [`DockTree::simplify`] keeps the leaf when it has no
    /// tabs.
    pub persistent: bool,
}

impl DockLeaf {
    pub fn new(
        area_id: impl Into<String>,
        style: DockAreaStyle,
    ) -> Self {
        Self {
            area_id: area_id.into(),
            style,
            windows: Vec::new(),
            active: None,
            persistent: false,
        }
    }

    /// This leaf with one tab per window id. The tabs get their ids
    /// when the leaf is inserted into a [`DockTree`].
    pub fn with_windows(mut self, windows: Vec<String>) -> Self {
        self.windows = windows
            .into_iter()
            .map(|window_id| DockTabEntry {
                window_id,
                id: TabId::PENDING,
            })
            .collect();
        self.active = self.windows.first().map(|t| t.id);
        self
    }

    pub fn persistent(mut self) -> Self {
        self.persistent = true;
        self
    }

    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    /// The `(window_id, tab_id)` of each tab in display order.
    pub fn tabs(&self) -> impl Iterator<Item = (&str, TabId)> {
        self.windows.iter().map(|t| (t.window_id.as_str(), t.id))
    }

    pub fn tab_index(&self, id: TabId) -> Option<usize> {
        self.windows.iter().position(|t| t.id == id)
    }

    pub fn has_window(&self, window_id: &str) -> bool {
        self.windows.iter().any(|t| t.window_id == window_id)
    }
}

/// An internal node dividing its rect between two children.
#[derive(Clone, Debug)]
pub struct DockSplit {
    pub axis: SplitAxis,
    /// The share of the split given to `a`, kept in `(0.0, 1.0)` by
    /// [`DockTree::set_fraction`].
    pub fraction: f32,
    pub a: NodeId,
    pub b: NodeId,
}

#[derive(Clone, Debug)]
pub enum DockNode {
    Leaf(DockLeaf),
    Split(DockSplit),
}

impl DockNode {
    pub fn as_leaf(&self) -> Option<&DockLeaf> {
        match self {
            DockNode::Leaf(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_leaf_mut(&mut self) -> Option<&mut DockLeaf> {
        match self {
            DockNode::Leaf(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_split(&self) -> Option<&DockSplit> {
        match self {
            DockNode::Split(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_split_mut(&mut self) -> Option<&mut DockSplit> {
        match self {
            DockNode::Split(s) => Some(s),
            _ => None,
        }
    }
}

/// What the layout is built from: the splits and leaves of a
/// [`DockTree`], without fractions, tabs or the active tab. It only
/// differs between two trees when a rebuild is due.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Leaf(NodeId, DockAreaStyle),
    Split(NodeId, SplitAxis, Box<Shape>, Box<Shape>),
}

/// The dock layout. Edits go through the tree, and the dock views
/// follow it.
#[derive(Resource, Clone, Debug, Default)]
pub struct DockTree {
    pub nodes: HashMap<NodeId, DockNode>,
    /// The top of the layout. `None` while there is none.
    pub root: Option<NodeId>,
    next_id: u64,
    /// Starts at 1 so [`TabId::PENDING`] is never a live id.
    next_tab_id: u64,
}

impl DockTree {
    pub fn new() -> Self {
        Self::default()
    }

    fn fresh_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn fresh_tab_id(&mut self) -> TabId {
        self.next_tab_id = self.next_tab_id.saturating_add(1).max(1);
        TabId(self.next_tab_id)
    }

    pub fn insert(&mut self, mut node: DockNode) -> NodeId {
        if let DockNode::Leaf(ref mut leaf) = node {
            self.assign_pending_tab_ids(leaf);
        }
        let id = self.fresh_id();
        self.nodes.insert(id, node);
        id
    }

    fn assign_pending_tab_ids(&mut self, leaf: &mut DockLeaf) {
        let active_was_pending = leaf.active == Some(TabId::PENDING);
        let mut first_real = None;
        for tab in leaf.windows.iter_mut() {
            if tab.id == TabId::PENDING {
                tab.id = self.fresh_tab_id();
            }
            if first_real.is_none() {
                first_real = Some(tab.id);
            }
        }
        if active_was_pending {
            leaf.active = first_real;
        }
    }

    /// Appends a tab of `window_id` to `leaf` and makes it active.
    /// `None` if `leaf` is not a leaf.
    pub fn add_tab(
        &mut self,
        leaf: NodeId,
        window_id: impl Into<String>,
    ) -> Option<TabId> {
        let window_id = window_id.into();
        if !matches!(self.nodes.get(&leaf), Some(DockNode::Leaf(_))) {
            return None;
        }
        let id = self.fresh_tab_id();
        let DockNode::Leaf(l) = self.nodes.get_mut(&leaf)? else {
            return None;
        };
        l.windows.push(DockTabEntry { window_id, id });
        l.active = Some(id);
        Some(id)
    }

    pub fn get(&self, id: NodeId) -> Option<&DockNode> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut DockNode> {
        self.nodes.get_mut(&id)
    }

    pub fn leaf(&self, id: NodeId) -> Option<&DockLeaf> {
        self.nodes.get(&id)?.as_leaf()
    }

    /// The active tab of `leaf`.
    pub fn active(&self, leaf: NodeId) -> Option<TabId> {
        self.leaf(leaf)?.active
    }

    /// The layout without what changes often, for telling whether
    /// the views need building again.
    pub fn shape(&self) -> Option<Shape> {
        self.shape_of(self.root?)
    }

    fn shape_of(&self, id: NodeId) -> Option<Shape> {
        Some(match self.nodes.get(&id)? {
            DockNode::Leaf(leaf) => {
                Shape::Leaf(id, leaf.style.clone())
            }
            DockNode::Split(split) => Shape::Split(
                id,
                split.axis,
                Box::new(self.shape_of(split.a)?),
                Box::new(self.shape_of(split.b)?),
            ),
        })
    }

    /// Makes a new leaf the root.
    pub fn set_root_leaf(&mut self, leaf: DockLeaf) -> NodeId {
        let id = self.insert(DockNode::Leaf(leaf));
        self.root = Some(id);
        id
    }

    /// The leaves under `root`, in layout order.
    pub fn leaves_under(
        &self,
        root: NodeId,
    ) -> Vec<(NodeId, &DockLeaf)> {
        let mut out = Vec::new();
        self.leaves_under_inner(root, &mut out);
        out
    }

    fn leaves_under_inner<'a>(
        &'a self,
        id: NodeId,
        out: &mut Vec<(NodeId, &'a DockLeaf)>,
    ) {
        match self.nodes.get(&id) {
            Some(DockNode::Leaf(l)) => out.push((id, l)),
            Some(DockNode::Split(s)) => {
                let (a, b) = (s.a, s.b);
                self.leaves_under_inner(a, out);
                self.leaves_under_inner(b, out);
            }
            None => {}
        }
    }

    /// The first leaf with a tab of `window_id`.
    pub fn find_leaf_with_window(
        &self,
        window_id: &str,
    ) -> Option<NodeId> {
        self.nodes.iter().find_map(|(id, node)| match node {
            DockNode::Leaf(l) if l.has_window(window_id) => Some(*id),
            _ => None,
        })
    }

    pub fn find_leaf_for_tab(&self, tab: TabId) -> Option<NodeId> {
        self.nodes.iter().find_map(|(id, node)| match node {
            DockNode::Leaf(l)
                if l.windows.iter().any(|t| t.id == tab) =>
            {
                Some(*id)
            }
            _ => None,
        })
    }

    /// Every `(leaf, tab)` pair.
    pub fn tabs(
        &self,
    ) -> impl Iterator<Item = (NodeId, &DockTabEntry)> {
        self.leaves().flat_map(|(leaf_id, leaf)| {
            leaf.windows.iter().map(move |t| (leaf_id, t))
        })
    }

    pub fn find_by_area_id(&self, area_id: &str) -> Option<NodeId> {
        self.nodes.iter().find_map(|(id, node)| match node {
            DockNode::Leaf(l) if l.area_id == area_id => Some(*id),
            _ => None,
        })
    }

    /// The split holding `child`, `None` for the root.
    pub fn parent_of(&self, child: NodeId) -> Option<NodeId> {
        self.nodes.iter().find_map(|(id, node)| match node {
            DockNode::Split(s) if s.a == child || s.b == child => {
                Some(*id)
            }
            _ => None,
        })
    }

    /// Every leaf, in no particular order.
    pub fn leaves(
        &self,
    ) -> impl Iterator<Item = (NodeId, &DockLeaf)> {
        self.nodes.iter().filter_map(|(id, node)| match node {
            DockNode::Leaf(l) => Some((*id, l)),
            _ => None,
        })
    }

    /// Splits the leaf `target` along `edge`, with a new leaf of one
    /// tab of `window` on that side. Returns the new leaf and its
    /// tab. `None` if `target` is not a leaf.
    pub fn split(
        &mut self,
        target: NodeId,
        edge: Edge,
        window: String,
    ) -> Option<(NodeId, TabId)> {
        self.leaf(target)?;
        let entry = DockTabEntry {
            window_id: window,
            id: self.fresh_tab_id(),
        };
        let tab = entry.id;
        let leaf = self.split_with(target, edge, entry)?;
        Some((leaf, tab))
    }

    /// Moves `tab` into a new leaf on the `edge` side of the leaf
    /// `target`, keeping its id. A source leaf left empty is removed
    /// unless it is persistent. Returns the new leaf, or `None` if
    /// there is nothing to do: `tab` is not in the tree, `target` is
    /// not a leaf, or `tab` is the only one of `target`.
    pub fn split_tab(
        &mut self,
        target: NodeId,
        edge: Edge,
        tab: TabId,
    ) -> Option<NodeId> {
        self.leaf(target)?;
        let from = self.find_leaf_for_tab(tab)?;
        if from == target && self.leaf(from)?.windows.len() == 1 {
            return None;
        }
        let DockNode::Leaf(source) = self.nodes.get_mut(&from)?
        else {
            return None;
        };
        let at = source.tab_index(tab)?;
        let entry = source.windows.remove(at);
        if source.active == Some(tab) {
            source.active = source.windows.first().map(|t| t.id);
        }
        let leaf = self.split_with(target, edge, entry);
        self.simplify();
        leaf
    }

    /// Splits `target` with a new leaf holding `entry` on the `edge`
    /// side.
    fn split_with(
        &mut self,
        target: NodeId,
        edge: Edge,
        entry: DockTabEntry,
    ) -> Option<NodeId> {
        let new_style = self.leaf(target)?.style.clone();

        let new_leaf_id = self.fresh_id();
        self.nodes.insert(
            new_leaf_id,
            DockNode::Leaf(DockLeaf {
                area_id: fresh_area_id(&entry.window_id, new_leaf_id),
                style: new_style,
                active: Some(entry.id),
                windows: vec![entry],
                persistent: false,
            }),
        );

        let parent = self.parent_of(target);

        let (a, b) = if edge.puts_new_in_a() {
            (new_leaf_id, target)
        } else {
            (target, new_leaf_id)
        };
        let split_id = self.insert(DockNode::Split(DockSplit {
            axis: edge.axis(),
            fraction: 0.5,
            a,
            b,
        }));

        self.replace_child(parent, target, split_id);

        Some(new_leaf_id)
    }

    /// Points `parent`, or the root when there is none, at `with`
    /// where it pointed at `child`.
    fn replace_child(
        &mut self,
        parent: Option<NodeId>,
        child: NodeId,
        with: NodeId,
    ) {
        match parent {
            Some(parent_id) => {
                if let Some(DockNode::Split(s)) =
                    self.nodes.get_mut(&parent_id)
                {
                    if s.a == child {
                        s.a = with;
                    }
                    if s.b == child {
                        s.b = with;
                    }
                }
            }
            None => {
                if self.root == Some(child) {
                    self.root = Some(with);
                }
            }
        }
    }

    /// Sets a split's fraction, kept within `(0.05, 0.95)`.
    pub fn set_fraction(&mut self, split: NodeId, fraction: f32) {
        if let Some(DockNode::Split(s)) = self.nodes.get_mut(&split) {
            s.fraction = fraction.clamp(0.05, 0.95);
        }
    }

    /// Makes `tab` the active tab of `leaf`, if `leaf` holds it.
    pub fn set_active(&mut self, leaf: NodeId, tab: TabId) {
        if let Some(DockNode::Leaf(l)) = self.nodes.get_mut(&leaf)
            && l.windows.iter().any(|t| t.id == tab)
        {
            l.active = Some(tab);
        }
    }

    /// Moves `tab` to the end of the leaf `to` and makes it active.
    /// A source leaf left empty is removed unless it is persistent.
    pub fn move_tab(&mut self, tab: TabId, to: NodeId) {
        self.insert_tab(tab, to, false, None);
    }

    /// Moves `tab` into the leaf `to` at `index`, or at the end for
    /// `None`. A move within the same leaf only happens with
    /// `allow_same`.
    pub fn insert_tab(
        &mut self,
        tab: TabId,
        to: NodeId,
        allow_same: bool,
        index: Option<usize>,
    ) {
        let Some(from) = self.find_leaf_for_tab(tab) else {
            return;
        };
        if !allow_same && from == to {
            return;
        }
        if self.leaf(to).is_none() {
            return;
        }
        let (entry, source_index) = {
            let Some(DockNode::Leaf(l)) = self.nodes.get_mut(&from)
            else {
                return;
            };
            let Some(pos) =
                l.windows.iter().position(|t| t.id == tab)
            else {
                return;
            };
            let entry = l.windows.remove(pos);
            if l.active == Some(tab) {
                l.active = l.windows.first().map(|t| t.id);
            }
            (entry, pos)
        };
        if let Some(DockNode::Leaf(l)) = self.nodes.get_mut(&to) {
            let new_id = entry.id;
            match index {
                Some(idx) => {
                    // Taking the tab out shifted every later slot of
                    // its own leaf left.
                    let idx = if from == to && idx > source_index {
                        idx - 1
                    } else {
                        idx
                    };
                    l.windows.insert(idx.min(l.windows.len()), entry);
                }
                None => l.windows.push(entry),
            }
            l.active = Some(new_id);
        }
        self.simplify();
    }

    /// Removes `tab`. A leaf left empty is removed unless it is
    /// persistent.
    pub fn remove_tab(&mut self, tab: TabId) {
        let Some(leaf) = self.find_leaf_for_tab(tab) else {
            return;
        };
        if let Some(DockNode::Leaf(l)) = self.nodes.get_mut(&leaf) {
            l.windows.retain(|t| t.id != tab);
            if l.active == Some(tab) {
                l.active = l.windows.first().map(|t| t.id);
            }
        }
        self.simplify();
    }

    /// Removes every tab of `window_id`.
    pub fn remove_window_kind(&mut self, window_id: &str) {
        let to_remove = self
            .tabs()
            .filter(|(_, t)| t.window_id == window_id)
            .map(|(_, t)| t.id)
            .collect::<Vec<_>>();
        for tab in to_remove {
            self.remove_tab(tab);
        }
    }

    /// Removes the empty leaves that are neither the root nor
    /// persistent. The sibling of a removed leaf takes the place of
    /// their split.
    pub fn simplify(&mut self) {
        loop {
            let root = self.root;
            let empty = self
                .nodes
                .iter()
                .find(|(id, node)| match node {
                    DockNode::Leaf(l) => {
                        l.windows.is_empty()
                            && Some(**id) != root
                            && !l.is_persistent()
                    }
                    _ => false,
                })
                .map(|(id, _)| *id);

            let Some(empty_id) = empty else {
                return;
            };
            let Some(parent_id) = self.parent_of(empty_id) else {
                self.nodes.remove(&empty_id);
                continue;
            };
            let Some(DockNode::Split(s)) =
                self.nodes.get(&parent_id).cloned()
            else {
                return;
            };
            let survivor = if s.a == empty_id { s.b } else { s.a };

            let grandparent = self.parent_of(parent_id);
            self.replace_child(grandparent, parent_id, survivor);

            self.nodes.remove(&empty_id);
            self.nodes.remove(&parent_id);
        }
    }
}

/// An area id unique to the leaf `split` makes.
fn fresh_area_id(window_id: &str, leaf_id: NodeId) -> String {
    format!("split.{window_id}.{}", leaf_id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(area_id: &str, windows: &[&str]) -> DockLeaf {
        DockLeaf::new(area_id, DockAreaStyle::TabBar).with_windows(
            windows.iter().map(ToString::to_string).collect(),
        )
    }

    fn window_ids(t: &DockTree, leaf: NodeId) -> Vec<String> {
        t.nodes[&leaf]
            .as_leaf()
            .unwrap()
            .windows
            .iter()
            .map(|w| w.window_id.clone())
            .collect()
    }

    fn active_window_id(t: &DockTree, leaf: NodeId) -> Option<&str> {
        let l = t.nodes[&leaf].as_leaf()?;
        let id = l.active?;
        l.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.window_id.as_str())
    }

    fn tab_id_for(
        t: &DockTree,
        leaf: NodeId,
        window_id: &str,
    ) -> TabId {
        t.nodes[&leaf]
            .as_leaf()
            .unwrap()
            .windows
            .iter()
            .find(|w| w.window_id == window_id)
            .unwrap()
            .id
    }

    #[test]
    fn set_root_leaf_works() {
        let mut t = DockTree::new();
        let id = t.set_root_leaf(leaf("root", &["a"]));
        assert_eq!(t.root, Some(id));
        assert_eq!(t.leaves().count(), 1);
    }

    #[test]
    fn pending_tab_ids_are_stamped_on_insert() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b"]));
        let l = t.nodes[&root].as_leaf().unwrap();
        assert!(l.windows.iter().all(|w| w.id != TabId::PENDING));
        assert_eq!(l.active, Some(l.windows[0].id));
    }

    #[test]
    fn split_inserts_new_leaf_and_wraps_target() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let (new_leaf, _) =
            t.split(root, Edge::Right, "b".into()).unwrap();

        let root_split =
            t.nodes[&t.root.unwrap()].as_split().unwrap();
        assert_eq!(root_split.axis, SplitAxis::Horizontal);
        assert_eq!(root_split.a, root);
        assert_eq!(root_split.b, new_leaf);
        assert_eq!(root_split.fraction, 0.5);

        assert_eq!(window_ids(&t, root), vec!["a"]);
        assert_eq!(window_ids(&t, new_leaf), vec!["b"]);
    }

    #[test]
    fn split_top_puts_new_in_a() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let (new_leaf, _) =
            t.split(root, Edge::Top, "b".into()).unwrap();
        let s = t.nodes[&t.root.unwrap()].as_split().unwrap();
        assert_eq!(s.a, new_leaf);
        assert_eq!(s.b, root);
    }

    #[test]
    fn split_bottom_puts_new_in_b() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let (new_leaf, _) =
            t.split(root, Edge::Bottom, "b".into()).unwrap();
        let s = t.nodes[&t.root.unwrap()].as_split().unwrap();
        assert_eq!(s.a, root);
        assert_eq!(s.b, new_leaf);
    }

    #[test]
    fn split_of_nested_leaf_preserves_other_sibling() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("left", &["a"]));
        let (right, _) =
            t.split(root, Edge::Right, "b".into()).unwrap();
        let _deeper =
            t.split(right, Edge::Bottom, "c".into()).unwrap();

        assert_eq!(window_ids(&t, root), vec!["a"]);
        let b_leaf = t.find_leaf_with_window("b").unwrap();
        assert_eq!(window_ids(&t, b_leaf), vec!["b"]);
    }

    #[test]
    fn move_tab_relocates_and_activates() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b"]));
        let (right, _) =
            t.split(root, Edge::Right, "c".into()).unwrap();
        let tab_a = tab_id_for(&t, root, "a");
        t.move_tab(tab_a, right);

        assert_eq!(window_ids(&t, root), vec!["b"]);
        assert_eq!(window_ids(&t, right), vec!["c", "a"]);
        assert_eq!(active_window_id(&t, right), Some("a"));
    }

    #[test]
    fn same_leaf_forward_reorder_lands_at_requested_index() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b", "c"]));
        let tab_a = tab_id_for(&t, root, "a");

        t.insert_tab(tab_a, root, true, Some(2));

        assert_eq!(window_ids(&t, root), vec!["b", "a", "c"]);
        assert_eq!(active_window_id(&t, root), Some("a"));
    }

    #[test]
    fn same_leaf_backward_reorder_is_unshifted() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b", "c"]));
        let tab_c = tab_id_for(&t, root, "c");

        t.insert_tab(tab_c, root, true, Some(0));

        assert_eq!(window_ids(&t, root), vec!["c", "a", "b"]);
    }

    #[test]
    fn move_last_tab_simplifies_tree() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let (right, _) =
            t.split(root, Edge::Right, "b".into()).unwrap();
        let tab_a = tab_id_for(&t, root, "a");
        t.move_tab(tab_a, right);

        assert!(matches!(
            t.nodes[&t.root.unwrap()],
            DockNode::Leaf(_)
        ));
        assert_eq!(t.leaves().count(), 1);
        let surviving = t.root.unwrap();
        assert_eq!(window_ids(&t, surviving), vec!["b", "a"]);
    }

    #[test]
    fn remove_last_tab_keeps_root_empty_leaf() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let tab_a = tab_id_for(&t, root, "a");
        t.remove_tab(tab_a);

        assert_eq!(t.root, Some(root));
        assert!(t.nodes[&root].as_leaf().unwrap().windows.is_empty());
    }

    #[test]
    fn set_fraction_clamps() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        t.split(root, Edge::Right, "b".into());
        let split_id = t.root.unwrap();
        t.set_fraction(split_id, 0.0);
        assert!(
            t.nodes[&split_id].as_split().unwrap().fraction >= 0.05
        );
        t.set_fraction(split_id, 1.5);
        assert!(
            t.nodes[&split_id].as_split().unwrap().fraction <= 0.95
        );
    }

    #[test]
    fn set_active_requires_tab_in_leaf() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b"]));
        let tab_b = tab_id_for(&t, root, "b");
        t.set_active(root, tab_b);
        assert_eq!(active_window_id(&t, root), Some("b"));
        t.set_active(root, TabId(9999));
        assert_eq!(active_window_id(&t, root), Some("b"));
    }

    #[test]
    fn duplicate_window_kind_supported() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(DockLeaf::new(
            "root",
            DockAreaStyle::TabBar,
        ));
        let first = t.add_tab(root, "outliner").unwrap();
        let second = t.add_tab(root, "outliner").unwrap();
        assert_ne!(first, second);
        assert_eq!(
            window_ids(&t, root),
            vec!["outliner", "outliner"]
        );

        t.remove_tab(second);
        let l = t.nodes[&root].as_leaf().unwrap();
        assert_eq!(l.windows.len(), 1);
        assert_eq!(l.windows[0].id, first);
        assert_eq!(l.active, Some(first));
    }

    #[test]
    fn persistent_leaf_kept_when_emptied_via_simplify() {
        let mut t = DockTree::new();
        let original = t.insert(DockNode::Leaf(
            DockLeaf::new("right", DockAreaStyle::TabBar)
                .with_windows(vec!["a".into()])
                .persistent(),
        ));
        t.root = Some(original);
        let (other, _) =
            t.split(original, Edge::Right, "b".into()).unwrap();
        let tab_a = tab_id_for(&t, original, "a");
        t.move_tab(tab_a, other);
        let persistent_leaf = t.nodes[&original].as_leaf().unwrap();
        assert!(persistent_leaf.windows.is_empty());
        assert!(persistent_leaf.is_persistent());
    }

    #[test]
    fn nested_split_chain_simplifies_when_drained() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let (right, _) =
            t.split(root, Edge::Right, "b".into()).unwrap();
        let _bottom =
            t.split(right, Edge::Bottom, "c".into()).unwrap();

        let tab_b = tab_id_for(&t, right, "b");
        t.move_tab(tab_b, root);
        let bottom_leaf = t.find_leaf_with_window("c").unwrap();
        let tab_c = tab_id_for(&t, bottom_leaf, "c");
        t.move_tab(tab_c, root);

        assert!(matches!(
            t.nodes[&t.root.unwrap()],
            DockNode::Leaf(_)
        ));
        assert_eq!(t.leaves().count(), 1);
    }

    #[test]
    fn split_tab_moves_the_tab_into_a_new_leaf_keeping_its_id() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b"]));
        let tab_b = tab_id_for(&t, root, "b");

        let new_leaf = t.split_tab(root, Edge::Left, tab_b).unwrap();

        let s = t.nodes[&t.root.unwrap()].as_split().unwrap();
        assert_eq!((s.a, s.b), (new_leaf, root));
        assert_eq!(window_ids(&t, root), vec!["a"]);
        assert_eq!(window_ids(&t, new_leaf), vec!["b"]);
        assert_eq!(tab_id_for(&t, new_leaf, "b"), tab_b);
        assert_eq!(active_window_id(&t, new_leaf), Some("b"));
        assert_eq!(active_window_id(&t, root), Some("a"));
    }

    #[test]
    fn split_tab_removes_a_source_leaf_it_empties() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let (right, _) =
            t.split(root, Edge::Right, "b".into()).unwrap();
        let (_, _) =
            t.split(right, Edge::Bottom, "c".into()).unwrap();
        let tab_a = tab_id_for(&t, root, "a");

        let new_leaf = t.split_tab(right, Edge::Top, tab_a).unwrap();

        assert!(t.get(root).is_none(), "the emptied leaf is gone");
        assert_eq!(window_ids(&t, new_leaf), vec!["a"]);
        assert_eq!(t.leaves().count(), 3);
        assert!(matches!(
            t.nodes[&t.root.unwrap()],
            DockNode::Split(_)
        ));
    }

    #[test]
    fn split_tab_of_a_lone_tab_onto_its_own_leaf_does_nothing() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a"]));
        let tab_a = tab_id_for(&t, root, "a");

        assert_eq!(t.split_tab(root, Edge::Right, tab_a), None);
        assert_eq!(t.leaves().count(), 1);
        assert_eq!(t.root, Some(root));
    }

    #[test]
    fn the_shape_ignores_fractions_tabs_and_the_active_tab() {
        let mut t = DockTree::new();
        let root = t.set_root_leaf(leaf("root", &["a", "b"]));
        t.split(root, Edge::Right, "c".into());
        let before = t.shape();

        let split = t.root.unwrap();
        t.set_fraction(split, 0.3);
        let tab_b = tab_id_for(&t, root, "b");
        t.set_active(root, tab_b);
        t.add_tab(root, "d");
        t.remove_tab(tab_b);
        assert_eq!(t.shape(), before);

        t.split(root, Edge::Bottom, "e".into());
        assert_ne!(t.shape(), before);
    }
}

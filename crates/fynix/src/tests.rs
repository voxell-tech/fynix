//! The core against a fake backend: a flat list of nodes holding some
//! text at a size, and one number the text can be bound to.

use core::mem;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;
use std::string::{String, ToString};
use std::vec::Vec;
use std::{panic, vec};

use lenz::Lenz;
use motiongfx_interp::ease;
use motiongfx_interp::interpolation::Interpolation;

mod leave;
mod state;
mod structure;

use motiongfx_interp::interpolation::InterpFn;

use crate::{
    AnyView, Backend, Condition, Curve, Cx, Element, Motion,
    MotionTokens, Mounted, Prop, Signal, Styled, Tick, Trace, Tween,
    View, derived,
};

#[derive(Default)]
pub struct World {
    nodes: Vec<Option<Node>>,
    count: i32,
    /// Bumped by every change to `count`.
    version: u32,
    /// The nodes despawned since the last update.
    gone: Vec<usize>,
    /// A number the structural views key on.
    pick: u32,
    /// The list `each` shows.
    list: Vec<u32>,
    /// The nodes a state rule is read on.
    watched: Vec<usize>,
}

#[derive(Default)]
struct Node {
    parent: Option<usize>,
    children: Vec<usize>,
    text: String,
    size: f32,
    /// The fake backend's one state.
    lit: bool,
    leaving: bool,
    /// How far the space it takes has collapsed, once it has begun.
    collapsed: Option<f32>,
}

struct Fake;

impl Backend for Fake {
    type World = World;
    type Node = usize;

    fn spawn(world: &mut World, parent: Option<usize>) -> usize {
        world.nodes.push(Some(Node {
            parent,
            ..Node::default()
        }));
        let node = world.nodes.len() - 1;
        if let Some(parent) = parent {
            world.nodes[parent]
                .as_mut()
                .expect("a live parent")
                .children
                .push(node);
        }
        node
    }

    fn despawn(world: &mut World, node: usize) {
        let Some(gone) = world.nodes[node].take() else {
            return;
        };
        world.gone.push(node);
        if let Some(parent) = gone.parent
            && let Some(parent) = world.nodes[parent].as_mut()
        {
            parent.children.retain(|&child| child != node);
        }
        for child in gone.children {
            Self::despawn(world, child);
        }
    }

    fn reorder(world: &mut World, parent: usize, children: &[usize]) {
        world.nodes[parent]
            .as_mut()
            .expect("a live parent")
            .children = children.to_vec();
    }

    fn leave(world: &mut World, node: usize) {
        if let Some(node) = world.nodes[node].as_mut() {
            node.leaving = true;
        }
    }

    fn collapse(world: &mut World, node: usize, progress: f32) {
        if let Some(node) = world.nodes[node].as_mut() {
            node.collapsed = Some(progress);
        }
    }
}

/// A signal reading `read`, checked against the world's version.
fn watch<T>(
    read: impl Fn(&World) -> T + Send + Sync + 'static,
) -> Signal<World, T> {
    let mut seen = None;
    derived(read).when(move |world: &World| {
        let changed = seen != Some(world.version);
        seen = Some(world.version);
        changed
    })
}

impl World {
    fn set(&mut self, count: i32) {
        self.count = count;
        self.version += 1;
    }

    fn node(&self, node: usize) -> &Node {
        self.nodes[node].as_ref().expect("a live node")
    }

    fn children(&self, parent: usize) -> Vec<usize> {
        self.node(parent).children.clone()
    }

    fn is_alive(&self, node: usize) -> bool {
        self.nodes[node].is_some()
    }

    fn light(&mut self, node: usize, lit: bool) {
        self.nodes[node].as_mut().expect("a live node").lit = lit;
    }
}

/// What the fake backend's one element reads from a theme.
trait Sizes {
    fn body(&self) -> f32;
}

/// A run of text.
#[derive(Lenz)]
pub struct Text {
    text: Prop<World, String>,
    size: Prop<World, f32>,
    #[lenz(ignore)]
    motion: Option<Motion>,
}

fn text(text: impl Into<Prop<World, String>>) -> Text {
    Text {
        text: text.into(),
        ..Text::unset()
    }
}

impl Text {
    fn size(mut self, size: impl Into<Prop<World, f32>>) -> Self {
        self.size = size.into();
        self
    }

    fn transition(mut self, motion: Motion) -> Self {
        self.motion = Some(motion);
        self
    }
}

crate::styled!(Text { text, size, motion });

#[derive(Clone, Debug, PartialEq)]
struct Shown {
    text: String,
    size: f32,
}

impl Shown {
    fn interp(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            text: to.text.clone(),
            size: <f32 as Interpolation<()>>::interp(
                &from.size, &to.size, t,
            ),
        }
    }
}

impl<T: Sizes + MotionTokens> Element<Fake, T> for Text {
    type Snapshot = Shown;

    fn prepare(_: &mut World, _: usize) {}

    fn snapshot(&self, world: &World, theme: &T) -> Shown {
        Shown {
            text: self.text.get(world).unwrap_or_default(),
            size: self.size.get(world).unwrap_or(theme.body()),
        }
    }

    fn write(shown: &Shown, world: &mut World, node: usize) {
        let node = world.nodes[node].as_mut().expect("a live node");
        node.text = shown.text.clone();
        node.size = shown.size;
    }

    fn is_live(&self) -> bool {
        self.text.is_bound() || self.size.is_bound()
    }

    fn changed(&mut self, world: &World) -> bool {
        self.text.changed(world) | self.size.changed(world)
    }

    fn tween(&self, theme: &T) -> Option<Tween<Shown>> {
        let curve = theme.motion(self.motion?);
        Some(Tween {
            curve,
            interp: Shown::interp,
        })
    }

    fn interp() -> Option<InterpFn<Shown>> {
        Some(Shown::interp)
    }
}

/// The fake backend's one state: a node is lit.
struct Lit;

impl Condition<Fake> for Lit {
    fn holds(world: &World, node: usize) -> bool {
        world.node(node).lit
    }

    fn watch(world: &mut World, node: usize) {
        if !world.watched.contains(&node) {
            world.watched.push(node);
        }
    }
}

/// One app's theme.
struct Warm;

impl Sizes for Warm {
    fn body(&self) -> f32 {
        14.0
    }
}

impl MotionTokens for Warm {
    fn motion(&self, _: Motion) -> Curve {
        Curve {
            duration: Duration::from_secs(1),
            ease: ease::linear,
        }
    }
}

/// Another app's, stored nothing like the first.
struct Cold {
    sizes: [f32; 1],
}

impl Sizes for Cold {
    fn body(&self) -> f32 {
        self.sizes[0]
    }
}

impl MotionTokens for Cold {
    fn motion(&self, _: Motion) -> Curve {
        Curve {
            duration: Duration::ZERO,
            ease: ease::linear,
        }
    }
}

/// A world, a theme and its mounted elements, built into together.
struct Ui<T> {
    world: World,
    theme: T,
    mounted: Mounted<Fake, T>,
}

impl<T: 'static> Ui<T> {
    fn new(theme: T) -> Self {
        Self {
            world: World::default(),
            theme,
            mounted: Mounted::default(),
        }
    }

    fn build(&mut self, view: impl View<Fake, T>) -> usize {
        let mut cx =
            Cx::new(&mut self.world, &self.theme, &mut self.mounted);
        view.build(&mut cx)
    }

    /// Drops what was despawned since the last call, as a backend
    /// does.
    fn unmount_gone(&mut self) {
        for node in mem::take(&mut self.world.gone) {
            self.mounted.unmount(node);
        }
    }

    fn update(&mut self, delta: Duration, reduced_motion: bool) {
        self.unmount_gone();
        self.mounted.update_structure(&mut self.world, &self.theme);
        self.unmount_gone();
        self.mounted.update_elements(
            &mut self.world,
            &self.theme,
            Tick {
                delta,
                reduced_motion,
            },
        );
    }

    /// A root node with whatever `build` puts under it.
    fn under(
        &mut self,
        build: impl FnOnce(&mut Cx<'_, Fake, T>) + 'static,
    ) -> Vec<usize> {
        let root =
            self.build(AnyView::new(|cx: &mut Cx<'_, Fake, T>| {
                let root = cx.spawn();
                cx.under(root, build);
                root
            }));
        self.world.children(root)
    }
}

#[test]
fn an_unset_prop_falls_back_to_the_theme() {
    let mut ui = Ui::new(Warm);
    let node = ui.build(text("Save"));

    assert_eq!(ui.world.node(node).text, "Save");
    assert_eq!(ui.world.node(node).size, 14.0);
}

#[test]
fn the_same_view_works_under_two_unrelated_themes() {
    let mut warm = Ui::new(Warm);
    let mut cold = Ui::new(Cold { sizes: [20.0] });
    let in_warm = warm.build(text("x"));
    let in_cold = cold.build(text("x"));

    assert_eq!(warm.world.node(in_warm).size, 14.0);
    assert_eq!(cold.world.node(in_cold).size, 20.0);
}

#[test]
fn a_set_rule_fills_only_what_the_call_site_left_unset() {
    let mut ui = Ui::new(Warm);
    let [ruled, explicit] = ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.build(text("ruled"));
        cx.build(text("explicit").size(9.0));
    })[..] else {
        panic!("two nodes");
    };

    assert_eq!(ui.world.node(ruled).size, 20.0);
    assert_eq!(ui.world.node(explicit).size, 9.0, "call site wins");
}

#[test]
fn a_rule_can_read_the_theme() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.set::<Text>(|t, theme: &Warm| t.size(theme.body() * 2.0));
        cx.build(text("big"));
    });

    assert_eq!(ui.world.node(nodes[0]).size, 28.0);
}

#[test]
fn an_inner_scope_wins_and_ends_with_its_scope() {
    let mut ui = Ui::new(Warm);
    let [inner, after] = ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.scope(|cx| {
            cx.set::<Text>(|t, _| t.size(30.0));
            cx.build(text("inner"));
        });
        cx.build(text("after"));
    })[..] else {
        panic!("two nodes");
    };

    assert_eq!(ui.world.node(inner).size, 30.0);
    assert_eq!(ui.world.node(after).size, 20.0);
}

#[test]
fn a_show_rule_wins_over_the_call_site() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.show::<Text>(|t, _| t.size(1.0));
        cx.build(text("x").size(9.0));
    });

    assert_eq!(ui.world.node(nodes[0]).size, 1.0);
}

#[test]
fn a_bound_prop_follows_the_world() {
    let mut ui = Ui::new(Warm);
    ui.world.set(1);
    let bound = ui
        .build(text(watch(|world: &World| world.count.to_string())));
    let fixed = ui.build(text("fixed"));
    assert_eq!(ui.world.node(bound).text, "1");

    ui.world.set(2);
    ui.update(Duration::ZERO, false);

    assert_eq!(ui.world.node(bound).text, "2");
    assert_eq!(ui.world.node(fixed).text, "fixed");
    assert_eq!(
        ui.mounted.len(),
        1,
        "only what can change stays mounted"
    );
}

/// A text bound to `world.count`, counting its reads in `reads`.
fn read_counted(reads: &'static AtomicUsize) -> Text {
    text(watch(move |world: &World| {
        reads.fetch_add(1, Ordering::Relaxed);
        world.count.to_string()
    }))
}

#[test]
fn an_unmounted_node_is_dropped() {
    let mut ui = Ui::new(Warm);
    let node = ui
        .build(text(watch(|world: &World| world.count.to_string())));
    ui.mounted.unmount(node);

    assert!(ui.mounted.is_empty());
    ui.world.set(1);
    ui.update(Duration::ZERO, false);
    assert_eq!(ui.world.node(node).text, "0", "no longer written");
}

#[test]
fn an_element_whose_source_did_not_change_is_not_re_read() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut ui = Ui::new(Warm);
    ui.build(read_counted(&READS));
    let built = READS.load(Ordering::Relaxed);

    ui.update(Duration::ZERO, false);
    ui.update(Duration::ZERO, false);

    assert_eq!(READS.load(Ordering::Relaxed), built);
}

#[test]
fn an_element_whose_source_changed_is_re_read_and_written() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut ui = Ui::new(Warm);
    let node = ui.build(read_counted(&READS));
    let built = READS.load(Ordering::Relaxed);

    ui.world.set(5);
    ui.update(Duration::ZERO, false);

    assert_eq!(READS.load(Ordering::Relaxed), built + 1);
    assert_eq!(ui.world.node(node).text, "5");
}

#[test]
fn the_first_update_after_mounting_does_not_re_read() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut ui = Ui::new(Warm);
    ui.build(read_counted(&READS));

    // Built once, and the check fired on its first call at mount.
    assert_eq!(READS.load(Ordering::Relaxed), 1);
    ui.update(Duration::ZERO, false);

    assert_eq!(READS.load(Ordering::Relaxed), 1);
}

#[test]
fn mark_dirty_forces_one_re_read() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut ui = Ui::new(Warm);
    let node = ui.build(read_counted(&READS));

    ui.mounted.mark_dirty(node);
    ui.update(Duration::ZERO, false);
    assert_eq!(READS.load(Ordering::Relaxed), 2);

    ui.update(Duration::ZERO, false);
    assert_eq!(READS.load(Ordering::Relaxed), 2, "only once");
}

#[test]
fn a_running_transition_advances_without_a_source_change() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut ui = Ui::new(Warm);
    let node = ui.build(
        text("x")
            .size(watch(|world: &World| {
                READS.fetch_add(1, Ordering::Relaxed);
                world.count as f32
            }))
            .transition(Motion::Interact),
    );

    ui.world.set(10);
    ui.update(Duration::ZERO, false);
    ui.update(Duration::from_millis(500), false);
    let reads = READS.load(Ordering::Relaxed);
    let halfway = ui.world.node(node).size;
    assert!(halfway > 0.0 && halfway < 10.0);

    ui.update(Duration::from_millis(250), false);
    assert!(ui.world.node(node).size > halfway, "still travelling");
    ui.update(Duration::from_millis(500), false);
    assert_eq!(ui.world.node(node).size, 10.0);
    assert_eq!(
        READS.load(Ordering::Relaxed),
        reads,
        "and never re-read for it"
    );
}

/// A text whose size follows `world.count`, travelling when asked.
fn counted(motion: Option<Motion>) -> Text {
    let text =
        text("x").size(watch(|world: &World| world.count as f32));
    match motion {
        Some(motion) => text.transition(motion),
        None => text,
    }
}

#[test]
fn nothing_travels_unless_asked() {
    let mut ui = Ui::new(Warm);
    ui.world.set(10);
    let node = ui.build(counted(None));

    ui.world.set(20);
    ui.update(Duration::from_millis(100), false);

    assert_eq!(ui.world.node(node).size, 20.0);
}

#[test]
fn a_transition_reaches_its_target_over_the_curve() {
    let mut ui = Ui::new(Warm);
    ui.world.set(10);
    let node = ui.build(counted(Some(Motion::Interact)));

    ui.world.set(20);
    ui.update(Duration::ZERO, false);
    ui.update(Duration::from_millis(500), false);
    assert_eq!(ui.world.node(node).size, 15.0);

    ui.update(Duration::from_millis(500), false);
    assert_eq!(ui.world.node(node).size, 20.0);
}

#[test]
fn an_interrupted_transition_starts_from_where_it_got_to() {
    let mut ui = Ui::new(Warm);
    ui.world.set(0);
    let node = ui.build(counted(Some(Motion::Interact)));

    ui.world.set(100);
    ui.update(Duration::ZERO, false);
    ui.update(Duration::from_millis(500), false);
    assert_eq!(ui.world.node(node).size, 50.0);

    // Heads back to 0 from 50, not from 100.
    ui.world.set(0);
    ui.update(Duration::ZERO, false);
    ui.update(Duration::from_millis(500), false);
    assert_eq!(ui.world.node(node).size, 25.0);
}

#[test]
fn reduced_motion_snaps() {
    let mut ui = Ui::new(Warm);
    ui.world.set(10);
    let node = ui.build(counted(Some(Motion::Interact)));

    ui.world.set(20);
    ui.update(Duration::from_millis(100), true);

    assert_eq!(ui.world.node(node).size, 20.0);
}

#[test]
fn a_zero_length_curve_snaps() {
    let mut ui = Ui::new(Cold { sizes: [12.0] });
    ui.world.set(10);
    let node = ui.build(counted(Some(Motion::Interact)));

    ui.world.set(20);
    ui.update(Duration::from_millis(1), false);

    assert_eq!(ui.world.node(node).size, 20.0);
}

/// A composite: a title over a body, both texts of its own.
#[derive(Lenz)]
pub struct Card {
    title: Text,
    body: Text,
}

fn card(title: &str, body: &str) -> Card {
    Card {
        title: text(title),
        body: text(body),
    }
}

impl Styled for Card {
    fn unset() -> Self {
        Self {
            title: Text::unset(),
            body: Text::unset(),
        }
    }

    fn over(self, below: Self) -> Self {
        Self {
            title: self.title.over(below.title),
            body: self.body.over(below.body),
        }
    }
}

// For one theme only: generic over `T`, it would overlap the core's
// `View` for every `Element`, since another crate could make `Card` an
// `Element<Fake, ItsTheme>`.
impl View<Fake, Warm> for Card {
    fn build(self, cx: &mut Cx<'_, Fake, Warm>) -> usize {
        let card = cx.resolve(self);
        let root = cx.spawn();
        cx.under(root, |cx| {
            cx.build(card.title);
            cx.build(card.body);
        });
        root
    }
}

#[test]
fn a_field_rule_fills_only_what_the_call_site_left_unset() {
    let mut ui = Ui::new(Warm);
    let [ruled, explicit] = ui.under(|cx| {
        cx.set_field(Text::cursor().size(), 20.0);
        cx.build(text("ruled"));
        cx.build(text("explicit").size(9.0));
    })[..] else {
        panic!("two nodes");
    };

    assert_eq!(ui.world.node(ruled).size, 20.0);
    assert_eq!(ui.world.node(explicit).size, 9.0, "call site wins");
}

#[test]
fn a_field_rule_can_read_the_theme() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.set_field_with(Text::cursor().size(), |theme: &Warm| {
            theme.body() * 2.0
        });
        cx.build(text("big"));
    });

    assert_eq!(ui.world.node(nodes[0]).size, 28.0);
}

#[test]
fn a_path_reaches_one_part_of_a_composite() {
    let mut ui = Ui::new(Warm);
    let [card, loose] = ui.under(|cx| {
        cx.set_field(Card::cursor().title().size(), 30.0);
        cx.build(card("Title", "Body"));
        cx.build(text("loose"));
    })[..] else {
        panic!("a card and a text");
    };
    let [title, body] = ui.world.children(card)[..] else {
        panic!("a title and a body");
    };

    assert_eq!(ui.world.node(title).size, 30.0);
    assert_eq!(
        ui.world.node(body).size,
        14.0,
        "the body is untouched"
    );
    assert_eq!(ui.world.node(loose).size, 14.0, "so is a lone text");
}

#[test]
fn a_composite_part_rule_beats_a_rule_for_the_part_s_kind() {
    let mut ui = Ui::new(Warm);
    let [card] = ui.under(|cx| {
        cx.set_field(Card::cursor().title().size(), 30.0);
        cx.set_field(Text::cursor().size(), 10.0);
        cx.build(card("Title", "Body"));
    })[..] else {
        panic!("a card");
    };
    let [title, body] = ui.world.children(card)[..] else {
        panic!("a title and a body");
    };

    // The card's rule reaches its title as if its call site set it.
    assert_eq!(ui.world.node(title).size, 30.0);
    assert_eq!(ui.world.node(body).size, 10.0);
}

#[test]
fn a_trace_names_the_path_rules_and_counts_the_rest() {
    let mut ui = Ui::new(Warm);
    ui.under(|cx| {
        cx.set_field(Text::cursor().size(), 20.0);
        cx.set::<Text>(|t, _| t.size(21.0));
        cx.scope(|cx| {
            cx.set_field(Text::cursor().text(), "x".to_string());
            cx.set_field(Text::cursor().size(), 22.0);

            let size = Text::cursor().size().key();
            assert_eq!(
                cx.trace::<Text>(size),
                Trace {
                    named: vec![0, 1],
                    opaque: 1,
                }
            );
        });
    });
}

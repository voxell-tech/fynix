//! `keyed` and `each` against the fake backend.

use core::time::Duration;
use std::format;
use std::string::{String, ToString};
use std::vec::Vec;

use super::{Fake, Text, TextCursor, Ui, Warm, World, text, watch};
use crate::{AnyView, Cx, Each, Keyed, View, ViewExt, each, keyed};

impl World {
    fn pick(&mut self, pick: u32) {
        self.pick = pick;
        self.version += 1;
    }

    fn show(&mut self, list: &[u32]) {
        self.list = list.to_vec();
        self.version += 1;
    }
}

/// A text of the key the world picks, built again when it changes.
fn picked() -> Keyed<Fake, Warm, u32> {
    keyed(watch(|world: &World| world.pick), |&pick| {
        text(pick.to_string()).boxed()
    })
}

/// The one node a `keyed` container holds.
fn only_child(ui: &Ui<Warm>, container: usize) -> usize {
    let [child] = ui.world.children(container)[..] else {
        panic!("one child");
    };
    child
}

/// The texts of the nodes under `container`, in order.
fn texts(ui: &Ui<Warm>, container: usize) -> Vec<String> {
    ui.world
        .children(container)
        .into_iter()
        .map(|node| ui.world.node(node).text.clone())
        .collect()
}

/// A composite that scopes a rule over its content.
struct Panel(AnyView<Fake, Warm>);

impl View<Fake, Warm> for Panel {
    fn build(self, cx: &mut Cx<'_, Fake, Warm>) -> usize {
        let root = cx.spawn();
        cx.under(root, |cx| {
            cx.scope(|cx| {
                cx.set::<Text>(|t, _| t.size(30.0));
                cx.build(self.0);
            });
        });
        root
    }
}

#[test]
fn a_keyed_view_builds_its_first_branch() {
    let mut ui = Ui::new(Warm);
    ui.world.pick(3);
    let [container] = ui.under(|cx| {
        cx.build(picked());
    })[..] else {
        panic!("a container");
    };

    assert_eq!(texts(&ui, container), ["3"]);
    assert_eq!(ui.mounted.structure_len(), 1);
}

#[test]
fn a_keyed_view_rebuilds_when_its_key_changes() {
    let mut ui = Ui::new(Warm);
    let [container] = ui.under(|cx| {
        cx.build(picked());
    })[..] else {
        panic!("a container");
    };
    let old = only_child(&ui, container);

    ui.world.pick(1);
    ui.update(Duration::ZERO, false);

    assert!(!ui.world.is_alive(old));
    assert_eq!(texts(&ui, container), ["1"]);
}

#[test]
fn a_keyed_view_keeps_its_place_among_siblings() {
    let mut ui = Ui::new(Warm);
    let [_, container, _] = ui.under(|cx| {
        cx.build(text("before"));
        cx.build(picked());
        cx.build(text("after"));
    })[..] else {
        panic!("three nodes");
    };

    ui.world.pick(1);
    ui.update(Duration::ZERO, false);

    let root = ui.world.node(container).parent.expect("a root");
    assert_eq!(ui.world.children(root)[1], container);
}

#[test]
fn a_keyed_view_does_not_rebuild_for_an_equal_key() {
    let mut ui = Ui::new(Warm);
    let [container] = ui.under(|cx| {
        cx.build(picked());
    })[..] else {
        panic!("a container");
    };
    let child = only_child(&ui, container);

    // The check fires on any change of the world, the key stays.
    ui.world.set(9);
    ui.update(Duration::ZERO, false);

    assert_eq!(only_child(&ui, container), child);
    assert!(ui.world.gone.is_empty());
}

#[test]
fn a_set_rule_outside_a_keyed_view_applies_to_what_it_rebuilds() {
    let mut ui = Ui::new(Warm);
    let [container] = ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.show::<Text>(|t, _| t.size(21.0));
        cx.build(picked());
    })[..] else {
        panic!("a container");
    };
    assert_eq!(ui.world.node(only_child(&ui, container)).size, 21.0);

    ui.world.pick(1);
    ui.update(Duration::ZERO, false);

    let child = ui.world.node(only_child(&ui, container));
    assert_eq!(child.size, 21.0, "show rules are captured too");
    assert_eq!(child.text, "1");
}

#[test]
fn a_scoped_rule_of_an_enclosing_composite_applies_to_a_rebuild() {
    let mut ui = Ui::new(Warm);
    let [panel, loose] = ui.under(|cx| {
        cx.build(Panel(picked().boxed()));
        cx.build(text("loose"));
    })[..] else {
        panic!("a panel and a text");
    };
    let container = only_child(&ui, panel);

    ui.world.pick(1);
    ui.update(Duration::ZERO, false);

    assert_eq!(ui.world.node(only_child(&ui, container)).size, 30.0);
    assert_eq!(ui.world.node(loose).size, 14.0, "outside the panel");
}

#[test]
fn rules_set_during_a_rebuild_are_scoped_to_it() {
    let mut ui = Ui::new(Warm);
    let [container, after] = ui.under(|cx| {
        cx.build(keyed(watch(|world: &World| world.pick), |&pick| {
            AnyView::new(move |cx: &mut Cx<'_, Fake, Warm>| {
                cx.set::<Text>(|t, _| t.size(40.0));
                cx.build(text(pick.to_string()))
            })
        }));
        cx.build(text("after"));
    })[..] else {
        panic!("a container and a text");
    };

    ui.world.pick(1);
    ui.update(Duration::ZERO, false);

    assert_eq!(ui.world.node(only_child(&ui, container)).size, 40.0);
    assert_eq!(ui.world.node(after).size, 14.0);
}

#[test]
fn a_scope_that_ended_leaves_no_rules_behind() {
    let mut ui = Ui::new(Warm);
    ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.scope(|cx| {
            cx.set::<Text>(|t, _| t.size(30.0));
            cx.build(text("x"));
        });
    });

    assert!(ui.mounted.rules().is_empty());
}

#[test]
fn rebuilding_a_keyed_view_does_not_grow_the_rule_arena() {
    let mut ui = Ui::new(Warm);
    let [container] = ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.build(keyed(watch(|world: &World| world.pick), |&pick| {
            AnyView::new(move |cx: &mut Cx<'_, Fake, Warm>| {
                cx.set::<Text>(|t, _| t.size(40.0));
                cx.show::<Text>(|t, _| t.size(41.0));
                cx.scope(|cx| {
                    cx.set_field(Text::cursor().size(), 42.0);
                    cx.build(text(pick.to_string()))
                })
            })
        }));
    })[..] else {
        panic!("a container");
    };
    let built = ui.mounted.rules().len();
    assert_eq!(built, 1, "only the rule outside is kept");

    for pick in 1..=100 {
        ui.world.pick(pick);
        ui.update(Duration::ZERO, false);
        assert_eq!(ui.mounted.rules().len(), built);
    }
    assert_eq!(texts(&ui, container), ["100"]);

    ui.mounted.unmount(container);
    assert!(ui.mounted.rules().is_empty(), "released with its view");
    assert_eq!(ui.mounted.structure_len(), 0);
}

#[test]
fn a_nested_keyed_view_rebuilds_on_its_own_and_with_its_outer() {
    let mut ui = Ui::new(Warm);
    let [outer] = ui.under(|cx| {
        cx.build(keyed(watch(|world: &World| world.pick), |&pick| {
            keyed(watch(|world: &World| world.count), move |&count| {
                text(format!("{pick}:{count}")).boxed()
            })
            .boxed()
        }));
    })[..] else {
        panic!("a container");
    };
    let inner = only_child(&ui, outer);
    assert_eq!(texts(&ui, inner), ["0:0"]);
    assert_eq!(ui.mounted.structure_len(), 2);

    ui.world.set(1);
    ui.update(Duration::ZERO, false);
    assert_eq!(texts(&ui, inner), ["0:1"]);

    // Both change at once: the outer drops the inner before it runs.
    ui.world.set(2);
    ui.world.pick(5);
    ui.update(Duration::ZERO, false);
    assert!(!ui.world.is_alive(inner));
    let inner = only_child(&ui, outer);
    assert_eq!(texts(&ui, inner), ["5:2"]);
    assert_eq!(ui.mounted.structure_len(), 2);

    ui.world.set(3);
    ui.update(Duration::ZERO, false);
    assert_eq!(texts(&ui, inner), ["5:3"]);
}

/// A text per number in the world's list, matched by the number.
fn listed() -> Each<Fake, Warm, u32, u32> {
    each(
        watch(|world: &World| world.list.clone()),
        |&id| id,
        |&id| text(id.to_string()).boxed(),
    )
}

/// An `each` over `list` under a root, and its container.
fn list_ui(list: &[u32]) -> (Ui<Warm>, usize) {
    let mut ui = Ui::new(Warm);
    ui.world.show(list);
    let [container] = ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.build(listed());
    })[..] else {
        panic!("a container");
    };
    (ui, container)
}

#[test]
fn an_each_view_builds_one_node_per_item() {
    let (ui, container) = list_ui(&[1, 2]);

    assert_eq!(texts(&ui, container), ["1", "2"]);
}

#[test]
fn an_each_view_adds_new_items() {
    let (mut ui, container) = list_ui(&[1, 2]);

    ui.world.show(&[1, 2, 3]);
    ui.update(Duration::ZERO, false);

    assert_eq!(texts(&ui, container), ["1", "2", "3"]);
}

#[test]
fn an_each_view_removes_items_whose_key_left() {
    let (mut ui, container) = list_ui(&[1, 2, 3]);
    let second = ui.world.children(container)[1];

    ui.world.show(&[1, 3]);
    ui.update(Duration::ZERO, false);

    assert_eq!(texts(&ui, container), ["1", "3"]);
    assert!(!ui.world.is_alive(second));
}

#[test]
fn an_each_view_reorders_its_children() {
    let (mut ui, container) = list_ui(&[1, 2, 3]);

    ui.world.show(&[3, 1, 2]);
    ui.update(Duration::ZERO, false);
    assert_eq!(texts(&ui, container), ["3", "1", "2"]);

    ui.world.show(&[2, 1, 3, 4]);
    ui.update(Duration::ZERO, false);
    assert_eq!(texts(&ui, container), ["2", "1", "3", "4"]);
}

#[test]
fn an_each_view_keeps_the_node_of_a_kept_key() {
    let (mut ui, container) = list_ui(&[1, 2, 3]);
    let [one, _, three] = ui.world.children(container)[..] else {
        panic!("three rows");
    };

    ui.world.show(&[3, 4, 1]);
    ui.update(Duration::ZERO, false);

    let [now_three, _, now_one] = ui.world.children(container)[..]
    else {
        panic!("three rows");
    };
    assert_eq!((now_one, now_three), (one, three));
}

#[test]
fn an_each_view_builds_new_items_under_the_captured_rules() {
    let (mut ui, container) = list_ui(&[1]);

    ui.world.show(&[1, 2]);
    ui.update(Duration::ZERO, false);

    let new = ui.world.children(container)[1];
    assert_eq!(ui.world.node(new).size, 20.0);
}

#[test]
fn an_each_view_does_not_grow_the_rule_arena() {
    let (mut ui, container) = list_ui(&[1]);
    let built = ui.mounted.rules().len();

    for round in 0..100 {
        ui.world.show(&[round, round + 1]);
        ui.update(Duration::ZERO, false);
    }

    assert_eq!(ui.mounted.rules().len(), built);
    ui.mounted.unmount(container);
    assert!(ui.mounted.rules().is_empty());
}

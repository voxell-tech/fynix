//! State rules, root and default rules, and transition rules against
//! the fake backend.

use core::time::Duration;
use std::string::ToString;

use super::{Fake, Lit, Text, Ui, Warm, World, text, watch};
use crate::{
    AnyView, Cx, Motion, Prop, ScopedExt, View, ViewExt, keyed,
};

impl Ui<Warm> {
    /// Lights `node` or puts it out, reports it as a backend would,
    /// and runs an update.
    fn light(&mut self, node: usize, lit: bool) {
        self.world.light(node, lit);
        self.mounted.mark_dirty(node);
        self.update(Duration::ZERO, false);
    }

    fn size(&self, node: usize) -> f32 {
        self.world.node(node).size
    }
}

/// A root node holding `children`, its rules waiting on [`Lit`].
fn lit_root(
    rules: impl FnOnce(&mut Cx<'_, Fake, Warm>) + 'static,
    children: impl FnOnce(&mut Cx<'_, Fake, Warm>) + 'static,
) -> impl View<Fake, Warm> {
    AnyView::new(|cx: &mut Cx<'_, Fake, Warm>| {
        let root = cx.spawn();
        cx.under(root, children);
        root
    })
    .when_in::<Lit, _>(rules)
}

fn big(cx: &mut Cx<'_, Fake, Warm>) {
    cx.set::<Text>(|text, _| text.size(30.0));
}

#[test]
fn a_state_rule_on_an_element_beats_its_call_site_while_it_holds() {
    let mut ui = Ui::new(Warm);
    let node = ui.build(text("x").size(9.0).when_in::<Lit, _>(big));
    assert_eq!(ui.size(node), 9.0);

    ui.light(node, true);
    assert_eq!(ui.size(node), 30.0);

    ui.light(node, false);
    assert_eq!(ui.size(node), 9.0);
}

#[test]
fn of_two_state_rules_on_one_element_the_later_written_wins() {
    let mut ui = Ui::new(Warm);
    let node =
        ui.build(text("x").when_in::<Lit, _>(big).when_in::<Lit, _>(
            |cx: &mut Cx<'_, Fake, Warm>| {
                cx.set::<Text>(|text, _| text.size(40.0));
            },
        ));

    ui.light(node, true);

    assert_eq!(ui.size(node), 40.0);
}

#[test]
fn a_state_rule_on_an_ancestor_fills_only_what_the_call_site_left_unset()
 {
    let mut ui = Ui::new(Warm);
    let root = ui.build(lit_root(big, |cx| {
        cx.build(text("ruled"));
        cx.build(text("explicit").size(9.0));
    }));
    let [ruled, explicit] = ui.world.children(root)[..] else {
        panic!("two texts");
    };

    ui.light(root, true);

    assert_eq!(ui.size(ruled), 30.0, "read on the root, not its own");
    assert_eq!(ui.size(explicit), 9.0, "an explicit value opts out");
}

#[test]
fn a_state_rule_beats_a_set_rule_set_inside_it() {
    let mut ui = Ui::new(Warm);
    let root = ui.build(lit_root(big, |cx| {
        cx.set::<Text>(|text, _| text.size(20.0));
        cx.build(text("x"));
    }));
    let child = ui.world.children(root)[0];
    assert_eq!(ui.size(child), 20.0);

    ui.light(root, true);

    assert_eq!(ui.size(child), 30.0);
}

#[test]
fn the_node_a_state_rule_is_read_on_is_watched() {
    let mut ui = Ui::new(Warm);
    let root = ui.build(lit_root(big, |cx| {
        cx.build(text("a"));
        cx.build(text("b"));
    }));

    assert_eq!(ui.world.watched, [root]);
}

#[test]
fn a_rebuilt_element_keeps_the_state_rule_around_it() {
    let mut ui = Ui::new(Warm);
    let root = ui.build(lit_root(big, |cx| {
        cx.build(keyed(watch(|world: &World| world.pick), |&pick| {
            text(pick.to_string()).boxed()
        }));
    }));
    let container = ui.world.children(root)[0];

    ui.world.pick = 1;
    ui.world.version += 1;
    ui.update(Duration::ZERO, false);
    ui.light(root, true);

    let child = ui.world.children(container)[0];
    assert_eq!(ui.world.node(child).text, "1");
    assert_eq!(ui.size(child), 30.0);
}

#[test]
fn a_root_rule_reaches_the_root_alone() {
    let mut ui = Ui::new(Warm);
    let lone = ui.build(
        text("lone")
            .rules(|cx: &mut Cx<'_, Fake, Warm>| cx.root(big)),
    );
    let root = ui.build(
        AnyView::new(|cx: &mut Cx<'_, Fake, Warm>| {
            let root = cx.build(text("root"));
            cx.under(root, |cx| cx.build(text("child")));
            root
        })
        .rules(|cx: &mut Cx<'_, Fake, Warm>| cx.root(big)),
    );
    let child = ui.world.children(root)[0];

    assert_eq!(ui.size(lone), 30.0);
    assert_eq!(ui.size(root), 30.0);
    assert_eq!(ui.size(child), 14.0);
}

#[test]
fn a_default_is_weaker_than_an_outer_rule() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.set::<Text>(|text, _| text.size(20.0));
        cx.scope(|cx| {
            cx.defaults(big);
            cx.build(text("x"));
        });
        cx.scope(|cx| {
            cx.defaults(|cx| {
                cx.set::<Text>(|mut text, _| {
                    text.text = "d".into();
                    text
                });
            });
            cx.build(text(Prop::Unset));
        });
    });

    assert_eq!(ui.size(nodes[0]), 20.0);
    assert_eq!(ui.world.node(nodes[1]).text, "d", "and still fills");
}

#[test]
fn a_transition_rule_moves_an_element_over_the_curve() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.transition(Motion::Interact);
        cx.build(
            text("x").size(watch(|world: &World| world.count as f32)),
        );
    });

    ui.world.set(10);
    ui.update(Duration::ZERO, false);
    ui.update(Duration::from_millis(500), false);

    assert_eq!(ui.size(nodes[0]), 5.0);
}

#[test]
fn a_state_rule_moves_over_a_transition_around_it() {
    let mut ui = Ui::new(Warm);
    let node = ui.build(
        text("x")
            .size(10.0)
            .when_in::<Lit, _>(big)
            .transition(Motion::Interact),
    );

    ui.light(node, true);
    ui.update(Duration::from_millis(500), false);

    assert_eq!(ui.size(node), 20.0);
}

#[test]
fn a_bound_prop_in_a_state_rule_is_followed() {
    let mut ui = Ui::new(Warm);
    let node = ui.build(text("x").when_in::<Lit, _>(
        |cx: &mut Cx<'_, Fake, Warm>| {
            cx.set::<Text>(|text, _| {
                text.size(watch(|world: &World| world.count as f32))
            });
        },
    ));
    ui.light(node, true);

    ui.world.set(40);
    ui.update(Duration::ZERO, false);

    assert_eq!(ui.size(node), 40.0);
}

#[test]
fn nothing_waits_on_a_state_without_a_state_rule() {
    let mut ui = Ui::new(Warm);
    ui.build(text("x"));

    assert!(ui.mounted.is_empty());
    assert!(ui.world.watched.is_empty());
}

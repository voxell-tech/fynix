//! Show rules against the fake backend.

use std::string::{String, ToString};
use std::vec::Vec;
use std::{panic, vec};

use super::{Fake, Text, Ui, Warm, text};
use crate::{AnyView, Cx, Matched, Prop, ViewExt};

/// A show rule putting the text it matches in a bare node named
/// `name`.
fn wrap(
    name: &'static str,
) -> impl Fn(Matched<Text>, &Warm) -> AnyView<Fake, Warm>
+ Send
+ Sync
+ 'static {
    move |it, _| {
        AnyView::new(move |cx: &mut Cx<'_, Fake, Warm>| {
            let node = cx.spawn();
            cx.world.nodes[node]
                .as_mut()
                .expect("a live node")
                .text = name.to_string();
            cx.under(node, |cx| cx.build(it));
            node
        })
    }
}

/// The texts of `node` and of its first child, its first child's,
/// and so on down.
fn chain(ui: &Ui<Warm>, mut node: usize) -> Vec<String> {
    let mut texts = vec![ui.world.node(node).text.clone()];
    while let Some(&child) = ui.world.node(node).children.first() {
        texts.push(ui.world.node(child).text.clone());
        node = child;
    }
    texts
}

#[test]
fn a_show_rule_can_replace_an_element_with_another_view() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.set::<Text>(|text, _| text.size(20.0));
        cx.show::<Text>(|original, _| {
            let reaction = match &original.size {
                Prop::Value(9.0) => "inline",
                _ => "not inline",
            };
            AnyView::new(move |cx| {
                let wrapper = cx.spawn();
                cx.under(wrapper, |cx| {
                    cx.build(original);
                    cx.build(text(reaction));
                });
                wrapper
            })
        });
        cx.build(text("original").size(9.0));
    });

    let children = ui.world.children(nodes[0]);
    assert_eq!(children.len(), 2);
    assert_eq!(ui.world.node(children[0]).text, "original");
    assert_eq!(ui.world.node(children[0]).size, 9.0);
    assert_eq!(ui.world.node(children[1]).text, "inline");
}

#[test]
fn the_latest_show_rule_is_outermost_and_each_runs_once() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.show::<Text>(wrap("older"));
        cx.show::<Text>(wrap("newer"));
        cx.build(text("x"));
    });

    assert_eq!(chain(&ui, nodes[0]), ["newer", "older", "x"]);
}

#[test]
fn a_new_text_in_a_replacement_is_replaced_too() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.show::<Text>(|it, _| {
            let label =
                matches!(&it.text, Prop::Value(text) if text == "label");
            AnyView::new(move |cx: &mut Cx<'_, Fake, Warm>| {
                let node = cx.spawn();
                cx.under(node, |cx| {
                    cx.build(it);
                    if !label {
                        cx.build(text("label"));
                    }
                });
                node
            })
        });
        cx.build(text("x"));
    });

    let [shown, label] = ui.world.children(nodes[0])[..] else {
        panic!("the text and its label");
    };
    assert_eq!(ui.world.node(shown).text, "x");
    assert_eq!(chain(&ui, label), ["", "label"]);
}

#[test]
#[should_panic(expected = "nested 64 deep")]
fn a_rule_that_always_builds_its_own_kind_stops_at_a_depth() {
    let mut ui = Ui::new(Warm);
    ui.under(|cx| {
        cx.show::<Text>(|_, _| text("again").boxed());
        cx.build(text("x"));
    });
}

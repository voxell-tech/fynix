//! Show rules against the fake backend.

use std::string::{String, ToString};
use std::vec::Vec;
use std::{panic, vec};

use super::{Fake, Frame, Lit, Text, Ui, Warm, text, wrap};
use crate::{AnyView, Cx, Prop, Styled, ViewExt};

impl Ui<Warm> {
    /// The texts of `node` and of its first child, its first
    /// child's, and so on down.
    fn chain(&self, mut node: usize) -> Vec<String> {
        let mut texts = vec![self.world.node(node).text.clone()];
        while let Some(&child) =
            self.world.node(node).children.first()
        {
            texts.push(self.world.node(child).text.clone());
            node = child;
        }
        texts
    }
}

#[test]
fn a_show_rule_is_handed_the_element_with_its_set_rules_applied() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.set::<Text>(|text, _| text.size(20.0).text("ruled"));
        cx.show::<Text>(|it, _| {
            let size = match it.size {
                Prop::Value(size) => size,
                _ => 0.0,
            };
            let call = it.call_mask();
            AnyView::new(move |cx: &mut Cx<'_, Fake, Warm>| {
                let node = cx.spawn();
                let shown = cx.world.nodes[node]
                    .as_mut()
                    .expect("a live node");
                shown.size = size;
                shown.text = call.to_string();
                cx.under(node, |cx| cx.build(it));
                node
            })
        });
        cx.build(text(Prop::Unset).size(9.0));
        cx.build(text(Prop::Unset));
    });

    let [inline, ruled] = nodes[..] else {
        panic!("two replacements");
    };
    assert_eq!(ui.world.node(inline).size, 9.0);
    assert_eq!(ui.world.node(inline).text, "2", "the size bit alone");
    assert_eq!(
        ui.world.node(ruled).size,
        20.0,
        "the rule saw the set"
    );
    assert_eq!(ui.world.node(ruled).text, "0");
    let [text] = ui.world.children(ruled)[..] else {
        panic!("the text");
    };
    assert_eq!(ui.world.node(text).text, "ruled");
}

#[test]
fn the_latest_show_rule_is_outermost_and_each_runs_once() {
    let mut ui = Ui::new(Warm);
    let nodes = ui.under(|cx| {
        cx.show::<Text>(wrap("older"));
        cx.show::<Text>(wrap("newer"));
        cx.build(text("x"));
    });

    assert_eq!(ui.chain(nodes[0]), ["newer", "older", "x"]);
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
    assert_eq!(ui.chain(label), ["", "label"]);
}

#[test]
fn a_filtered_rule_leaves_what_it_does_not_take_to_older_rules() {
    let mut ui = Ui::new(Warm);
    let [ruled, inline] = ui.under(|cx| {
        cx.set::<Text>(|t, _| t.size(20.0));
        cx.show::<Text>(wrap("any"));
        cx.show_where::<Text>(
            |it| matches!(it.size, Prop::Value(size) if size > 16.0),
            wrap("big"),
        );
        cx.build(text("ruled"));
        cx.build(text("inline").size(9.0));
    })[..] else {
        panic!("two replacements");
    };

    assert_eq!(ui.chain(ruled), ["big", "any", "ruled"]);
    assert_eq!(ui.chain(inline), ["any", "inline"]);
}

#[test]
fn a_rule_set_in_a_replacement_ends_with_it() {
    let mut ui = Ui::new(Warm);
    let [replaced, after] = ui.under(|cx| {
        cx.show_where::<Text>(
            |it| matches!(&it.text, Prop::Value(text) if text == "x"),
            |it, _| {
                AnyView::new(move |cx: &mut Cx<'_, Fake, Warm>| {
                    cx.set::<Text>(|t, _| t.size(40.0));
                    let node = cx.spawn();
                    cx.under(node, |cx| {
                        cx.build(it);
                        cx.build(text("inside"));
                    });
                    node
                })
            },
        );
        cx.build(text("x"));
        cx.build(text("after"));
    })[..] else {
        panic!("a replacement and a text");
    };

    let [x, inside] = ui.world.children(replaced)[..] else {
        panic!("two texts");
    };
    assert_eq!(ui.world.node(x).size, 14.0, "resolved before");
    assert_eq!(ui.world.node(inside).size, 40.0);
    assert_eq!(ui.world.node(after).size, 14.0);
}

#[test]
fn a_show_set_rule_holds_within_every_element_of_its_kind() {
    let mut ui = Ui::new(Warm);
    let [framed, after] = ui.under(|cx| {
        cx.show_set::<Frame, Text>(|t, _| t.size(30.0));
        cx.show_set::<Frame, Frame>(|frame, _| frame.size(5.0));
        let frame = cx.build(Frame::unset());
        cx.under(frame, |cx| {
            cx.build(text("ruled"));
            cx.build(text("inline").size(9.0));
        });
        cx.build(text("after"));
    })[..] else {
        panic!("a frame and a text");
    };

    assert_eq!(ui.world.node(framed).size, 5.0, "the frame itself");
    let [ruled, inline] = ui.world.children(framed)[..] else {
        panic!("two texts");
    };
    assert_eq!(ui.world.node(ruled).size, 30.0);
    assert_eq!(ui.world.node(inline).size, 9.0, "call site wins");
    assert_eq!(ui.world.node(after).size, 14.0, "outside the frame");
}

#[test]
fn a_rule_set_under_an_element_with_show_set_rules_stays_in_force() {
    let mut ui = Ui::new(Warm);
    let [_, after] = ui.under(|cx| {
        cx.show_set::<Frame, Text>(|t, _| t.size(30.0));
        let frame = cx.build(Frame::unset());
        cx.under(frame, |cx| cx.set::<Text>(|t, _| t.size(20.0)));
        cx.build(text("after"));
    })[..] else {
        panic!("a frame and a text");
    };

    assert_eq!(ui.world.node(after).size, 20.0, "as under any node");
}

#[test]
#[should_panic(expected = "cannot wait on a state")]
fn a_show_rule_cannot_wait_on_a_state() {
    let mut ui = Ui::new(Warm);
    ui.under(|cx| cx.when::<Lit>(|cx| cx.show::<Text>(wrap("lit"))));
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

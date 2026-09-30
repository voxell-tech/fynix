//! Views dropped by `keyed` and `each` animating out, against the
//! fake backend.

use core::time::Duration;
use std::string::{String, ToString};
use std::vec::Vec;

use super::{Fake, Ui, Warm, World, text, watch};
use crate::{Cx, Motion, ViewExt, each, keyed};

/// Warm's curve, one second long.
const CURVE: Duration = Duration::from_secs(1);

impl World {
    fn list(&mut self, list: &[u32]) {
        self.list = list.to_vec();
        self.version += 1;
    }
}

/// An `each` over `list` whose rows travel over Warm's curve when
/// `moving`, and its container.
fn rows(list: &[u32], moving: bool) -> (Ui<Warm>, usize) {
    let mut ui = Ui::new(Warm);
    ui.world.list(list);
    let [container] = ui.under(move |cx: &mut Cx<'_, Fake, Warm>| {
        if moving {
            cx.transition(Motion::Interact);
        }
        cx.build(each(
            watch(|world: &World| world.list.clone()),
            |&id| id,
            |&id| text(id.to_string()).boxed(),
        ));
    })[..] else {
        panic!("a container");
    };
    (ui, container)
}

fn texts(ui: &Ui<Warm>, container: usize) -> Vec<String> {
    ui.world
        .children(container)
        .into_iter()
        .map(|node| ui.world.node(node).text.clone())
        .collect()
}

#[test]
fn a_dropped_row_fades_then_collapses_then_goes() {
    let (mut ui, container) = rows(&[1, 2], true);
    let two = ui.world.children(container)[1];

    ui.world.list(&[1]);
    ui.update(Duration::ZERO, false);
    assert!(ui.world.node(two).leaving);
    assert_eq!(ui.world.node(two).collapsed, None, "fading first");

    ui.update(CURVE, false);
    assert_eq!(ui.world.node(two).collapsed, Some(0.0));

    ui.update(CURVE / 2, false);
    assert_eq!(ui.world.node(two).collapsed, Some(0.5));

    ui.update(CURVE / 2, false);
    assert!(!ui.world.is_alive(two));
    assert_eq!(texts(&ui, container), ["1"]);
}

#[test]
fn a_dropped_row_without_a_transition_goes_at_once() {
    let (mut ui, container) = rows(&[1, 2], false);
    let two = ui.world.children(container)[1];

    ui.world.list(&[1]);
    ui.update(Duration::ZERO, false);

    assert!(!ui.world.is_alive(two));
}

#[test]
fn a_leaving_row_keeps_its_place_among_the_rest() {
    let (mut ui, container) = rows(&[1, 2, 3], true);

    ui.world.list(&[1, 3]);
    ui.update(Duration::ZERO, false);
    assert_eq!(texts(&ui, container), ["1", "2", "3"]);

    ui.world.list(&[3, 1, 4]);
    ui.update(Duration::ZERO, false);
    assert_eq!(
        texts(&ui, container),
        ["3", "1", "2", "4"],
        "still after the row it followed"
    );

    ui.update(CURVE * 2, false);
    ui.update(Duration::ZERO, false);
    ui.world.list(&[4, 3, 1]);
    ui.update(Duration::ZERO, false);
    assert_eq!(
        texts(&ui, container),
        ["4", "3", "1"],
        "and gone after"
    );
}

#[test]
fn reduced_motion_drops_a_leaving_row_at_once() {
    let (mut ui, container) = rows(&[1, 2], true);
    let two = ui.world.children(container)[1];

    ui.world.list(&[1]);
    ui.update(Duration::ZERO, true);

    assert!(!ui.world.is_alive(two));
}

#[test]
fn a_keyed_view_animates_its_old_view_out_beside_the_new() {
    let mut ui = Ui::new(Warm);
    let [container] = ui.under(|cx: &mut Cx<'_, Fake, Warm>| {
        cx.transition(Motion::Interact);
        cx.build(keyed(watch(|world: &World| world.pick), |&pick| {
            text(pick.to_string()).boxed()
        }));
    })[..] else {
        panic!("a container");
    };
    let old = ui.world.children(container)[0];

    ui.world.pick = 1;
    ui.world.version += 1;
    ui.update(Duration::ZERO, false);

    assert!(ui.world.node(old).leaving);
    assert_eq!(texts(&ui, container), ["0", "1"]);

    ui.update(CURVE, false);
    ui.update(CURVE, false);
    assert_eq!(texts(&ui, container), ["1"]);
}

#[test]
fn a_container_can_be_a_view_of_its_own() {
    let mut ui = Ui::new(Warm);
    ui.world.list(&[1, 2]);
    let [container] = ui.under(|cx: &mut Cx<'_, Fake, Warm>| {
        cx.build(
            each(
                watch(|world: &World| world.list.clone()),
                |&id| id,
                |&id| text(id.to_string()).boxed(),
            )
            .within(text("list")),
        );
    })[..] else {
        panic!("a container");
    };

    assert_eq!(ui.world.node(container).text, "list");
    assert_eq!(texts(&ui, container), ["1", "2"]);
}

#[test]
fn a_leaving_row_despawned_with_its_container_is_forgotten() {
    let (mut ui, container) = rows(&[1, 2], true);
    let two = ui.world.children(container)[1];
    ui.world.list(&[1]);
    ui.update(Duration::ZERO, false);

    <Fake as crate::Backend>::despawn(&mut ui.world, container);
    ui.update(Duration::ZERO, false);

    assert!(!ui.mounted.is_leaving(two));
}

//! `keyed` and `each` in a headless app, and the update order they
//! need.

use bevy::app::App;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;

use super::{app, children, text};
use crate::mounted::Mounts;
use crate::tokens::{SpacingTokens, TextTokens, Tone};
use crate::views::{column, label, row};
use crate::{
    AnyView, Bevy, Unmounted, ViewExt, component, each, keyed, mount,
    resource,
};

struct Plain;

impl TextTokens for Plain {
    fn tone(&self, _: Tone) -> Color {
        Color::WHITE
    }

    fn body_size(&self) -> f32 {
        14.0
    }

    fn small_size(&self) -> f32 {
        11.0
    }
}

impl SpacingTokens for Plain {
    fn gap(&self) -> f32 {
        4.0
    }

    fn row(&self) -> f32 {
        20.0
    }

    fn radius(&self) -> f32 {
        2.0
    }
}

#[derive(Resource, Clone, Copy, PartialEq)]
enum Screen {
    Home,
    Settings,
}

#[derive(Resource)]
struct Count(u32);

fn count_label() -> crate::views::Label {
    label(resource::<Count, _>(|count| count.0.to_string()))
}

fn screens() -> AnyView<Bevy, Plain> {
    keyed(resource::<Screen, _>(|screen| *screen), |screen| {
        match screen {
            Screen::Home => {
                column((label("home"), count_label(), count_label()))
                    .boxed()
            }
            Screen::Settings => {
                column((label("settings"), count_label())).boxed()
            }
        }
    })
    .boxed()
}

fn set_screen(app: &mut App, screen: Screen) {
    *app.world_mut().resource_mut::<Screen>() = screen;
}

fn mounts(app: &App) -> usize {
    app.world().resource::<Mounts<Plain>>().len()
}

fn setup_screens() -> (App, bevy::ecs::entity::Entity) {
    let mut app = app(Plain);
    app.insert_resource(Screen::Home).insert_resource(Count(1));
    let switch = mount::<Plain>(app.world_mut(), screens());
    (app, switch)
}

#[test]
fn a_screen_switch_replaces_the_subtree() {
    let (mut app, switch) = setup_screens();
    let [home] = children(&app, switch)[..] else {
        panic!("one screen");
    };
    assert_eq!(mounts(&app), 2);

    set_screen(&mut app, Screen::Settings);
    app.update();

    let [settings] = children(&app, switch)[..] else {
        panic!("one screen");
    };
    assert_ne!(home, settings);
    assert!(app.world().get_entity(home).is_err(), "old one is gone");
    assert_eq!(text(&app, children(&app, settings)[0]), "settings");
    assert_eq!(mounts(&app), 1, "the old screen's labels left");
    assert!(app.world().resource::<Unmounted>().0.is_empty());
}

#[test]
fn the_new_screen_is_bound_and_keeps_updating() {
    let (mut app, switch) = setup_screens();

    set_screen(&mut app, Screen::Settings);
    app.update();
    app.world_mut().resource_mut::<Count>().0 = 7;
    app.update();

    let [settings] = children(&app, switch)[..] else {
        panic!("one screen");
    };
    assert_eq!(text(&app, children(&app, settings)[1]), "7");
}

#[test]
fn rapid_switches_never_touch_a_despawned_node() {
    let (mut app, switch) = setup_screens();

    // The bound labels of both screens change in the same frame as
    // the switch that despawns them.
    for round in 0..8u32 {
        let screen = if round % 2 == 0 {
            Screen::Settings
        } else {
            Screen::Home
        };
        set_screen(&mut app, screen);
        app.world_mut().resource_mut::<Count>().0 = round;
        app.update();
    }
    for _ in 0..3 {
        set_screen(&mut app, Screen::Settings);
        app.update();
        set_screen(&mut app, Screen::Home);
        app.update();
    }

    let [home] = children(&app, switch)[..] else {
        panic!("one screen");
    };
    assert_eq!(text(&app, children(&app, home)[0]), "home");
    assert_eq!(mounts(&app), 2);
    assert_eq!(
        app.world().resource::<Mounts<Plain>>().structure_len(),
        1
    );
}

#[test]
fn a_switch_inside_a_switch_is_dropped_with_its_outer() {
    #[derive(Resource)]
    struct Inner(u32);

    let mut app = app(Plain);
    app.insert_resource(Screen::Home).insert_resource(Inner(0));
    mount::<Plain>(
        app.world_mut(),
        keyed(resource::<Screen, _>(|screen| *screen), |_| {
            keyed(resource::<Inner, _>(|inner| inner.0), |inner| {
                label(inner.to_string()).boxed()
            })
            .boxed()
        }),
    );
    assert_eq!(
        app.world().resource::<Mounts<Plain>>().structure_len(),
        2
    );

    // The outer rebuilds, and drops the inner it built before.
    set_screen(&mut app, Screen::Settings);
    app.world_mut().resource_mut::<Inner>().0 = 1;
    app.update();
    app.world_mut().resource_mut::<Inner>().0 = 2;
    app.update();

    assert_eq!(
        app.world().resource::<Mounts<Plain>>().structure_len(),
        2
    );
}

#[derive(Resource)]
struct SelectedEntity(Option<Entity>);

/// The names of the selected entity's components.
#[derive(Resource)]
struct ComponentNames(Vec<&'static str>);

#[derive(Component)]
struct Health(u32);

#[derive(Component)]
struct Mana(u32);

#[derive(Component)]
struct Armor(u32);

/// A row of a component's name and its value, bound to `entity`.
fn component_row(
    entity: Entity,
    name: &'static str,
) -> AnyView<Bevy, Plain> {
    let value = match name {
        "Health" => component::<Health, _>(entity, |health| {
            health.map_or("-".to_string(), |h| h.0.to_string())
        }),
        "Mana" => component::<Mana, _>(entity, |mana| {
            mana.map_or("-".to_string(), |m| m.0.to_string())
        }),
        _ => component::<Armor, _>(entity, |armor| {
            armor.map_or("-".to_string(), |a| a.0.to_string())
        }),
    };
    row((label(name), label(value))).boxed()
}

fn inspector() -> AnyView<Bevy, Plain> {
    keyed(
        resource::<SelectedEntity, _>(|selected| selected.0),
        |&selected| match selected {
            None => label("nothing selected").boxed(),
            Some(entity) => each(
                resource::<ComponentNames, _>(|names| {
                    names.0.clone()
                }),
                |name| *name,
                move |&name| component_row(entity, name),
            )
            .boxed(),
        },
    )
    .boxed()
}

struct Inspected {
    app: App,
    /// The keyed container.
    root: Entity,
    first: Entity,
    second: Entity,
}

fn inspected() -> Inspected {
    let mut app = app(Plain);
    let first =
        app.world_mut().spawn((Health(10), Mana(5), Armor(2))).id();
    let second = app.world_mut().spawn(Health(99)).id();
    app.insert_resource(SelectedEntity(Some(first)))
        .insert_resource(ComponentNames(vec!["Health", "Mana"]));
    let root = mount::<Plain>(app.world_mut(), inspector());
    Inspected {
        app,
        root,
        first,
        second,
    }
}

impl Inspected {
    /// The rows of the list under the keyed container.
    fn rows(&self) -> Vec<Entity> {
        let [list] = children(&self.app, self.root)[..] else {
            panic!("one list");
        };
        children(&self.app, list)
    }

    /// The name and value shown in each row.
    fn shown(&self) -> Vec<(String, String)> {
        self.rows()
            .into_iter()
            .map(|row| {
                let [name, value] = children(&self.app, row)[..]
                else {
                    panic!("a name and a value");
                };
                (text(&self.app, name), text(&self.app, value))
            })
            .collect()
    }
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

#[test]
fn the_inspector_shows_one_row_per_component() {
    let inspected = inspected();

    assert_eq!(
        inspected.shown(),
        [pair("Health", "10"), pair("Mana", "5")]
    );
}

#[test]
fn a_changed_component_updates_its_label_and_rebuilds_no_row() {
    let mut inspected = inspected();
    let rows = inspected.rows();

    inspected
        .app
        .world_mut()
        .entity_mut(inspected.first)
        .insert(Health(11));
    inspected.app.update();

    assert_eq!(inspected.rows(), rows, "same row entities");
    assert_eq!(
        inspected.shown(),
        [pair("Health", "11"), pair("Mana", "5")]
    );
}

#[test]
fn an_added_component_name_adds_one_row_and_keeps_the_others() {
    let mut inspected = inspected();
    let rows = inspected.rows();

    inspected
        .app
        .world_mut()
        .resource_mut::<ComponentNames>()
        .0
        .push("Armor");
    inspected.app.update();

    let now = inspected.rows();
    assert_eq!(now.len(), 3);
    assert_eq!(now[..2], rows[..]);
    assert_eq!(inspected.shown()[2], pair("Armor", "2"));
}

#[test]
fn a_removed_component_name_despawns_only_its_row() {
    let mut inspected = inspected();
    let rows = inspected.rows();
    let before = mounts(&inspected.app);

    inspected
        .app
        .world_mut()
        .resource_mut::<ComponentNames>()
        .0
        .remove(0);
    inspected.app.update();

    assert_eq!(inspected.rows(), [rows[1]]);
    assert!(inspected.app.world().get_entity(rows[0]).is_err());
    assert_eq!(mounts(&inspected.app), before - 1);
}

#[test]
fn a_new_selection_rebuilds_the_inspector() {
    let mut inspected = inspected();
    let rows = inspected.rows();

    inspected.app.world_mut().resource_mut::<SelectedEntity>().0 =
        Some(inspected.second);
    inspected.app.update();
    assert!(inspected.rows().iter().all(|row| !rows.contains(row)));
    assert_eq!(
        inspected.shown(),
        [pair("Health", "99"), pair("Mana", "-")]
    );

    inspected.app.world_mut().resource_mut::<SelectedEntity>().0 =
        None;
    inspected.app.update();
    let [nothing] = children(&inspected.app, inspected.root)[..]
    else {
        panic!("one label");
    };
    assert_eq!(text(&inspected.app, nothing), "nothing selected");
    assert_eq!(mounts(&inspected.app), 0);
}

#[derive(Resource)]
struct Ids(Vec<u32>);

fn ids() -> AnyView<Bevy, Plain> {
    each(
        resource::<Ids, _>(|ids| ids.0.clone()),
        |id| *id,
        |id| label(id.to_string()).boxed(),
    )
    .boxed()
}

fn shown(app: &App, list: Entity) -> Vec<String> {
    children(app, list)
        .into_iter()
        .map(|node| text(app, node))
        .collect()
}

#[test]
fn an_each_view_keeps_the_entity_of_a_kept_key() {
    let mut app = app(Plain);
    app.insert_resource(Ids(vec![1, 2, 3]));
    let list = mount::<Plain>(app.world_mut(), ids());
    let [one, two, three] = children(&app, list)[..] else {
        panic!("three rows");
    };

    app.world_mut().resource_mut::<Ids>().0 = vec![3, 4, 1];
    app.update();

    let [now_three, four, now_one] = children(&app, list)[..] else {
        panic!("three rows");
    };
    assert_eq!((now_one, now_three), (one, three));
    assert_ne!(four, two);
    assert!(app.world().get_entity(two).is_err());
    assert_eq!(shown(&app, list), ["3", "4", "1"]);
}

#[test]
fn an_each_view_reorders_the_children_of_its_container() {
    let mut app = app(Plain);
    app.insert_resource(Ids(vec![1, 2, 3]));
    let list = mount::<Plain>(app.world_mut(), ids());
    let rows = children(&app, list);

    app.world_mut().resource_mut::<Ids>().0 = vec![3, 2, 1];
    app.update();

    let reversed = rows.iter().rev().copied().collect::<Vec<_>>();
    assert_eq!(children(&app, list), reversed);
    assert_eq!(shown(&app, list), ["3", "2", "1"]);
}

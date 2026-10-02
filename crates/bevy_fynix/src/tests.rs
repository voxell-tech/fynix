//! The crate's tests: the shared helpers in `support`, and a headless
//! app per test.

use core::sync::atomic::{AtomicUsize, Ordering};

use bevy::app::App;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::name::Name;
use bevy::ecs::relationship::RelationshipTarget;
use bevy::ecs::resource::Resource;
use bevy::text::TextColor;
use bevy::time::TimePlugin;
use bevy::ui::widget::Text;

mod layout;
mod structure;
mod support;

pub(crate) use support::{
    Plain, app, app_with, hover, key_down, keyboard, kids,
    pointer_press, press,
};

use crate::mounted::Mounts;
use crate::tokens::{TextTokens, Tone};
use crate::views::label;
use crate::{
    AnyView, Bevy, FynixPlugin, Theme, component, derived,
    every_frame, mount, resource,
};

/// One app's theme.
struct Warm;

impl TextTokens for Warm {
    fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Body => Color::WHITE,
            Tone::Dim | Tone::Faint => Color::srgb(0.5, 0.5, 0.5),
            Tone::Accent | Tone::OnAccent | Tone::Critical => {
                Color::srgb(1.0, 0.5, 0.0)
            }
        }
    }

    fn body_size(&self) -> f32 {
        14.0
    }

    fn small_size(&self) -> f32 {
        11.0
    }
}

fn themed_app<T: Send + Sync + 'static>(theme: T) -> App {
    let mut app = App::new();
    app.add_plugins((TimePlugin, FynixPlugin::<T>::default()))
        .insert_resource(Theme(theme));
    app
}

fn text(app: &App, node: Entity) -> String {
    app.world().get::<Text>(node).expect("a label").0.clone()
}

fn color(app: &App, node: Entity) -> Color {
    app.world().get::<TextColor>(node).expect("a label").0
}

/// The nodes built directly under `root`, in order.
fn children(app: &App, root: Entity) -> Vec<Entity> {
    app.world()
        .get::<Children>(root)
        .map(|children| children.iter().collect())
        .unwrap_or_default()
}

#[derive(Resource)]
struct Count(u32);

#[test]
fn a_bound_prop_follows_the_world() {
    let mut app = themed_app(Warm);
    app.insert_resource(Count(1));
    let bound = mount::<Warm>(
        app.world_mut(),
        label(resource::<Count, _>(|count| count.0.to_string())),
    );
    let fixed = mount::<Warm>(app.world_mut(), label("fixed"));
    assert_eq!(text(&app, bound), "1");

    app.world_mut().resource_mut::<Count>().0 = 2;
    app.update();

    assert_eq!(text(&app, bound), "2");
    assert_eq!(text(&app, fixed), "fixed");
    assert_eq!(
        app.world().resource::<Mounts<Warm>>().len(),
        1,
        "only what can change stays mounted"
    );
}

#[test]
fn a_despawned_view_is_dropped_after_one_update() {
    let mut app = themed_app(Warm);
    app.insert_resource(Count(1));
    let node = mount::<Warm>(
        app.world_mut(),
        label(resource::<Count, _>(|count| count.0.to_string())),
    );
    let kept = mount::<Warm>(
        app.world_mut(),
        label(resource::<Count, _>(|count| count.0.to_string())),
    );
    assert_eq!(app.world().resource::<Mounts<Warm>>().len(), 2);

    app.world_mut().despawn(node);
    app.update();

    assert_eq!(app.world().resource::<Mounts<Warm>>().len(), 1);
    assert!(app.world().get_entity(kept).is_ok());
}

#[test]
fn a_resource_signal_is_read_only_after_the_resource_changes() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut app = themed_app(Warm);
    app.insert_resource(Count(1));
    let node = mount::<Warm>(
        app.world_mut(),
        label(resource::<Count, _>(|count| {
            READS.fetch_add(1, Ordering::Relaxed);
            count.0.to_string()
        })),
    );
    // The first update re-reads once: nothing ran since the resource
    // was inserted, so the mount could not tell a later write apart.
    app.update();
    let settled = READS.load(Ordering::Relaxed);

    app.update();
    app.update();
    assert_eq!(READS.load(Ordering::Relaxed), settled);

    app.world_mut().resource_mut::<Count>().0 = 2;
    app.update();
    assert_eq!(READS.load(Ordering::Relaxed), settled + 1);
    assert_eq!(text(&app, node), "2");

    app.update();
    assert_eq!(READS.load(Ordering::Relaxed), settled + 1);
}

#[test]
fn a_change_made_right_after_mounting_is_not_missed() {
    let mut app = themed_app(Warm);
    app.insert_resource(Count(1));
    let node = mount::<Warm>(
        app.world_mut(),
        label(resource::<Count, _>(|count| count.0.to_string())),
    );

    // No system has run since the resource was inserted, so the write
    // lands on the tick the mount saw.
    app.world_mut().resource_mut::<Count>().0 = 2;
    app.update();

    assert_eq!(text(&app, node), "2");
}

#[test]
fn a_component_signal_follows_the_component() {
    let mut app = themed_app(Warm);
    let entity = app.world_mut().spawn(Name::new("Cube")).id();
    let node = mount::<Warm>(
        app.world_mut(),
        label(component::<Name, _>(entity, |name| {
            name.map_or("(none)".to_string(), ToString::to_string)
        })),
    );
    assert_eq!(text(&app, node), "Cube");

    app.world_mut()
        .entity_mut(entity)
        .insert(Name::new("Sphere"));
    app.update();
    assert_eq!(text(&app, node), "Sphere");

    app.world_mut().entity_mut(entity).remove::<Name>();
    app.update();
    assert_eq!(text(&app, node), "(none)");

    app.world_mut().entity_mut(entity).insert(Name::new("Cone"));
    app.update();
    assert_eq!(text(&app, node), "Cone");
}

#[test]
fn a_derived_signal_reads_every_frame_when_asked() {
    let mut app = themed_app(Warm);
    app.insert_resource(Count(1));
    let node = mount::<Warm>(
        app.world_mut(),
        label(
            derived(|world| world.resource::<Count>().0.to_string())
                .when(every_frame()),
        ),
    );

    app.world_mut().resource_mut::<Count>().0 = 7;
    app.update();

    assert_eq!(text(&app, node), "7");
}

#[test]
fn a_seeded_state_rule_holds_for_the_first_write() {
    use crate::Hovered;
    use crate::views::BehaviorExt;

    let mut app = themed_app(Warm);
    let seeded = mount::<Warm>(
        app.world_mut(),
        label("a")
            .when::<Hovered, Warm>(|label, _| {
                label.tone(Tone::Accent)
            })
            .seeded(Hovered),
    );
    let plain = mount::<Warm>(
        app.world_mut(),
        label("a").when::<Hovered, Warm>(|label, _| {
            label.tone(Tone::Accent)
        }),
    );

    assert_eq!(color(&app, seeded), Color::srgb(1.0, 0.5, 0.0));
    assert_eq!(color(&app, plain), Color::WHITE);
}

#[test]
fn a_seed_goes_to_the_first_node_of_its_view_alone() {
    use crate::Hovered;
    use crate::views::BehaviorExt;

    let mut app = themed_app(Warm);
    let root = mount::<Warm>(
        app.world_mut(),
        AnyView::<Bevy, Warm>::new(|cx| {
            let root = cx.spawn();
            cx.under(root, |cx| {
                cx.build(label("a"));
                cx.build(label("b"));
            });
            root
        })
        .seeded(Hovered),
    );
    let kids = children(&app, root);

    assert!(app.world().get::<Hovered>(root).is_some());
    assert!(app.world().get::<Hovered>(kids[0]).is_none());
    assert!(app.world().get::<Hovered>(kids[1]).is_none());
}

#[test]
fn a_seed_nothing_took_does_not_reach_a_later_node() {
    use crate::Hovered;
    use crate::views::BehaviorExt;

    let mut app = themed_app(Warm);
    let existing = mount::<Warm>(app.world_mut(), label("a"));
    mount::<Warm>(
        app.world_mut(),
        AnyView::<Bevy, Warm>::new(move |_| existing).seeded(Hovered),
    );
    let later = mount::<Warm>(app.world_mut(), label("b"));

    assert!(app.world().get::<Hovered>(later).is_none());
}

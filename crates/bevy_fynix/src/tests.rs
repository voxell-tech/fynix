//! A headless app per test, and two unrelated themes to build under.

use core::sync::atomic::{AtomicUsize, Ordering};

use bevy::app::App;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::name::Name;
use bevy::ecs::relationship::RelationshipTarget;
use bevy::ecs::resource::Resource;
use bevy::text::{
    FontSize, LineBreak, TextColor, TextFont, TextLayout,
};
use bevy::time::TimePlugin;
use bevy::ui::widget::Text;

mod structure;

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

/// Another app's, stored nothing like the first.
struct Cold {
    sizes: [f32; 2],
}

impl TextTokens for Cold {
    fn tone(&self, _: Tone) -> Color {
        Color::srgb(0.0, 0.5, 1.0)
    }

    fn body_size(&self) -> f32 {
        self.sizes[0]
    }

    fn small_size(&self) -> f32 {
        self.sizes[1]
    }
}

fn app<T: Send + Sync + 'static>(theme: T) -> App {
    let mut app = App::new();
    app.add_plugins((TimePlugin, FynixPlugin::<T>::default()))
        .insert_resource(Theme(theme));
    app
}

fn size(app: &App, node: Entity) -> FontSize {
    app.world()
        .get::<TextFont>(node)
        .expect("a label")
        .font_size
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

#[test]
fn an_unset_prop_falls_back_to_the_theme() {
    let mut app = app(Warm);
    let node = mount::<Warm>(app.world_mut(), label("Save"));

    assert_eq!(text(&app, node), "Save");
    assert_eq!(size(&app, node), FontSize::Px(14.0));
    assert_eq!(color(&app, node), Color::WHITE);
}

#[test]
fn the_same_view_works_under_two_unrelated_themes() {
    let mut warm = app(Warm);
    let mut cold = app(Cold {
        sizes: [20.0, 16.0],
    });
    let in_warm = mount::<Warm>(warm.world_mut(), label("x"));
    let in_cold = mount::<Cold>(cold.world_mut(), label("x"));

    assert_eq!(size(&warm, in_warm), FontSize::Px(14.0));
    assert_eq!(size(&cold, in_cold), FontSize::Px(20.0));
}

#[test]
fn a_set_rule_fills_what_the_call_site_left_unset() {
    let mut app = app(Warm);
    let root = mount::<Warm>(
        app.world_mut(),
        AnyView::<Bevy, Warm>::new(|cx| {
            let root = cx.spawn();
            cx.under(root, |cx| {
                cx.set::<crate::views::Label>(|l, _| {
                    l.size(20.0).tone(Tone::Dim)
                });
                cx.build(label("ruled"));
                cx.build(label("explicit").size(9.0));
            });
            root
        }),
    );
    let [ruled, explicit] = children(&app, root)[..] else {
        panic!("two labels");
    };

    assert_eq!(size(&app, ruled), FontSize::Px(20.0));
    assert_eq!(
        size(&app, explicit),
        FontSize::Px(9.0),
        "call site wins"
    );
    assert_eq!(color(&app, explicit), Color::srgb(0.5, 0.5, 0.5));
}

#[test]
fn an_inner_scope_wins_and_ends_with_its_scope() {
    let mut app = app(Warm);
    let root = mount::<Warm>(
        app.world_mut(),
        AnyView::<Bevy, Warm>::new(|cx| {
            let root = cx.spawn();
            cx.under(root, |cx| {
                cx.set::<crate::views::Label>(|l, _| l.size(20.0));
                cx.scope(|cx| {
                    cx.set::<crate::views::Label>(|l, _| {
                        l.size(30.0)
                    });
                    cx.build(label("inner"));
                });
                cx.build(label("after"));
            });
            root
        }),
    );
    let [inner, after] = children(&app, root)[..] else {
        panic!("two labels");
    };

    assert_eq!(size(&app, inner), FontSize::Px(30.0));
    assert_eq!(size(&app, after), FontSize::Px(20.0));
}

#[test]
fn a_rule_can_read_the_theme() {
    let mut app = app(Warm);
    let root = mount::<Warm>(
        app.world_mut(),
        AnyView::<Bevy, Warm>::new(|cx| {
            let root = cx.spawn();
            cx.under(root, |cx| {
                cx.set::<crate::views::Label>(|l, theme: &Warm| {
                    l.size(theme.small_size())
                });
                cx.build(label("small"));
            });
            root
        }),
    );

    assert_eq!(
        size(&app, children(&app, root)[0]),
        FontSize::Px(11.0)
    );
}

#[test]
fn a_show_rule_wins_over_the_call_site() {
    let mut app = app(Warm);
    let root = mount::<Warm>(
        app.world_mut(),
        AnyView::<Bevy, Warm>::new(|cx| {
            let root = cx.spawn();
            cx.under(root, |cx| {
                cx.show::<crate::views::Label>(|l, _| l.wrap(false));
                cx.build(label("x").wrap(true));
            });
            root
        }),
    );
    let node = children(&app, root)[0];

    let layout =
        app.world().get::<TextLayout>(node).expect("a label");
    assert_eq!(layout.linebreak, LineBreak::NoWrap);
}

#[derive(Resource)]
struct Count(u32);

#[test]
fn a_bound_prop_follows_the_world() {
    let mut app = app(Warm);
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
    let mut app = app(Warm);
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
    let mut app = app(Warm);
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
    let mut app = app(Warm);
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
    let mut app = app(Warm);
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
    let mut app = app(Warm);
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

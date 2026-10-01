//! The crate's views in a window, one section per idea: a theme
//! implemented through token traits, an app-wide set rule, bound
//! labels, hover rules with transitions, one state rule reaching a
//! button's parts, button styles (bundles and a custom one), a scoped
//! rule, a folding section, field rows, text and number fields, a
//! screen switch and a keyed list that rebuild structure and animate
//! views in and out, menus (a dropdown, a right-click menu and a
//! tooltip), and a reduced-motion switch.
//!
//! `cargo run -p bevy_fynix --example gallery`

use core::time::Duration;

use bevy::DefaultPlugins;
use bevy::app::{App, Startup};
use bevy::asset::Handle;
use bevy::camera::Camera2d;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::world::World;
use bevy::time::Time;
use bevy::ui::{AlignItems, FlexDirection, UiRect, percent, px};
use bevy_fynix::dock::{
    DockAreaStyle, DockLeaf, DockPlugin, DockRegistry, DockTree,
    DockWindowKind, Edge, dock,
};
use bevy_fynix::tokens::{
    Curve, Motion, MotionTokens, SpacingTokens, SurfaceTokens,
    TextTokens, Tone,
};
use bevy_fynix::views::{
    AnimatedField, BehaviorExt, ContextMenuExt, Frame, FrameProps,
    HasAction, Label, TooltipExt, button, checkbox, column, danger,
    dropdown, field_row, foldable, frame, ghost, icon_button, label,
    menu_bar, menu_item, number_field, primary, row, segmented,
    text_field, tint,
};
use bevy_fynix::{
    AnyView, Bevy, Cx, FynixPlugin, Hovered, Pressed, ReducedMotion,
    ScopedExt, StateExt, Style, Theme, View, ViewExt, each, hidden,
    keyed, mount, resource, style,
};

/// What a view is built with, in this app.
type Build<'a> = Cx<'a, Bevy, Monokai>;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FynixPlugin::<Monokai>::default(),
            DockPlugin::<Monokai>::default(),
        ))
        .insert_resource(Theme(Monokai))
        .insert_resource(Clicks(0))
        .insert_resource(Mode(0))
        .insert_resource(Agreed(false))
        .insert_resource(Easing(0))
        .insert_resource(Screen::Overview)
        .insert_resource(Title("fynix".into()))
        .insert_resource(Amount(1.0))
        .insert_resource(Copies(1))
        .insert_resource(Rows {
            ids: vec![1, 2, 3],
            next: 4,
        })
        .add_systems(Startup, setup)
        .run();
}

/// The app's theme. Nothing in the views names it: they ask for
/// whatever token traits they need, and this implements them.
struct Monokai;

fn hex(rgb: u32) -> Color {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color::srgb_u8(r, g, b)
}

impl TextTokens for Monokai {
    fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Body => hex(0xFCFCFA),
            Tone::Dim => hex(0x939293),
            Tone::Faint => hex(0x727072),
            Tone::Accent => hex(0xFFD866),
            Tone::OnAccent => hex(0x2D2A2E),
            Tone::Critical => hex(0xFF6188),
        }
    }

    fn body_size(&self) -> f32 {
        14.0
    }

    fn small_size(&self) -> f32 {
        11.0
    }
}

impl SurfaceTokens for Monokai {
    fn fill(&self) -> Color {
        hex(0x403E41)
    }

    fn hover(&self) -> Color {
        hex(0x5B595C)
    }

    fn panel(&self) -> Color {
        hex(0x221F22)
    }

    fn accent(&self) -> Color {
        hex(0xFFD866)
    }
}

impl SpacingTokens for Monokai {
    fn gap(&self) -> f32 {
        8.0
    }

    fn row(&self) -> f32 {
        24.0
    }

    fn radius(&self) -> f32 {
        4.0
    }
}

impl MotionTokens for Monokai {
    fn motion(&self, motion: Motion) -> Curve {
        let millis = match motion {
            Motion::Interact => 180,
            Motion::Expand => 280,
        };
        Curve {
            duration: Duration::from_millis(millis),
            // Ease out: quick to answer, gentle to land.
            ease: |t| 1.0 - (1.0 - t) * (1.0 - t),
        }
    }
}

/// How many times the counter's button was pressed.
#[derive(Resource)]
struct Clicks(u32);

/// The segment picked in the segmented control.
#[derive(Resource)]
struct Mode(usize);

/// Whether the checkbox is ticked.
#[derive(Resource)]
struct Agreed(bool);

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    seed_dock(world);
    mount::<Monokai>(world, gallery());
}

/// Three window kinds, two of them tabbed in one area and the third
/// in an area beside it.
fn seed_dock(world: &mut World) {
    let mut registry = world.resource_mut::<DockRegistry<Monokai>>();
    for (id, name) in
        [("notes", "Notes"), ("log", "Log"), ("stats", "Stats")]
    {
        let window = DockWindowKind::new(name, move || {
            label(format!("The {name} window")).boxed()
        });
        registry.register(id, window);
    }
    let mut tree = world.resource_mut::<DockTree>();
    let main = tree.set_root_leaf(
        DockLeaf::new("main", DockAreaStyle::TabBar)
            .with_windows(vec!["notes".into(), "log".into()]),
    );
    tree.split(main, Edge::Right, "stats".into());
}

/// Split panes with tabs: drag the line between them, click a tab,
/// close one, or add a window with the plus.
fn docking() -> impl View<Bevy, Monokai> {
    column((dock::<Monokai>(),))
        .width(px(520.0))
        .height(px(200.0))
}

fn gallery() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        // The app's preamble: every label is 13px unless its call
        // site or an inner scope says otherwise.
        cx.set::<Label>(|label, _| label.size(13.0));
        let panel = cx.theme().panel();
        cx.build(
            column((
                label("Fynix on Bevy").size(20.0),
                section("Bound values", bound_values()),
                section("Hover, with a transition", hover_list()),
                section(
                    "One state rule across a button's parts",
                    parts(),
                ),
                section(
                    "Button variants as rule bundles",
                    variants(),
                ),
                section("Styles", styles()),
                section("Segmented control and checkbox", controls()),
                section("A scoped rule", scoped()),
                section("Folding", folding()),
                section("Field rows", fields()),
                section("Text and number fields", inputs()),
                section("Switching", switching()),
                section("A keyed list", keyed_list()),
                section("Menus", menus()),
                section("Docking", docking()),
                section("Motion", motion_switch()),
            ))
            .width(percent(100.0))
            .height(percent(100.0))
            .padding(UiRect::all(px(20.0)))
            .gap(18.0)
            .fill(panel),
        )
    })
}

/// A dim caption over its contents.
fn section(
    title: &'static str,
    body: impl View<Bevy, Monokai>,
) -> impl View<Bevy, Monokai> {
    column((label(title).size(11.0).tone(Tone::Dim), body)).gap(6.0)
}

/// Labels bound to the world, and buttons that change it.
fn bound_values() -> impl View<Bevy, Monokai> {
    let padding = UiRect::axes(px(10.0), px(4.0));
    column((
        row((
            label(resource::<Clicks, _>(|clicks| {
                format!("Clicked {} times", clicks.0)
            })),
            button(label("+1")).padding(padding).on_activate(
                |world| world.resource_mut::<Clicks>().0 += 1,
            ),
            button(label("Reset")).padding(padding).on_activate(
                |world| world.resource_mut::<Clicks>().0 = 0,
            ),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
        label(resource::<Time, _>(|time| {
            format!("{:.1}s since start", time.elapsed_secs())
        }))
        .tone(Tone::Dim),
    ))
    .gap(6.0)
}

/// A label that turns accent and grows under the pointer, over the
/// theme's curve. The growth is a transform, so nothing around the
/// label moves.
fn line(text: &str) -> impl View<Bevy, Monokai> + use<> {
    label(text)
        .when::<Hovered, Monokai>(|label, _| {
            label.tone(Tone::Accent).scale(1.1)
        })
        .transition(Motion::Interact)
}

/// One state rule for a whole button: hovering lights every label in
/// it that did not choose its own tone, and pressing darkens the
/// frame over the button's own hover fill.
fn parts() -> impl View<Bevy, Monokai> {
    row((
        button(
            row((label("Save"), label("Ctrl+S").tone(Tone::Dim)))
                .gap(12.0),
        )
        .padding(UiRect::axes(px(10.0), px(4.0)))
        .when::<Hovered, _>(|cx: &mut Build| {
            cx.set::<Label>(|label, _| label.tone(Tone::Accent));
        })
        .when::<Pressed, _>(|cx: &mut Build| {
            cx.root(|cx| {
                cx.set::<Frame>(|frame, theme| {
                    frame.fill(theme.panel())
                });
            });
        })
        .transition(Motion::Interact),
        label("The shortcut keeps its dim tone").tone(Tone::Dim),
    ))
    .gap(8.0)
    .align(AlignItems::Center)
}

fn hover_list() -> impl View<Bevy, Monokai> {
    // Start-aligned, so each label is as wide as its text and scales
    // around it.
    column(
        ["cube.glb", "sphere.glb", "brick", "hello_world.mox"]
            .into_iter()
            .map(line)
            .collect::<Vec<_>>(),
    )
    .gap(4.0)
    .align(AlignItems::Start)
}

/// Rules set in a scope end with it, and an explicit value still
/// beats them.
fn scoped() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        let root = cx
            .build(frame().direction(FlexDirection::Column).gap(4.0));
        cx.under(root, |cx| {
            cx.scope(|cx| {
                cx.set::<Label>(|label, _| label.tone(Tone::Dim));
                cx.build(label(
                    "Inside the scope: dim, from one rule",
                ));
                cx.build(
                    label("An explicit tone still wins")
                        .tone(Tone::Accent),
                );
            });
            cx.build(label("After the scope: back to the preamble"));
        });
        root
    })
}

fn folding() -> impl View<Bevy, Monokai> {
    foldable(
        label("Assets"),
        column((
            label("meshes/cube.glb").tone(Tone::Dim),
            label("materials/brick").tone(Tone::Dim),
            label("scenes/hello_world.mox").tone(Tone::Dim),
        ))
        .gap(2.0)
        .padding(UiRect::left(px(24.0))),
    )
    .open(true)
}

/// A field row's label column stays on one line through a scoped
/// rule. The animatable ones turn accent while their node holds
/// [`HasAction`], which the button toggles.
fn fields() -> impl View<Bevy, Monokai> {
    column((
        field_row(
            label("translation").animatable::<Monokai>("translation"),
            label("0.0  0.0  0.0").tone(Tone::Dim),
        ),
        field_row(
            label("rotation").animatable::<Monokai>("rotation"),
            label("0.0  0.0  0.0").tone(Tone::Dim),
        ),
        field_row(
            label("a long field name, kept on one line"),
            label("plain, not animatable").tone(Tone::Dim),
        ),
        // A row of its own, so the column does not stretch it.
        row((button(label("Toggle keyframes"))
            .padding(UiRect::axes(px(10.0), px(4.0)))
            .on_activate(toggle_keyframes),)),
    ))
    .gap(4.0)
}

/// The text and the number the input fields edit.
#[derive(Resource)]
struct Title(String);

#[derive(Resource)]
struct Amount(f64);

#[derive(Resource)]
struct Copies(u32);

/// A text field and a number field, each bound to a resource and
/// writing back to it. The number drags by default and types after a
/// click.
fn inputs() -> impl View<Bevy, Monokai> {
    column((
        row((
            label("title"),
            text_field(
                resource::<Title, _>(|title| title.0.clone()),
                |world, text| world.resource_mut::<Title>().0 = text,
            )
            .width(px(160.0)),
            label(resource::<Title, _>(|title| {
                format!("= {}", title.0)
            }))
            .tone(Tone::Dim),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
        row((
            label("amount"),
            number_field(
                resource::<Amount, _>(|amount| amount.0),
                |world, value| {
                    world.resource_mut::<Amount>().0 = value
                },
            )
            .step(0.1)
            .precision(1)
            .range(0.0, 100.0)
            .width(px(80.0)),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
        row((
            label("copies"),
            number_field(
                resource::<Copies, _>(|copies| copies.0),
                |world, value| {
                    world.resource_mut::<Copies>().0 = value
                },
            )
            .range(1, 99)
            .width(px(80.0)),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
    ))
    .gap(6.0)
}

fn toggle_keyframes(world: &mut World) {
    let fields = world
        .query_filtered::<Entity, With<AnimatedField>>()
        .iter(world)
        .collect::<Vec<_>>();
    for field in fields {
        let mut field = world.entity_mut(field);
        if field.contains::<HasAction>() {
            field.remove::<HasAction>();
        } else {
            field.insert(HasAction);
        }
    }
}

/// The screen shown by the switching section.
#[derive(Resource, Clone, Copy, PartialEq)]
enum Screen {
    Overview,
    Details,
}

/// A button that changes the world, padded like the others.
fn action(
    text: &'static str,
    run: fn(&mut World),
) -> impl View<Bevy, Monokai> {
    button(label(text))
        .padding(UiRect::axes(px(10.0), px(4.0)))
        .on_activate(run)
}

/// A `keyed` on the screen resource. Each screen is built when it is
/// shown, under the app's preamble, and a switch is one sequence: the
/// last screen fades out and its space collapses, then the new
/// screen's space expands and it fades in. The content below follows
/// the space, and never jumps.
fn switching() -> impl View<Bevy, Monokai> {
    column((
        row((
            action("Switch screen", |world| {
                let mut screen = world.resource_mut::<Screen>();
                *screen = match *screen {
                    Screen::Overview => Screen::Details,
                    Screen::Details => Screen::Overview,
                };
            }),
            label(resource::<Screen, _>(|screen| {
                match screen {
                    Screen::Overview => "Showing: overview",
                    Screen::Details => "Showing: details",
                }
                .to_string()
            }))
            .tone(Tone::Dim),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
        keyed(resource::<Screen, _>(|screen| *screen), |screen| {
            match screen {
                Screen::Overview => column((
                    label("Overview"),
                    label("Three things happened today")
                        .tone(Tone::Dim),
                ))
                .gap(2.0)
                .appear::<Monokai>(hidden)
                .transition(Motion::Expand)
                .boxed(),
                Screen::Details => column((
                    label("Details").tone(Tone::Accent),
                    label("Nothing else to see").tone(Tone::Dim),
                    label(resource::<Clicks, _>(|clicks| {
                        format!("Counter reads {}", clicks.0)
                    })),
                ))
                .gap(2.0)
                .appear::<Monokai>(hidden)
                .transition(Motion::Expand)
                .boxed(),
            }
        })
        .within(column(())),
    ))
    .gap(6.0)
}

/// The ids of the rows in the keyed list, and the next id to hand
/// out.
#[derive(Resource)]
struct Rows {
    ids: Vec<u32>,
    next: u32,
}

/// An `each` keyed by row id. A row that stays keeps its entity, so
/// its hover transition survives a reorder. A new row first expands
/// its space and then fades in, and a removed row fades out and then
/// collapses its space.
fn keyed_list() -> impl View<Bevy, Monokai> {
    column((
        row((
            action("Add row", |world| {
                let mut rows = world.resource_mut::<Rows>();
                let id = rows.next;
                rows.ids.push(id);
                rows.next += 1;
            }),
            action("Remove first", |world| {
                let mut rows = world.resource_mut::<Rows>();
                if !rows.ids.is_empty() {
                    rows.ids.remove(0);
                }
            }),
            action("Reverse", |world| {
                world.resource_mut::<Rows>().ids.reverse();
            }),
        ))
        .gap(8.0),
        each(
            resource::<Rows, _>(|rows| rows.ids.clone()),
            |id| *id,
            |id| {
                line(&format!("Row {id}"))
                    .appear::<Monokai>(hidden)
                    .boxed()
            },
        )
        .within(column(()).gap(4.0).align(AlignItems::Start)),
    ))
    .gap(6.0)
}

/// The same button under four bundles.
fn variants() -> impl View<Bevy, Monokai> {
    row((
        button(label("Ghost")).rules(ghost),
        button(label("Tint")).rules(tint),
        button(label("Menu")).rules(menu_bar),
        button(row((label("Icon and"), label("label")))).rules(ghost),
    ))
    .gap(8.0)
    .align(AlignItems::Center)
}

/// A button of a kind of its own: a hover fill and a press fill from
/// the theme, a click look that shrinks it a little, and a label that
/// goes critical under the finger.
fn chunky() -> Style<Monokai> {
    style::<Monokai>()
        .fill(|theme| theme.panel())
        .frame(|frame, theme| {
            frame
                .padding(UiRect::axes(
                    px(theme.gap() * 2.0),
                    px(theme.gap()),
                ))
                .radius(theme.radius() * 3.0)
                .border(1.0)
                .border_color(theme.hairline())
        })
        .hovered(|s| s.fill(|theme| theme.fill()).tone(Tone::Accent))
        .pressed(|s| {
            s.fill(|theme| theme.pressed())
                .frame(|frame, _| frame.scale(0.96))
                .tone(Tone::Critical)
        })
        .transition(Motion::Interact)
}

/// Each bundle and a custom style, hovering and pressing each to see
/// its looks.
fn styles() -> impl View<Bevy, Monokai> {
    column((
        row((
            button(label("Default"))
                .padding(UiRect::axes(px(16.0), px(8.0))),
            button(label("Ghost")).rules(ghost),
            button(label("Tint")).rules(tint),
            button(label("+").size(16.0)).rules(icon_button),
            button(label("Primary")).rules(primary),
            button(label("Danger")).rules(danger),
            button(label("Custom")).rules(chunky().bundle()),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
        label("Hover for a surface, hold the button down to press")
            .tone(Tone::Dim),
    ))
    .gap(6.0)
}

/// A segmented control and a checkbox, each bound to the world and
/// writing to it.
fn controls() -> impl View<Bevy, Monokai> {
    column((
        segmented(
            ["Select", "Move", "Scale"],
            resource::<Mode, _>(|mode| mode.0),
            |world, index| world.resource_mut::<Mode>().0 = index,
        )
        .width(px(240.0)),
        row((
            checkbox(resource::<Agreed, _>(|agreed| agreed.0))
                .on_change(|world, value| {
                    world.resource_mut::<Agreed>().0 = value;
                }),
            label(resource::<Agreed, _>(|agreed| {
                if agreed.0 { "Agreed" } else { "Not agreed" }
                    .to_string()
            })),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
    ))
    .gap(8.0)
}

/// The option the dropdown shows.
#[derive(Resource)]
struct Easing(usize);

const EASINGS: [&str; 3] = ["Linear", "Ease in", "Ease out"];

/// A dropdown bound to a resource, a menu on a right-click, and a
/// tooltip that fades in after a pause.
fn menus() -> impl View<Bevy, Monokai> {
    let padding = UiRect::axes(px(10.0), px(4.0));
    row((
        dropdown(
            EASINGS,
            resource::<Easing, _>(|easing| easing.0),
            Handle::default(),
            |world, at| world.resource_mut::<Easing>().0 = at,
        ),
        label("Right-click for a menu")
            .tone(Tone::Dim)
            .context_menu(|| {
                (
                    menu_item(label("Reset the counter"))
                        .on_activate(|world| {
                            world.resource_mut::<Clicks>().0 = 0
                        }),
                    menu_item(label("Add a click")).on_activate(
                        |world| world.resource_mut::<Clicks>().0 += 1,
                    ),
                )
            }),
        button(label("Hover me"))
            .padding(padding)
            .tooltip(|| label("A tooltip, after a short pause")),
    ))
    .gap(12.0)
    .align(AlignItems::Center)
}

fn motion_switch() -> impl View<Bevy, Monokai> {
    row((
        button(label(resource::<ReducedMotion, _>(|reduced| {
            format!(
                "Reduced motion: {}",
                if reduced.0 { "on" } else { "off" }
            )
        })))
        .padding(UiRect::axes(px(10.0), px(4.0)))
        .on_activate(|world| {
            let mut reduced = world.resource_mut::<ReducedMotion>();
            reduced.0 = !reduced.0;
        }),
        label("Snaps hover, switching and list changes")
            .tone(Tone::Dim),
    ))
    .gap(8.0)
    .align(AlignItems::Center)
}

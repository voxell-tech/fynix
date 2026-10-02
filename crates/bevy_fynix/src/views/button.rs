//! A clickable [`Frame`] around one content view.
//!
//! Like a [`Stack`](super::Stack), it holds its frame and takes the
//! frame's props through [`FrameProps`]. Everything else is rules:
//! its defaults (fill, radius, centred content, hover and pressed
//! fills and a transition) are defaults for its root frame alone, so
//! an app's `set::<Frame>` and a call site's
//! `.when::<Hovered, _>(..)` both beat them, and none of them reach
//! the content.
//!
//! The looks a button comes in are rule bundles, applied with
//! `.rules(..)`. Each sets a resting, a hovered and a pressed look
//! from the theme's tokens, and travels between them over
//! [`Motion::Interact`]:
//!
//! | Bundle | Look |
//! |---|---|
//! | [`ghost`] | no surface until the pointer is on it |
//! | [`tint`] | no surface; the content turns accent on hover |
//! | [`icon_button`] | a square, row-high tint for an icon |
//! | [`primary`] | an accent fill with content on accent |
//! | [`danger`] | critical content on the usual surface |
//! | [`menu_bar`] | full height and square, for a menu bar |
//! | [`segment`] | a square, growing part of a row of options |
//!
//! For a look of your own, build a [`Style`] and apply its
//! [`bundle`](Style::bundle).

use bevy::asset::Handle;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::image::Image;
use bevy::ui::{AlignItems, JustifyContent, UiRect, percent, px};
use bevy::ui_widgets::Button as ButtonBehavior;
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::style::{Style, style};
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
    Tone,
};
use crate::views::frame::{Frame, FrameProps};
use crate::views::icon::icon;
use crate::{Bevy, Cx, ScopedExt, Styled, View};

pub struct Button<C> {
    pub frame: Frame,
    pub content: C,
}

pub fn button<C>(content: C) -> Button<C> {
    Button {
        frame: Frame::unset(),
        content,
    }
}

/// A button's defaults, for its root frame alone.
fn defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    surface::<T>(
        |theme| theme.fill(),
        |theme| theme.hover(),
        |theme| theme.pressed(),
    )
    .frame(|frame, theme| {
        frame
            .radius(theme.radius())
            .justify(JustifyContent::Center)
            .align(AlignItems::Center)
    })
    .apply(cx);
}

/// A root frame filled with `rest`, `hover` while the pointer is on
/// it and `pressed` while a button is down, travelling between them.
fn surface<T>(
    rest: fn(&T) -> Color,
    hover: fn(&T) -> Color,
    pressed: fn(&T) -> Color,
) -> Style<T>
where
    T: MotionTokens + 'static,
{
    style()
        .fill(rest)
        .hovered(|s| s.fill(hover))
        .pressed(|s| s.fill(pressed))
        .transition(Motion::Interact)
}

/// A surface that is not there, under the pointer or not.
fn clear<T>(_: &T) -> Color {
    Color::NONE
}

/// A surface that lights up under the pointer and deepens when
/// pressed, but is not there at rest.
fn ghostly<T>() -> Style<T>
where
    T: SurfaceTokens + MotionTokens + 'static,
{
    surface::<T>(
        clear,
        |theme| theme.hover(),
        |theme| theme.pressed(),
    )
}

/// The padding along a strip of buttons, twice the theme's gap.
fn strip_pad<T: SpacingTokens>(theme: &T) -> f32 {
    theme.gap() * 2.0
}

/// A button with no surface until the pointer is on it, then a
/// hover surface, and a deeper one while pressed. Padded, with the
/// theme's radius. Apply with `.rules(ghost)`.
pub fn ghost<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    ghostly::<T>()
        .frame(|frame, theme| {
            frame
                .padding(UiRect::axes(
                    px(strip_pad(theme)),
                    px(theme.gap()),
                ))
                .radius(theme.radius())
        })
        .apply(cx);
}

/// A button with no surface at all, resting, hovered or pressed.
/// Every [`Label`](super::Label) and [`Icon`](super::Icon) in it
/// turns [`Tone::Accent`] while the pointer is on it, and
/// [`Tone::Dim`] while pressed. Apply with `.rules(tint)`.
pub fn tint<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + MotionTokens + 'static,
{
    ghostly::<T>()
        .hovered(|s| s.fill(clear).tone(Tone::Accent))
        .pressed(|s| s.fill(clear).tone(Tone::Dim))
        .apply(cx);
}

/// A square button, a theme row on each side, for one icon. It only
/// tints: no surface at rest, hovered or pressed, and a dim icon at
/// rest, a body icon under the pointer and an accent icon while
/// pressed. Apply with `.rules(icon_button)`.
pub fn icon_button<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    style::<T>()
        .fill(clear)
        .frame(|frame, theme| {
            frame
                .width(px(theme.row()))
                .height(px(theme.row()))
                .padding(UiRect::ZERO)
                .radius(theme.radius())
        })
        .tone(Tone::Dim)
        .hovered(|s| s.fill(clear).tone(Tone::Body))
        .pressed(|s| s.fill(clear).tone(Tone::Accent))
        .transition(Motion::Interact)
        .apply(cx);
}

/// The button to press: an accent fill, drawn on with
/// [`Tone::OnAccent`], lighter under the pointer and darker while
/// pressed. Padded, with the theme's radius. Apply with
/// `.rules(primary)`.
pub fn primary<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    surface::<T>(
        |theme| theme.accent(),
        |theme| theme.accent_hover(),
        |theme| theme.accent_pressed(),
    )
    .frame(|frame, theme| {
        frame
            .padding(UiRect::axes(
                px(strip_pad(theme)),
                px(theme.gap()),
            ))
            .radius(theme.radius())
    })
    .tone(Tone::OnAccent)
    .apply(cx);
}

/// A button for a destructive action: its content in
/// [`Tone::Critical`] on the usual surfaces, hovered and pressed.
/// Padded, with the theme's radius. Apply with `.rules(danger)`.
pub fn danger<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    surface::<T>(
        |theme| theme.fill(),
        |theme| theme.hover(),
        |theme| theme.pressed(),
    )
    .frame(|frame, theme| {
        frame
            .padding(UiRect::axes(
                px(strip_pad(theme)),
                px(theme.gap()),
            ))
            .radius(theme.radius())
    })
    .tone(Tone::Critical)
    .apply(cx);
}

/// A button for a menu bar: no resting surface, the height of its
/// parent, square corners and horizontal padding, lighting up under
/// the pointer and deepening when pressed. Apply with
/// `.rules(menu_bar)`.
pub fn menu_bar<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    ghostly::<T>()
        .frame(|frame, theme| {
            frame
                .height(percent(100.0))
                .radius(0.0)
                .padding(UiRect::axes(px(strip_pad(theme)), px(0.0)))
        })
        .apply(cx);
}

/// A button for a row of exclusive options: a full row high, square,
/// and growing to share the row. It lights up and deepens like any
/// button while not `active`. While `active` it is filled with the
/// theme's accent, drawn on with [`Tone::OnAccent`], and does not
/// change under the pointer or when pressed. Apply with
/// `.rules(segment(active))`.
pub fn segment<T>(
    active: bool,
) -> impl FnOnce(&mut Cx<'_, Bevy, T>) + Send + Sync + 'static
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    let look = if active {
        surface::<T>(
            |theme| theme.accent(),
            |theme| theme.accent(),
            |theme| theme.accent(),
        )
        .tone(Tone::OnAccent)
    } else {
        surface::<T>(
            |theme| theme.fill(),
            |theme| theme.hover(),
            |theme| theme.pressed(),
        )
    };
    look.frame(|frame, theme| {
        frame.height(px(theme.row())).radius(0.0).grow(1.0)
    })
    .bundle()
}

impl<C> FrameProps for Button<C> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// A button of one icon in the [`tint`] look, padded by half the
/// theme's gap: for an action that sits beside content, such as
/// adding to a list. Its frame styles the button.
pub struct TintedIcon {
    pub frame: Frame,
    image: Handle<Image>,
}

/// A [`TintedIcon`] showing `image`.
pub fn tinted_icon(image: Handle<Image>) -> TintedIcon {
    TintedIcon {
        frame: Frame::unset(),
        image,
    }
}

impl FrameProps for TintedIcon {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T> View<Bevy, T> for TintedIcon
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let mut button = button(icon(self.image));
        button.frame = self.frame;
        let padded = style::<T>()
            .frame(|frame, theme| {
                frame.padding(UiRect::all(px(theme.gap() / 2.0)))
            })
            .bundle();
        cx.build(button.rules(tint).rules(padded))
    }
}

impl<T, C> View<Bevy, T> for Button<C>
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    C: View<Bevy, T>,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        cx.scope(|cx| {
            cx.defaults(|cx| cx.root(defaults));
            let node = cx.build(self.frame);
            cx.world.entity_mut(node).insert((
                ButtonBehavior,
                EntityCursor(SystemCursorIcon::Pointer),
            ));
            cx.under(node, |cx| cx.build(self.content));
            node
        })
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::{App, PreUpdate};
    use bevy::asset::Handle;
    use bevy::color::{Color, Luminance};
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::lifecycle::Remove;
    use bevy::ecs::observer::On;
    use bevy::ecs::resource::Resource;
    use bevy::ecs::system::ResMut;
    use bevy::picking::backend::HitData;
    use bevy::picking::hover::{HoverMap, update_is_hovered};
    use bevy::picking::pointer::PointerId;
    use bevy::text::{FontSize, TextColor, TextFont};
    use bevy::ui::widget::ImageNode;
    use bevy::ui::{BackgroundColor, Node, Val};
    use motiongfx_interp::interpolation::Interpolation;

    use super::*;
    use crate::tests::{self, Plain};
    use crate::transition::{BevyMarker, ReducedMotion};
    use crate::views::{Label, icon, label, row};
    use crate::{
        AnyView, Hovered, Pressed, ScopedExt, StateExt, mount,
    };

    const REST: Color = Color::srgb(0.2, 0.2, 0.2);
    const HOVER: Color = Color::srgb(0.3, 0.3, 0.3);
    const ACCENT: Color = Color::srgb(0.9, 0.5, 0.1);
    const ON_ACCENT: Color = Color::srgb(0.1, 0.1, 0.1);

    /// How many times [`Hovered`] was taken off a node.
    #[derive(Resource, Default)]
    struct Releases(usize);

    fn app() -> App {
        let mut app = tests::app_with(Plain {
            accent: Some(ACCENT),
            accent_tone: Some(ACCENT),
            on_accent: Some(ON_ACCENT),
            ..Plain::default()
        });
        app.init_resource::<Releases>()
            .add_systems(PreUpdate, update_is_hovered)
            .add_observer(
                |_: On<Remove, Hovered>,
                 mut releases: ResMut<Releases>| {
                    releases.0 += 1;
                },
            );
        app
    }

    fn fill(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
    }

    fn blend(from: Color, to: Color, t: f32) -> Color {
        <Color as Interpolation<BevyMarker>>::interp(&from, &to, t)
    }

    fn hover(app: &mut App, node: Entity, on: bool) {
        let mut node = app.world_mut().entity_mut(node);
        if on {
            node.insert(Hovered);
        } else {
            node.remove::<Hovered>();
        }
    }

    /// Puts the mouse over `entity` alone, then runs an update.
    fn point_at(app: &mut App, entity: Option<Entity>) {
        let mut map = HoverMap::default();
        if let Some(entity) = entity {
            let hit =
                HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
            map.0
                .entry(PointerId::Mouse)
                .or_default()
                .insert(entity, hit);
        }
        app.insert_resource(map);
        app.update();
    }

    fn hovered(app: &App, node: Entity) -> bool {
        app.world().get::<Hovered>(node).is_some()
    }

    #[test]
    fn hovering_moves_the_fill_to_the_hover_colour_and_back() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));

        hover(&mut app, node, true);
        app.update();
        assert_eq!(
            fill(&app, node),
            blend(REST, HOVER, 0.5),
            "50ms of 100ms"
        );
        app.update();
        assert_eq!(fill(&app, node), HOVER);

        hover(&mut app, node, false);
        app.update();
        assert_eq!(fill(&app, node), blend(HOVER, REST, 0.5));
        app.update();
        assert_eq!(fill(&app, node), REST);
    }

    #[test]
    fn a_call_site_state_rule_beats_the_default_hover_fill() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).when::<Hovered, _>(
                |cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Frame>(|f, _| f.fill(Color::WHITE));
                },
            ),
        );

        hover(&mut app, node, true);
        app.update();
        app.update();

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn a_state_rule_on_the_button_reaches_its_content() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).when::<Hovered, _>(
                |cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Label>(|l, _| l.size(20.0));
                },
            ),
        );
        let content = app.world().get::<Children>(node).unwrap()[0];
        let size = |app: &App| {
            app.world().get::<TextFont>(content).unwrap().font_size
        };
        assert_eq!(size(&app), FontSize::Px(14.0));

        hover(&mut app, node, true);
        app.update();

        assert_eq!(size(&app), FontSize::Px(20.0));
    }

    #[test]
    fn a_call_site_fill_is_the_resting_fill() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).fill(Color::BLACK),
        );
        assert_eq!(fill(&app, node), Color::BLACK);

        hover(&mut app, node, true);
        app.update();
        app.update();
        assert_eq!(fill(&app, node), HOVER);

        hover(&mut app, node, false);
        app.update();
        app.update();
        assert_eq!(fill(&app, node), Color::BLACK);
    }

    #[test]
    fn a_hovered_child_keeps_the_button_hovered() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));
        let child = app.world().get::<Children>(node).unwrap()[0];

        point_at(&mut app, Some(child));
        assert!(hovered(&app, node));

        // From the label to the padding and back, the way the
        // pointer crosses the button.
        point_at(&mut app, Some(node));
        assert!(hovered(&app, node));
        point_at(&mut app, Some(child));
        assert!(hovered(&app, node));
        assert_eq!(app.world().resource::<Releases>().0, 0);
        assert_eq!(fill(&app, node), HOVER);

        point_at(&mut app, None);
        assert!(!hovered(&app, node));
        assert_eq!(app.world().resource::<Releases>().0, 1);
    }

    #[test]
    fn reduced_motion_snaps_the_fill() {
        let mut app = app();
        app.insert_resource(ReducedMotion(true));
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));

        hover(&mut app, node, true);
        app.update();
        assert_eq!(fill(&app, node), HOVER);

        hover(&mut app, node, false);
        app.update();
        assert_eq!(fill(&app, node), REST);
    }

    /// Hovers `node`, or lets go, and runs the transition out.
    fn settle(app: &mut App, node: Entity, on: bool) {
        hover(app, node, on);
        app.update();
        app.update();
    }

    fn ui(app: &App, node: Entity) -> &Node {
        app.world().get::<Node>(node).unwrap()
    }

    #[test]
    fn a_tint_button_has_no_surface_and_turns_its_parts_accent() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(row((icon(Handle::default()), label("x"))))
                .rules(tint),
        );
        let content = app.world().get::<Children>(node).unwrap()[0];
        let [mark, text] =
            app.world().get::<Children>(content).unwrap()[..]
        else {
            panic!("an icon and a label");
        };
        let colours = |app: &App| {
            (
                app.world().get::<ImageNode>(mark).unwrap().color,
                app.world().get::<TextColor>(text).unwrap().0,
            )
        };
        assert_eq!(fill(&app, node), Color::NONE);
        assert_eq!(colours(&app), (Color::WHITE, Color::WHITE));

        settle(&mut app, node, true);
        assert_eq!(fill(&app, node), Color::NONE);
        assert_eq!(colours(&app), (ACCENT, ACCENT));
        assert_eq!(
            ui(&app, content).padding,
            UiRect::DEFAULT,
            "the row is not a button"
        );

        settle(&mut app, node, false);
        assert_eq!(colours(&app), (Color::WHITE, Color::WHITE));
    }

    const DIM: Color = Color::srgb(0.5, 0.5, 0.5);
    const CRITICAL: Color = Color::srgb(1.0, 0.5, 0.0);

    fn pressed_fill() -> Color {
        Plain::default().pressed()
    }

    /// Puts the pointer on `node`, and a button down on it, as asked,
    /// and runs the transitions out.
    fn feel(app: &mut App, node: Entity, over: bool, down: bool) {
        hover(app, node, over);
        tests::press(app, node, down);
        app.update();
        app.update();
    }

    /// The fill of `node` resting, hovered and pressed, in that
    /// order.
    fn fills(app: &mut App, node: Entity) -> [Color; 3] {
        let rest = fill(app, node);
        feel(app, node, true, false);
        let hovered = fill(app, node);
        feel(app, node, true, true);
        let pressed = fill(app, node);
        feel(app, node, false, false);
        assert_eq!(fill(app, node), rest, "and back at rest");
        [rest, hovered, pressed]
    }

    /// The colour of the text in `node`'s content, resting, hovered
    /// and pressed.
    fn inks(app: &mut App, node: Entity) -> [Color; 3] {
        let text = app.world().get::<Children>(node).unwrap()[0];
        let ink =
            |app: &App| app.world().get::<TextColor>(text).unwrap().0;
        let rest = ink(app);
        feel(app, node, true, false);
        let hovered = ink(app);
        feel(app, node, true, true);
        let pressed = ink(app);
        feel(app, node, false, false);
        assert_eq!(ink(app), rest, "and back at rest");
        [rest, hovered, pressed]
    }

    #[test]
    fn a_plain_button_deepens_when_pressed() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));

        assert_eq!(
            fills(&mut app, node),
            [REST, HOVER, pressed_fill()]
        );
    }

    #[test]
    fn letting_the_pointer_go_lets_the_button_up() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(ghost),
        );

        point_at(&mut app, Some(node));
        tests::press(&mut app, node, true);
        app.update();
        app.update();
        assert_eq!(fill(&app, node), pressed_fill());

        point_at(&mut app, None);
        app.update();
        assert!(app.world().get::<Pressed>(node).is_none());
        assert_eq!(fill(&app, node), Color::NONE);
    }

    #[test]
    fn a_ghost_button_deepens_when_pressed() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(ghost),
        );

        assert_eq!(
            fills(&mut app, node),
            [Color::NONE, HOVER, pressed_fill()]
        );
    }

    #[test]
    fn a_tint_button_stays_clear_and_moves_its_content_through_tones()
    {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(tint),
        );

        assert_eq!(
            fills(&mut app, node),
            [Color::NONE; 3],
            "never a surface"
        );
        assert_eq!(inks(&mut app, node), [Color::WHITE, ACCENT, DIM]);
    }

    #[test]
    fn an_icon_button_only_tints_its_icon() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(icon(Handle::default())).rules(icon_button),
        );
        let mark = app.world().get::<Children>(node).unwrap()[0];

        assert_eq!(
            fills(&mut app, node),
            [Color::NONE; 3],
            "never a surface"
        );
        let tint = |app: &App| {
            app.world().get::<ImageNode>(mark).unwrap().color
        };
        assert_eq!(tint(&app), DIM);
        feel(&mut app, node, true, false);
        assert_eq!(tint(&app), Color::WHITE);
        feel(&mut app, node, true, true);
        assert_eq!(tint(&app), ACCENT);
    }

    #[test]
    fn a_primary_button_is_accent_with_content_on_accent() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(primary),
        );

        assert_eq!(
            fills(&mut app, node),
            [ACCENT, ACCENT.lighter(0.08), ACCENT.darker(0.1)]
        );
        assert_eq!(inks(&mut app, node), [ON_ACCENT; 3]);
    }

    #[test]
    fn a_danger_button_draws_critical_on_the_usual_surfaces() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(danger),
        );

        assert_eq!(
            fills(&mut app, node),
            [REST, HOVER, pressed_fill()]
        );
        assert_eq!(inks(&mut app, node), [CRITICAL; 3]);
    }

    #[test]
    fn a_menu_bar_button_and_an_idle_segment_deepen_when_pressed() {
        let mut app = app();
        let bar = mount::<Plain>(
            app.world_mut(),
            button(label("File")).rules(menu_bar),
        );
        let segment = mount::<Plain>(
            app.world_mut(),
            button(label("a")).rules(segment(false)),
        );

        assert_eq!(
            fills(&mut app, bar),
            [Color::NONE, HOVER, pressed_fill()]
        );
        assert_eq!(
            fills(&mut app, segment),
            [REST, HOVER, pressed_fill()]
        );
    }

    #[test]
    fn an_active_segment_does_not_change_when_pressed() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("a")).rules(segment(true)),
        );

        assert_eq!(fills(&mut app, node), [ACCENT; 3]);
    }

    #[test]
    fn a_press_travels_from_the_hover_fill_over_the_curve() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(primary),
        );

        feel(&mut app, node, true, false);
        tests::press(&mut app, node, true);
        app.update();
        assert_eq!(
            fill(&app, node),
            blend(ACCENT.lighter(0.08), ACCENT.darker(0.1), 0.5),
            "50ms of 100ms from hovered to pressed"
        );
    }

    #[test]
    fn a_call_site_pressed_rule_beats_a_bundles() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(ghost).when::<Pressed, _>(
                |cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Frame>(|f, _| f.fill(Color::WHITE));
                },
            ),
        );

        feel(&mut app, node, true, true);

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn a_call_site_prop_beats_a_bundle() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).fill(Color::WHITE).rules(ghost),
        );
        let strip = mount::<Plain>(
            app.world_mut(),
            button(label("x"))
                .height(px(50.0))
                .fill(Color::BLACK)
                .rules(segment(true)),
        );

        assert_eq!(fill(&app, node), Color::WHITE);
        assert_eq!(fill(&app, strip), Color::BLACK);
        assert_eq!(ui(&app, strip).height, px(50.0));
        assert_eq!(ui(&app, strip).flex_grow, 1.0, "the rest stays");
    }

    #[test]
    fn a_call_site_state_rule_beats_a_bundles() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x"))
                .rules(segment(true))
                .when::<Hovered, _>(|cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Frame>(|f, _| f.fill(Color::WHITE));
                }),
        );

        settle(&mut app, node, true);

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn a_bundle_does_not_reach_a_frame_in_the_content() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(row((label("x"),))).rules(menu_bar),
        );
        let content = app.world().get::<Children>(node).unwrap()[0];

        assert_eq!(ui(&app, content).height, Val::Auto);
        assert_eq!(fill(&app, content), Color::NONE);
    }

    #[test]
    fn a_set_rule_on_frame_does_not_reach_the_content() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            AnyView::<Bevy, Plain>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.set::<Frame>(|f, _| f.width(px(10.0)));
                    cx.set::<Label>(|l, _| l.size(20.0));
                    cx.build(button(label("x")));
                });
                root
            }),
        );
        let node = app.world().get::<Children>(root).unwrap()[0];

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.width, Val::Px(10.0));
        let content = app.world().get::<Children>(node).unwrap()[0];
        assert_eq!(
            app.world().get::<TextFont>(content).unwrap().font_size,
            FontSize::Px(20.0)
        );
    }
}

//! Styles: rule bundles written as a chain instead of by hand.
//!
//! A [`Style`] only collects what `cx.set`, `cx.root` and `cx.when`
//! would be told, then plays it back when applied. It adds nothing to
//! what rules can do: colours are read from the theme in closures, a
//! state's look is a nested style, and a style applies with
//! [`ScopedExt::rules`](crate::ScopedExt::rules) like any bundle.
//!
//! ```ignore
//! let danger = style::<Theme>()
//!     .fill(|t| t.fill())
//!     .tone(Tone::Critical)
//!     .hovered(|s| s.fill(|t| t.hover()))
//!     .pressed(|s| s.fill(|t| t.pressed()))
//!     .transition(Motion::Interact);
//!
//! button(label("Delete")).rules(danger.bundle())
//! ```
//!
//! The theme type is named once, on [`style`], as a closure cannot
//! ask for the token traits of a theme it has not been told.
//!
//! Fills and frame rules reach the root frame alone, so a button's
//! content keeps its own. Tones and element rules reach every
//! [`Label`] and [`Icon`] in the view, as a set rule does.
//!
//! Later steps win over earlier ones, in one style and in
//! [`then`](Style::then), as later set rules do. A state's rules beat
//! the resting ones while it holds, and a state written later beats
//! an earlier one while both hold, so write `pressed` after
//! `hovered`. A style that gives `hovered` but no `pressed` shows its
//! hover look while pressed.

use bevy::color::Color;
use bevy::ecs::component::Component;

use crate::state::{Hovered, Pressed, State};
use crate::tokens::{Motion, MotionTokens, Tone};
use crate::views::{Frame, Icon, Label};
use crate::{Bevy, Cx};

type Step<T> = Box<dyn FnOnce(&mut Cx<'_, Bevy, T>) + Send + Sync>;

/// A chain of rules for a view built under the theme `T`, applied
/// with [`bundle`](Self::bundle).
#[must_use]
pub struct Style<T> {
    steps: Vec<Step<T>>,
}

/// An empty style under the theme `T`.
pub fn style<T: 'static>() -> Style<T> {
    Style { steps: Vec::new() }
}

impl<T: 'static> Default for Style<T> {
    fn default() -> Self {
        style()
    }
}

impl<T: 'static> Style<T> {
    fn step(
        mut self,
        step: impl FnOnce(&mut Cx<'_, Bevy, T>) + Send + Sync + 'static,
    ) -> Self {
        self.steps.push(Box::new(step));
        self
    }

    /// This, with the root frame restyled by `rule`.
    pub fn frame(
        self,
        rule: impl Fn(Frame, &T) -> Frame + Send + Sync + 'static,
    ) -> Self {
        self.step(|cx| cx.root(|cx| cx.set::<Frame>(rule)))
    }

    /// This, with the root frame filled with the colour `colour`
    /// reads from the theme.
    pub fn fill(
        self,
        colour: impl Fn(&T) -> Color + Send + Sync + 'static,
    ) -> Self {
        self.frame(move |frame, theme| frame.fill(colour(theme)))
    }

    /// This, with every [`Label`] restyled by `rule`.
    pub fn label(
        self,
        rule: impl Fn(Label, &T) -> Label + Send + Sync + 'static,
    ) -> Self {
        self.step(|cx| cx.set::<Label>(rule))
    }

    /// This, with every [`Icon`] restyled by `rule`.
    pub fn icon(
        self,
        rule: impl Fn(Icon, &T) -> Icon + Send + Sync + 'static,
    ) -> Self {
        self.step(|cx| cx.set::<Icon>(rule))
    }

    /// This, with every [`Label`] and [`Icon`] drawn in `tone`.
    pub fn tone(self, tone: Tone) -> Self {
        self.label(move |label, _| label.tone(tone))
            .icon(move |icon, _| icon.tone(tone))
    }

    /// This, with what `look` gives a style holding while the root
    /// node has the component `S`.
    pub fn when<S: Component>(
        self,
        look: impl FnOnce(Self) -> Self,
    ) -> Self {
        let inner = look(style());
        self.step(|cx| {
            cx.when::<State<S>>(|cx| inner.apply(cx));
        })
    }

    /// This, with `look` holding while the pointer is over the view.
    pub fn hovered(self, look: impl FnOnce(Self) -> Self) -> Self {
        self.when::<Hovered>(look)
    }

    /// This, with `look` holding while a pointer button is down on
    /// the view. Write it after [`hovered`](Self::hovered).
    pub fn pressed(self, look: impl FnOnce(Self) -> Self) -> Self {
        self.when::<Pressed>(look)
    }

    /// This, with every element in the view travelling to new values
    /// over the theme's curve for `motion`.
    pub fn transition(self, motion: Motion) -> Self
    where
        T: MotionTokens,
    {
        self.step(move |cx| cx.transition(motion))
    }

    /// This, running the bundle `bundle`, such as
    /// [`ghost`](crate::views::ghost), as the next step. A hovered
    /// look written after it shows while pressed too, unless a
    /// pressed look is written as well.
    pub fn with(
        self,
        bundle: impl FnOnce(&mut Cx<'_, Bevy, T>) + Send + Sync + 'static,
    ) -> Self {
        self.step(bundle)
    }

    /// This, then `next`, which wins where both say.
    pub fn then(mut self, next: Self) -> Self {
        self.steps.extend(next.steps);
        self
    }

    /// Sets the rules of this style in `cx`.
    pub fn apply(self, cx: &mut Cx<'_, Bevy, T>) {
        for step in self.steps {
            step(cx);
        }
    }

    /// This, as a bundle for [`ScopedExt::rules`].
    ///
    /// [`ScopedExt::rules`]: crate::ScopedExt::rules
    pub fn bundle(
        self,
    ) -> impl FnOnce(&mut Cx<'_, Bevy, T>) + Send + Sync + 'static
    {
        move |cx: &mut Cx<'_, Bevy, T>| self.apply(cx)
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::entity::Entity;
    use bevy::text::TextColor;
    use bevy::ui::BackgroundColor;

    use super::*;
    use crate::tests::{self, Plain};
    use crate::tokens::{SurfaceTokens, TextTokens};
    use crate::views::{FrameProps, button, ghost, label, row};
    use crate::{ScopedExt, mount};

    const REST: Color = Color::srgb(0.2, 0.2, 0.2);
    const HOVER: Color = Color::srgb(0.3, 0.3, 0.3);
    const RED: Color = Color::srgb(1.0, 0.0, 0.0);
    const GREEN: Color = Color::srgb(0.0, 1.0, 0.0);
    const BLUE: Color = Color::srgb(0.0, 0.0, 1.0);

    fn fill(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
    }

    fn ink(app: &App, node: Entity) -> Color {
        let text = tests::kids(app, node)[0];
        app.world().get::<TextColor>(text).unwrap().0
    }

    /// Puts the pointer on `node`, and a button down on it, as asked,
    /// and runs the transitions out.
    fn feel(app: &mut App, node: Entity, over: bool, down: bool) {
        tests::hover(app, node, over);
        tests::press(app, node, down);
        app.update();
        app.update();
    }

    fn custom() -> Style<Plain> {
        style::<Plain>()
            .fill(|theme| theme.fill())
            .hovered(|s| s.fill(|_| GREEN))
            .pressed(|s| s.fill(|_| BLUE).tone(Tone::Accent))
            .transition(Motion::Interact)
    }

    #[test]
    fn a_style_sets_the_resting_look_from_the_theme() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(custom().bundle()),
        );

        assert_eq!(fill(&app, node), REST);
    }

    #[test]
    fn a_style_has_a_look_for_hover_and_a_look_for_press() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(custom().bundle()),
        );

        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), GREEN);
        assert_eq!(ink(&app, node), Color::WHITE);

        feel(&mut app, node, true, true);
        assert_eq!(fill(&app, node), BLUE, "pressed beats hovered");
        assert_eq!(
            ink(&app, node),
            Plain::default().tone(Tone::Accent)
        );

        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), GREEN);
        feel(&mut app, node, false, false);
        assert_eq!(fill(&app, node), REST);
    }

    #[test]
    fn without_a_pressed_look_a_press_keeps_the_hover_look() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .hovered(|s| s.fill(|_| GREEN))
                    .bundle(),
            ),
        );

        feel(&mut app, node, true, true);

        assert_eq!(fill(&app, node), GREEN);
    }

    #[test]
    fn a_style_reads_colours_from_whatever_theme_it_is_built_for() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .fill(|theme| theme.panel())
                    .hovered(|s| s.fill(|theme| theme.hover()))
                    .bundle(),
            ),
        );
        assert_eq!(fill(&app, node), Color::BLACK);

        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), HOVER);
    }

    #[test]
    fn a_style_reaches_the_root_frame_and_not_the_content() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(row((label("x"),)))
                .rules(style::<Plain>().fill(|_| RED).bundle()),
        );
        let content = tests::kids(&app, node)[0];

        assert_eq!(fill(&app, node), RED);
        assert_eq!(fill(&app, content), Color::NONE);
    }

    #[test]
    fn a_style_tones_the_labels_in_the_view() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(row((label("x"),))).rules(
                style::<Plain>()
                    .tone(Tone::Dim)
                    .hovered(|s| s.tone(Tone::Accent))
                    .bundle(),
            ),
        );
        let row = tests::kids(&app, node)[0];
        let text = tests::kids(&app, row)[0];
        let colour =
            |app: &App| app.world().get::<TextColor>(text).unwrap().0;
        assert_eq!(colour(&app), Plain::default().tone(Tone::Dim));

        feel(&mut app, node, true, false);
        assert_eq!(colour(&app), Plain::default().tone(Tone::Accent));
    }

    #[test]
    fn a_call_site_prop_beats_a_style() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x"))
                .fill(Color::WHITE)
                .rules(custom().bundle()),
        );

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn of_two_steps_in_one_style_the_later_wins() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .fill(|_| RED)
                    .fill(|_| GREEN)
                    .hovered(|s| s.fill(|_| RED).fill(|_| BLUE))
                    .bundle(),
            ),
        );
        assert_eq!(fill(&app, node), GREEN);

        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), BLUE);
    }

    #[test]
    fn then_lets_the_next_style_win() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .fill(|_| RED)
                    .hovered(|s| s.fill(|_| RED))
                    .then(
                        style()
                            .fill(|_| GREEN)
                            .hovered(|s| s.fill(|_| BLUE)),
                    )
                    .bundle(),
            ),
        );
        assert_eq!(fill(&app, node), GREEN);

        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), BLUE);
    }

    #[test]
    fn a_style_extends_a_bundle() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .with(ghost)
                    .hovered(|s| s.fill(|_| GREEN))
                    .pressed(|s| s.fill(|_| BLUE))
                    .bundle(),
            ),
        );
        assert_eq!(
            fill(&app, node),
            Color::NONE,
            "the bundle's rest"
        );

        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), GREEN, "its own hover look");
        feel(&mut app, node, true, true);
        assert_eq!(fill(&app, node), BLUE, "its own press look");
    }

    #[test]
    fn a_hover_look_added_to_a_bundle_shows_while_pressed_too() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .with(ghost)
                    .hovered(|s| s.fill(|_| GREEN))
                    .bundle(),
            ),
        );

        feel(&mut app, node, true, true);

        assert_eq!(
            fill(&app, node),
            GREEN,
            "written later than ghost's"
        );
    }

    #[test]
    fn bundles_chained_with_rules_both_apply() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x"))
                .rules(
                    style::<Plain>()
                        .tone(Tone::Dim)
                        .hovered(|s| s.fill(|_| GREEN))
                        .bundle(),
                )
                .rules(style::<Plain>().fill(|_| RED).bundle()),
        );

        assert_eq!(fill(&app, node), RED);
        assert_eq!(ink(&app, node), Plain::default().tone(Tone::Dim));
        feel(&mut app, node, true, false);
        assert_eq!(fill(&app, node), GREEN);
    }

    #[test]
    fn a_style_is_a_plain_scope_for_any_view() {
        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            row((label("x"),))
                .rules(style::<Plain>().tone(Tone::Dim).bundle()),
        );
        let text = tests::kids(&app, node)[0];

        assert_eq!(
            app.world().get::<TextColor>(text).unwrap().0,
            Plain::default().tone(Tone::Dim)
        );
    }

    #[test]
    fn a_style_can_wait_on_a_state_of_its_own() {
        use bevy::ecs::component::Component;

        #[derive(Component)]
        struct Selected;

        let mut app = tests::app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(
                style::<Plain>()
                    .when::<Selected>(|s| s.fill(|_| RED))
                    .bundle(),
            ),
        );
        assert_eq!(fill(&app, node), REST);

        app.world_mut().entity_mut(node).insert(Selected);
        app.update();
        app.update();

        assert_eq!(fill(&app, node), RED);
    }
}

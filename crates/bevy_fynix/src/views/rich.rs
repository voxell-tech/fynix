use std::sync::Arc;

use bevy::color::Color;
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::{Component, Mutable};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::world::{EntityWorldMut, World};
use bevy::text::{
    FontSize, FontSource, FontStyle, FontWeight, Strikethrough,
    StrikethroughColor, TextBackgroundColor, TextColor, TextFont,
    TextLayout, TextSpan, Underline, UnderlineColor,
};
use bevy::ui::widget::Text;
use fynix::{Patch, element};

use super::label::PatchWrap;
use crate::patch::{Paint, PatchInk, PatchOpacity, PatchScale};
use crate::prop::Prop;
use crate::state::own_when;
use crate::tokens::{TextTokens, Tone};
use crate::transition::{blend_color, blend_f32};
use crate::visual::faded;
use crate::{Bevy, Styled};

/// Text in more than one style: a [`Text`] with a [`TextSpan`] under
/// it for each span. A new list of spans rewrites only the spans that
/// differ.
#[element(backend = Bevy, theme = TextTokens, prepare = prepare)]
pub struct Rich {
    /// The font of a span with none of its own. Bevy's default font
    /// when unset.
    #[elem(patch = PatchSpanFont)]
    pub font: Prop<FontSource>,
    /// The size of a span with none of its own. The theme's body
    /// size when unset.
    #[elem(
        default = theme.body_size(),
        patch = PatchSpanSize,
        blend = blend_f32
    )]
    pub size: Prop<f32>,
    /// The colour of a span with none of its own, by role. Body when
    /// unset.
    #[elem(
        shown = Color,
        with = |tone, theme| theme.tone(tone),
        patch = PatchSpanInk,
        blend = blend_color
    )]
    pub tone: Prop<Tone>,
    /// Whether it breaks onto more lines. It does when unset.
    #[elem(default = true, patch = PatchWrap)]
    pub wrap: Prop<bool>,
    /// How opaque every span is, 1.0 when unset.
    #[elem(default = 1.0, patch = PatchSpanOpacity, blend = blend_f32)]
    pub opacity: Prop<f32>,
    /// The factor it is scaled by around its centre after layout,
    /// 1.0 when unset.
    #[elem(default = 1.0, patch = PatchScale, blend = blend_f32)]
    pub scale: Prop<f32>,
    #[elem(patch = PatchSpans)]
    pub spans: Prop<Spans>,
}

/// Text made of `spans`, each in its own style.
pub fn rich(spans: impl Into<Prop<Vec<Span>>>) -> Rich {
    Rich {
        spans: spans.into().map(Spans::from),
        ..Rich::unset()
    }
}

own_when!(Rich);

/// A run of text in a [`Rich`], and how it differs from the text
/// around it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub look: Look,
}

/// How a [`Span`] is drawn. Each `None` takes what the [`Rich`] says,
/// or Bevy's default where it says nothing.
///
/// Colours are raw rather than a [`Tone`], for colours the theme has
/// no role for, such as syntax colours.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    /// The rich text's tone when `None`.
    pub color: Option<Color>,
    /// The rich text's font when `None`.
    pub font: Option<FontSource>,
    /// The rich text's size when `None`.
    pub size: Option<f32>,
    pub weight: Option<FontWeight>,
    pub style: Option<FontStyle>,
    /// A fill behind the span, the height of its line.
    pub background: Option<Color>,
    pub underline: Option<LineColor>,
    pub strikethrough: Option<LineColor>,
}

/// The colour of a line drawn under or through a span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineColor {
    /// The span's text colour.
    Ink,
    Own(Color),
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            look: Look::default(),
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.look.color = Some(color);
        self
    }

    pub fn font(mut self, font: impl Into<FontSource>) -> Self {
        self.look.font = Some(font.into());
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.look.size = Some(size);
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.look.weight = Some(weight);
        self
    }

    pub fn bold(self) -> Self {
        self.weight(FontWeight::BOLD)
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.look.style = Some(style);
        self
    }

    pub fn italic(self) -> Self {
        self.style(FontStyle::Italic)
    }

    pub fn background(mut self, color: Color) -> Self {
        self.look.background = Some(color);
        self
    }

    pub fn underline(mut self, color: LineColor) -> Self {
        self.look.underline = Some(color);
        self
    }

    pub fn strikethrough(mut self, color: LineColor) -> Self {
        self.look.strikethrough = Some(color);
        self
    }
}

/// The spans of a [`Rich`]. A clone shares the list, so a list kept
/// and handed back unchanged compares equal without being walked.
#[derive(Clone, Debug, Default)]
pub struct Spans(Arc<[Span]>);

impl From<Vec<Span>> for Spans {
    fn from(spans: Vec<Span>) -> Self {
        Self(spans.into())
    }
}

impl PartialEq for Spans {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

#[derive(Component, Clone, Default)]
struct SpanDefaults {
    font: FontSource,
    size: f32,
}

#[derive(Component)]
struct Drawn(Look);

fn prepare(world: &mut World, node: Entity) {
    world.entity_mut(node).insert((
        Text::default(),
        TextFont::default(),
        TextColor::default(),
        TextLayout::default(),
        Paint::default(),
        SpanDefaults::default(),
    ));
}

fn spans_of(world: &World, root: Entity) -> Vec<Entity> {
    world
        .get::<Children>(root)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&child| world.get::<Drawn>(child).is_some())
        .collect()
}

fn put<C: Component<Mutability = Mutable> + PartialEq>(
    entity: &mut EntityWorldMut,
    value: C,
) {
    match entity.get_mut::<C>() {
        Some(mut current) => {
            current.set_if_neq(value);
        }
        None => {
            entity.insert(value);
        }
    }
}

fn put_or_take<C: Component<Mutability = Mutable> + PartialEq>(
    entity: &mut EntityWorldMut,
    value: Option<C>,
) {
    match value {
        Some(value) => put(entity, value),
        None => {
            entity.remove::<C>();
        }
    }
}

fn mark<C: Component + Default>(
    entity: &mut EntityWorldMut,
    on: bool,
) {
    if on != entity.contains::<C>() {
        if on {
            entity.insert(C::default());
        } else {
            entity.remove::<C>();
        }
    }
}

fn draw(
    entity: &mut EntityWorldMut,
    look: &Look,
    defaults: &SpanDefaults,
    paint: Paint,
) {
    let fade = |color| faded(color, paint.opacity);
    put(
        entity,
        TextFont {
            font: look
                .font
                .clone()
                .unwrap_or_else(|| defaults.font.clone()),
            font_size: FontSize::Px(
                look.size.unwrap_or(defaults.size),
            ),
            weight: look.weight.unwrap_or(FontWeight::NORMAL),
            style: look.style.unwrap_or_default(),
            ..TextFont::default()
        },
    );
    put(entity, TextColor(fade(look.color.unwrap_or(paint.ink))));
    match look.background {
        Some(color) => put(entity, TextBackgroundColor(fade(color))),
        // Bevy redraws a span when its background changes but not
        // when it is removed, so the colour is marked changed.
        None => {
            if entity.take::<TextBackgroundColor>().is_some()
                && let Some(mut ink) = entity.get_mut::<TextColor>()
            {
                ink.set_changed();
            }
        }
    }
    let line_ink = |line: Option<LineColor>| match line {
        Some(LineColor::Own(color)) => Some(fade(color)),
        Some(LineColor::Ink) | None => None,
    };
    mark::<Underline>(entity, look.underline.is_some());
    put_or_take(entity, line_ink(look.underline).map(UnderlineColor));
    mark::<Strikethrough>(entity, look.strikethrough.is_some());
    put_or_take(
        entity,
        line_ink(look.strikethrough).map(StrikethroughColor),
    );
}

fn inherited(
    world: &World,
    root: Entity,
) -> Option<(SpanDefaults, Paint)> {
    Some((
        world.get::<SpanDefaults>(root)?.clone(),
        *world.get::<Paint>(root)?,
    ))
}

pub struct PatchSpans;

impl Patch<Bevy, Spans> for PatchSpans {
    fn patch(world: &mut World, node: Entity, spans: &Spans) {
        let Some((defaults, paint)) = inherited(world, node) else {
            return;
        };
        // An empty list keeps one empty span: a text that loses its
        // last child loses `Children` with it, and Bevy lays a text
        // out again when `Children` changes but not when it is
        // removed, so its old glyphs would stay.
        let empty = [Span::default()];
        let spans = if spans.0.is_empty() {
            &empty[..]
        } else {
            &spans.0[..]
        };
        let shown = spans_of(world, node);
        for (index, span) in spans.iter().enumerate() {
            if let Some(&kid) = shown.get(index) {
                let mut kid = world.entity_mut(kid);
                if let Some(mut text) = kid.get_mut::<TextSpan>()
                    && text.0 != span.text
                {
                    text.0.clone_from(&span.text);
                }
                if let Some(mut drawn) = kid.get_mut::<Drawn>()
                    && drawn.0 != span.look
                {
                    drawn.0.clone_from(&span.look);
                    draw(&mut kid, &span.look, &defaults, paint);
                }
            } else {
                let mut kid = world.spawn((
                    TextSpan(span.text.clone()),
                    Drawn(span.look.clone()),
                    ChildOf(node),
                ));
                draw(&mut kid, &span.look, &defaults, paint);
            }
        }
        for &kid in shown.get(spans.len()..).unwrap_or_default() {
            world.entity_mut(kid).despawn();
        }
    }
}

fn redraw(world: &mut World, root: Entity) {
    let Some((defaults, paint)) = inherited(world, root) else {
        return;
    };
    for kid in spans_of(world, root) {
        let mut kid = world.entity_mut(kid);
        if let Some(look) =
            kid.get::<Drawn>().map(|drawn| drawn.0.clone())
        {
            draw(&mut kid, &look, &defaults, paint);
        }
    }
}

fn set_defaults(
    world: &mut World,
    root: Entity,
    edit: impl FnOnce(&mut SpanDefaults),
) {
    let Some(mut defaults) = world.get_mut::<SpanDefaults>(root)
    else {
        return;
    };
    edit(&mut defaults);
    let defaults = defaults.clone();
    if let Some(mut font) = world.get_mut::<TextFont>(root) {
        font.font = defaults.font;
        font.font_size = FontSize::Px(defaults.size);
    }
    redraw(world, root);
}

pub struct PatchSpanInk;

impl Patch<Bevy, Color> for PatchSpanInk {
    fn patch(world: &mut World, node: Entity, ink: &Color) {
        PatchInk::patch(world, node, ink);
        redraw(world, node);
    }
}

pub struct PatchSpanOpacity;

impl Patch<Bevy, f32> for PatchSpanOpacity {
    fn patch(world: &mut World, node: Entity, opacity: &f32) {
        PatchOpacity::patch(world, node, opacity);
        redraw(world, node);
    }
}

pub struct PatchSpanFont;

impl Patch<Bevy, FontSource> for PatchSpanFont {
    fn patch(world: &mut World, node: Entity, font: &FontSource) {
        set_defaults(world, node, |defaults| {
            defaults.font.clone_from(font);
        });
    }
}

pub struct PatchSpanSize;

impl Patch<Bevy, f32> for PatchSpanSize {
    fn patch(world: &mut World, node: Entity, size: &f32) {
        set_defaults(world, node, |defaults| defaults.size = *size);
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Alpha;
    use bevy::ecs::change_detection::Tick;
    use bevy::ecs::resource::Resource;

    use super::*;
    use crate::tests::{Plain, app, kids};
    use crate::tokens::Motion;
    use crate::{ScopedExt, StateExt, hidden, mount, resource};

    #[derive(Resource)]
    struct Line(Vec<Span>);

    fn words(count: usize) -> Vec<Span> {
        (0..count)
            .map(|index| Span::new(format!("w{index} ")))
            .collect()
    }

    fn line(app: &mut App, spans: Vec<Span>) -> Entity {
        app.insert_resource(Line(spans));
        mount::<Plain>(
            app.world_mut(),
            rich(resource::<Line, _>(|line| line.0.clone())),
        )
    }

    fn set_line(app: &mut App, spans: Vec<Span>) {
        app.world_mut().resource_mut::<Line>().0 = spans;
        app.update();
    }

    fn text(app: &App, span: Entity) -> &str {
        &app.world().get::<TextSpan>(span).expect("a span").0
    }

    fn color(app: &App, span: Entity) -> Color {
        app.world().get::<TextColor>(span).expect("a span").0
    }

    fn changed<C: Component>(app: &App, span: Entity) -> Tick {
        app.world()
            .entity(span)
            .get_change_ticks::<C>()
            .expect("the component")
            .changed
    }

    fn ticks(app: &App, span: Entity) -> [Tick; 3] {
        [
            changed::<TextSpan>(app, span),
            changed::<TextFont>(app, span),
            changed::<TextColor>(app, span),
        ]
    }

    #[test]
    fn editing_one_span_changes_only_its_components() {
        let mut app = app();
        let root = line(&mut app, words(4));
        let spans = kids(&app, root);
        let before = spans
            .iter()
            .map(|&span| ticks(&app, span))
            .collect::<Vec<_>>();

        let mut edited = words(4);
        edited[2] =
            Span::new("two ").color(Color::srgb(1.0, 0.0, 0.0));
        set_line(&mut app, edited);

        assert_eq!(kids(&app, root), spans, "no span respawned");
        for (index, &span) in spans.iter().enumerate() {
            let after = ticks(&app, span);
            if index == 2 {
                assert_ne!(
                    after[0], before[2][0],
                    "the text changed"
                );
                assert_eq!(
                    after[1], before[2][1],
                    "the font did not"
                );
                assert_ne!(
                    after[2], before[2][2],
                    "the colour changed"
                );
            } else {
                assert_eq!(
                    after, before[index],
                    "span {index} untouched"
                );
            }
        }
        assert_eq!(text(&app, spans[2]), "two ");
        assert_eq!(color(&app, spans[2]), Color::srgb(1.0, 0.0, 0.0));
    }

    #[test]
    fn spans_are_spawned_and_despawned_only_at_the_end() {
        let mut app = app();
        let root = line(&mut app, words(3));
        let first = kids(&app, root);

        set_line(&mut app, words(5));
        let grown = kids(&app, root);
        assert_eq!(grown.len(), 5);
        assert_eq!(grown[..3], first[..], "the first 3 kept");
        assert_eq!(text(&app, grown[4]), "w4 ");

        set_line(&mut app, Vec::new());
        assert_eq!(
            kids(&app, root),
            [first[0]],
            "one empty span kept"
        );
        assert_eq!(text(&app, first[0]), "");
        for &gone in &grown[1..] {
            assert!(
                app.world().get_entity(gone).is_err(),
                "despawned"
            );
        }

        set_line(&mut app, words(2));
        assert_eq!(kids(&app, root)[0], first[0]);
        assert_eq!(text(&app, kids(&app, root)[1]), "w1 ");
    }

    #[test]
    fn a_span_loses_what_its_look_no_longer_has() {
        let mut app = app();
        let plated = Span::new("x")
            .background(Color::BLACK)
            .underline(LineColor::Own(Color::WHITE));
        let root = line(&mut app, vec![plated]);
        let span = kids(&app, root)[0];
        assert!(
            app.world().get::<TextBackgroundColor>(span).is_some()
        );
        assert!(app.world().get::<UnderlineColor>(span).is_some());

        set_line(
            &mut app,
            vec![Span::new("x").underline(LineColor::Ink)],
        );

        let world = app.world();
        assert!(world.get::<TextBackgroundColor>(span).is_none());
        assert!(world.get::<Underline>(span).is_some());
        assert!(world.get::<UnderlineColor>(span).is_none());
    }

    #[derive(Resource)]
    struct Shade(Tone);

    #[test]
    fn a_tone_transition_reaches_every_span_without_a_colour() {
        let mut app = app();
        app.insert_resource(Shade(Tone::Body));
        let own = Color::srgb(0.0, 0.0, 1.0);
        let root = mount::<Plain>(
            app.world_mut(),
            rich(vec![
                Span::new("a "),
                Span::new("b ").color(own),
                Span::new("c"),
            ])
            .tone(resource::<Shade, _>(|shade| shade.0))
            .transition(Motion::Interact),
        );
        let spans = kids(&app, root);
        assert_eq!(color(&app, spans[0]), Color::WHITE);

        app.world_mut().resource_mut::<Shade>().0 = Tone::Accent;
        app.update();
        let orange = Color::srgb(1.0, 0.5, 0.0);
        let midway = color(&app, root);
        assert_ne!(midway, Color::WHITE, "50ms of 100ms");
        assert_ne!(midway, orange, "50ms of 100ms");
        assert_eq!(color(&app, spans[0]), midway);
        assert_eq!(color(&app, spans[2]), midway);
        assert_eq!(
            color(&app, spans[1]),
            own,
            "its own colour stays"
        );

        app.update();
        assert_eq!(color(&app, spans[0]), orange);
        assert_eq!(color(&app, spans[2]), orange);
    }

    #[test]
    fn opacity_fades_every_span_and_its_background() {
        let mut app = app();
        let own = Color::srgb(0.0, 0.0, 1.0);
        let plate = Color::srgb(0.2, 0.2, 0.2);
        let root = mount::<Plain>(
            app.world_mut(),
            rich(vec![
                Span::new("a "),
                Span::new("b ").color(own).background(plate),
            ])
            .opacity(0.25),
        );
        let spans = kids(&app, root);

        assert_eq!(
            color(&app, spans[0]),
            Color::WHITE.with_alpha(0.25)
        );
        assert_eq!(color(&app, spans[1]), own.with_alpha(0.25));
        let background =
            app.world().get::<TextBackgroundColor>(spans[1]).unwrap();
        assert_eq!(background.0, plate.with_alpha(0.25));

        let hidden = mount::<Plain>(
            app.world_mut(),
            rich(vec![
                Span::new("a "),
                Span::new("b").color(own).background(plate),
            ])
            .appear::<Plain>(hidden),
        );
        let spans = kids(&app, hidden);
        for &span in &spans {
            assert_eq!(
                color(&app, span).alpha(),
                0.0,
                "built unseen"
            );
        }

        app.update();
        app.update();
        assert_eq!(color(&app, spans[0]), Color::WHITE, "faded in");
        assert_eq!(color(&app, spans[1]), own, "faded in");
        let background =
            app.world().get::<TextBackgroundColor>(spans[1]).unwrap();
        assert_eq!(background.0, plate, "faded in");
    }

    #[derive(Resource)]
    struct Face(f32, &'static str);

    #[test]
    fn the_root_font_and_size_reach_spans_with_none_of_their_own() {
        let mut app = app();
        app.insert_resource(Face(9.0, "Root"));
        let root = mount::<Plain>(
            app.world_mut(),
            rich(vec![
                Span::new("a "),
                Span::new("b")
                    .size(20.0)
                    .font(FontSource::family("Own")),
            ])
            .size(resource::<Face, _>(|face| face.0))
            .font(resource::<Face, _>(|face| {
                FontSource::family(face.1)
            })),
        );
        let spans = kids(&app, root);
        let font = |span| app.world().get::<TextFont>(span).unwrap();

        assert_eq!(font(spans[0]).font_size, FontSize::Px(9.0));
        assert_eq!(font(spans[0]).font, FontSource::family("Root"));
        assert_eq!(font(spans[1]).font_size, FontSize::Px(20.0));
        assert_eq!(font(spans[1]).font, FontSource::family("Own"));

        *app.world_mut().resource_mut::<Face>() = Face(12.0, "Next");
        app.update();
        let font = |span| app.world().get::<TextFont>(span).unwrap();
        assert_eq!(font(spans[0]).font_size, FontSize::Px(12.0));
        assert_eq!(font(spans[0]).font, FontSource::family("Next"));
        assert_eq!(font(spans[1]).font_size, FontSize::Px(20.0));
        assert_eq!(font(spans[1]).font, FontSource::family("Own"));
    }
}

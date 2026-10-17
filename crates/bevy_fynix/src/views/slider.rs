//! A track with a thumb that is dragged along it to choose a number.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query};
use bevy::ecs::world::World;
use bevy::picking::events::{PointerDrag, PointerPress};
use bevy::picking::pointer::PointerButton;
use bevy::ui::{
    AlignItems, ComputedNode, PositionType, UiGlobalTransform,
    UiRect, Val, percent, px,
};
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::prop::{Prop, component};
use crate::tokens::{SpacingTokens, SurfaceTokens};
use crate::views::frame::{Frame, FrameProps, frame};
use crate::{Bevy, Cx, Styled, View};

/// A handler run with the world and the value the slider was asked
/// to take.
type Change = Box<dyn Fn(&mut World, f32) + Send + Sync>;

/// A [`Frame`] that is a track, with a groove along it filled as far
/// as a thumb that stands at the value.
pub struct Slider {
    pub frame: Frame,
    pub value: Prop<f32>,
    range: (f32, f32),
    on_change: Change,
}

/// A [`Slider`] showing `value`, from 0 to 1 unless
/// [`range`](Slider::range) says otherwise. It never moves itself: a
/// press on the track or a drag along it calls the handler of
/// [`on_change`](Slider::on_change) with the value under the
/// pointer, and a bound `value` follows what that writes to the
/// world. The thumb is under the pointer for as long as it is held.
pub fn slider(value: impl Into<Prop<f32>>) -> Slider {
    Slider {
        frame: Frame::unset(),
        value: value.into(),
        range: (0.0, 1.0),
        on_change: Box::new(|_, _| {}),
    }
}

impl Slider {
    /// This, from `least` at the left end to `most` at the right.
    pub fn range(mut self, least: f32, most: f32) -> Self {
        self.range = (least, most);
        self
    }

    /// This, calling `handler` with the value asked for when the
    /// pointer is pressed on it or dragged along it.
    pub fn on_change(
        mut self,
        handler: impl Fn(&mut World, f32) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Box::new(handler);
        self
    }
}

impl FrameProps for Slider {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// On a [`Slider`]'s node: its ends, how wide its thumb is and what
/// runs when it is slid.
#[derive(Component)]
struct Slides {
    range: (f32, f32),
    thumb: f32,
    on_change: Option<Change>,
}

/// On a [`Slider`]'s node: how far along its value is, from 0 to 1.
#[derive(Component, Clone, Copy, PartialEq)]
struct Along(f32);

/// How far along a track `wide`, whose left edge is at `left`, the
/// middle of a thumb `thumb` wide is when it is at `x`.
fn along(x: f32, left: f32, wide: f32, thumb: f32) -> f32 {
    let travel = (wide - thumb).max(1.0);
    ((x - left - thumb / 2.0) / travel).clamp(0.0, 1.0)
}

/// Asks the slider `node` for the value under a pointer at `x`, in
/// the window's logical pixels.
fn slide_to(
    node: Entity,
    x: f32,
    tracks: &Query<(&Slides, &ComputedNode, &UiGlobalTransform)>,
    commands: &mut Commands,
) {
    let Ok((slides, computed, at)) = tracks.get(node) else {
        return;
    };
    let scale = computed.inverse_scale_factor();
    let (_, _, centre) = at.to_scale_angle_translation();
    let wide = computed.size().x * scale;
    let left = centre.x * scale - wide / 2.0;
    let (least, most) = slides.range;
    let value =
        least + (most - least) * along(x, left, wide, slides.thumb);
    commands.queue(move |world: &mut World| {
        let Some(handler) = world
            .get_mut::<Slides>(node)
            .and_then(|mut slides| slides.on_change.take())
        else {
            return;
        };
        handler(world, value);
        if let Some(mut slides) = world.get_mut::<Slides>(node) {
            slides.on_change = Some(handler);
        }
    });
}

fn pressed(
    mut press: On<PointerPress>,
    tracks: Query<(&Slides, &ComputedNode, &UiGlobalTransform)>,
    mut commands: Commands,
) {
    if press.button != PointerButton::Primary {
        return;
    }
    press.propagate(false);
    let x = press.pointer.position.x;
    slide_to(press.event_target(), x, &tracks, &mut commands);
}

fn dragged(
    mut drag: On<PointerDrag>,
    tracks: Query<(&Slides, &ComputedNode, &UiGlobalTransform)>,
    mut commands: Commands,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    drag.propagate(false);
    let x = drag.pointer.position.x;
    slide_to(drag.event_target(), x, &tracks, &mut commands);
}

/// A slider's defaults, for its root frame alone.
fn defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + 'static,
{
    cx.set::<Frame>(|frame, theme: &T| {
        frame
            .width(px(theme.row() * 6.0))
            .height(px(theme.row()))
            .shrink(0.0)
            .align(AlignItems::Center)
    });
}

impl<T> View<Bevy, T> for Slider
where
    T: SurfaceTokens + SpacingTokens + Send + Sync + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let theme = cx.theme();
        let row = theme.row();
        let groove = (row / 7.0).round().max(2.0);
        let thumb = ((row * 0.36).round(), (row * 0.64).round());
        let (well, accent, edge) =
            (theme.fill(), theme.accent(), theme.hairline());
        let (least, most) = self.range;
        // Signed, so a range that falls from left to right fills
        // the same way a press on it reads.
        let span = most - least;
        cx.scope(|cx| {
            cx.defaults(|cx| cx.root(defaults));
            let node = cx.build(self.frame);
            cx.world
                .entity_mut(node)
                .insert((
                    Slides {
                        range: self.range,
                        thumb: thumb.0,
                        on_change: Some(self.on_change),
                    },
                    Along(0.0),
                    EntityCursor(SystemCursorIcon::Pointer),
                ))
                .observe(pressed)
                .observe(dragged);
            cx.effect(
                node,
                self.value,
                move |world, node, &value| {
                    let to = Along(if span == 0.0 {
                        0.0
                    } else {
                        ((value - least) / span).clamp(0.0, 1.0)
                    });
                    if let Ok(mut entity) = world.get_entity_mut(node)
                        && entity.get::<Along>() != Some(&to)
                    {
                        entity.insert(to);
                    }
                },
            );
            let share = move |along: Option<&Along>| {
                along.map_or(0.0, |along| along.0) * 100.0
            };
            let middle = move |tall: f32| px((row - tall) / 2.0);
            cx.under(node, |cx| {
                cx.build(
                    frame()
                        .width(percent(100.0))
                        .height(px(groove))
                        .radius(groove / 2.0)
                        .fill(well)
                        .border(1.0)
                        .border_color(edge),
                );
                cx.build(
                    frame()
                        .position(PositionType::Absolute)
                        .inset(UiRect {
                            left: px(0.0),
                            top: middle(groove),
                            ..UiRect::all(Val::Auto)
                        })
                        .width(component::<Along, _>(
                            node,
                            move |along| percent(share(along)),
                        ))
                        .height(px(groove))
                        .radius(groove / 2.0)
                        .fill(accent),
                );
                // The thumb travels the track less its own width.
                let travel = cx.build(
                    frame().position(PositionType::Absolute).inset(
                        UiRect {
                            right: px(thumb.0),
                            ..UiRect::all(px(0.0))
                        },
                    ),
                );
                cx.under(travel, |cx| {
                    cx.build(
                        frame()
                            .position(PositionType::Absolute)
                            .inset(component::<Along, _>(
                                node,
                                move |along| UiRect {
                                    left: percent(share(along)),
                                    top: middle(thumb.1),
                                    ..UiRect::all(Val::Auto)
                                },
                            ))
                            .width(px(thumb.0))
                            .height(px(thumb.1))
                            .radius(2.0)
                            .fill(accent),
                    );
                });
            });
            node
        })
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::resource::Resource;
    use bevy::math::Vec2;

    use super::*;
    use crate::tests::{self, Plain};
    use crate::{mount, resource};

    #[derive(Resource)]
    struct Level(f32);

    /// A slider bound to [`Level`], laid out 160 wide from 100.
    fn mounted(app: &mut App, range: (f32, f32)) -> Entity {
        let node = mount::<Plain>(
            app.world_mut(),
            slider(resource::<Level, _>(|level| level.0))
                .range(range.0, range.1)
                .on_change(|world, value| {
                    world.resource_mut::<Level>().0 = value;
                }),
        );
        app.world_mut().entity_mut(node).insert((
            ComputedNode {
                size: Vec2::new(160.0, 28.0),
                inverse_scale_factor: 1.0,
                ..ComputedNode::default()
            },
            UiGlobalTransform::from_xy(180.0, 14.0),
        ));
        node
    }

    fn press(app: &mut App, node: Entity, x: f32) -> f32 {
        tests::pointer_press(
            app,
            node,
            PointerButton::Primary,
            Vec2::new(x, 14.0),
        );
        app.update();
        app.world().resource::<Level>().0
    }

    #[test]
    fn a_press_asks_for_the_value_under_the_pointer() {
        let mut app = tests::app();
        app.insert_resource(Level(0.25));
        let node = mounted(&mut app, (0.0, 1.0));
        let thumb = app.world().get::<Slides>(node).unwrap().thumb;

        // The middle of the track is half, wherever it was before.
        assert_eq!(press(&mut app, node, 180.0), 0.5);
        assert_eq!(app.world().get::<Along>(node).unwrap().0, 0.5);
        // The thumb's middle is half its width in from either end,
        // and past an end is that end.
        assert_eq!(press(&mut app, node, 100.0 + thumb / 2.0), 0.0);
        assert_eq!(press(&mut app, node, 260.0 - thumb / 2.0), 1.0);
        assert_eq!(press(&mut app, node, 0.0), 0.0);
        assert_eq!(press(&mut app, node, 900.0), 1.0);
    }

    #[test]
    fn its_ends_are_those_of_its_range() {
        let mut app = tests::app();
        app.insert_resource(Level(15.0));
        let node = mounted(&mut app, (10.0, 20.0));
        app.update();

        assert_eq!(app.world().get::<Along>(node).unwrap().0, 0.5);
        assert_eq!(press(&mut app, node, 900.0), 20.0);
        assert_eq!(press(&mut app, node, 0.0), 10.0);
    }

    #[test]
    fn a_range_that_falls_fills_as_far_as_its_value() {
        let mut app = tests::app();
        app.insert_resource(Level(0.75));
        let node = mounted(&mut app, (1.0, 0.0));
        app.update();

        assert_eq!(app.world().get::<Along>(node).unwrap().0, 0.25);
        assert_eq!(press(&mut app, node, 900.0), 0.0);
        assert_eq!(app.world().get::<Along>(node).unwrap().0, 1.0);
    }
}

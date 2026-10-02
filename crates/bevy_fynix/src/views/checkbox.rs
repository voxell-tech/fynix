//! A small box that shows a mark while checked.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::world::World;
use bevy::ui::{AlignItems, Checked, Display, JustifyContent, px};
use bevy::ui_widgets::{Checkbox as CheckboxBehavior, ValueChange};
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::prop::{Prop, component};
use crate::state::State;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens,
};
use crate::views::frame::{Frame, FrameProps, frame};
use crate::{Bevy, Cx, Hovered, Styled, View};

/// A handler run with the world and the state the box was asked to
/// take.
type Change = Box<dyn Fn(&mut World, bool) + Send + Sync>;

/// A [`Frame`] on [`bevy::ui_widgets::Checkbox`], with a mark in it
/// shown while the node holds [`Checked`].
pub struct Checkbox {
    pub frame: Frame,
    pub checked: Prop<bool>,
    on_change: Change,
}

/// A [`Checkbox`] showing `checked`. It never toggles itself: a
/// click or key press calls the handler of
/// [`on_change`](Checkbox::on_change) with the state asked for, and
/// a bound `checked` follows what that writes to the world.
pub fn checkbox(checked: impl Into<Prop<bool>>) -> Checkbox {
    Checkbox {
        frame: Frame::unset(),
        checked: checked.into(),
        on_change: Box::new(|_, _| {}),
    }
}

impl Checkbox {
    /// This, calling `handler` with the state asked for when it is
    /// activated.
    pub fn on_change(
        mut self,
        handler: impl Fn(&mut World, bool) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Box::new(handler);
        self
    }
}

impl FrameProps for Checkbox {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// On a [`Checkbox`]'s node: what runs when it is activated.
#[derive(Component)]
struct ChangeHandler(Change);

/// The length of a side of the box.
fn side<T: SpacingTokens>(theme: &T) -> f32 {
    theme.row() * 0.8
}

/// A checkbox's defaults, for its root frame alone.
fn defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    cx.set::<Frame>(|frame, theme: &T| {
        frame
            .width(px(side(theme)))
            .height(px(side(theme)))
            .shrink(0.0)
            .radius(theme.radius())
            .fill(theme.fill())
            .justify(JustifyContent::Center)
            .align(AlignItems::Center)
    });
    cx.when::<State<Hovered>>(|cx| {
        cx.set::<Frame>(|frame, theme: &T| frame.fill(theme.hover()));
    });
    cx.transition(Motion::Interact);
}

fn changed(change: On<ValueChange<bool>>, mut commands: Commands) {
    let node = change.event_target();
    let value = change.value;
    commands.queue(move |world: &mut World| {
        let Some(ChangeHandler(handler)) = world
            .get_entity_mut(node)
            .ok()
            .and_then(|mut entity| entity.take::<ChangeHandler>())
        else {
            return;
        };
        handler(world, value);
        if let Ok(mut entity) = world.get_entity_mut(node) {
            entity.insert(ChangeHandler(handler));
        }
    });
}

impl<T> View<Bevy, T> for Checkbox
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let theme = cx.theme();
        let mark = side(theme) / 2.0;
        let (accent, radius) = (theme.accent(), theme.radius() / 2.0);
        cx.scope(|cx| {
            cx.defaults(|cx| cx.root(defaults));
            let node = cx.build(self.frame);
            cx.world
                .entity_mut(node)
                .insert((
                    CheckboxBehavior,
                    EntityCursor(SystemCursorIcon::Pointer),
                    ChangeHandler(self.on_change),
                ))
                .observe(changed);
            cx.effect(node, self.checked, |world, node, &checked| {
                let Ok(mut entity) = world.get_entity_mut(node)
                else {
                    return;
                };
                if checked {
                    if !entity.contains::<Checked>() {
                        entity.insert(Checked);
                    }
                } else {
                    entity.remove::<Checked>();
                }
            });
            cx.under(node, |cx| {
                cx.build(
                    frame()
                        .width(px(mark))
                        .height(px(mark))
                        .radius(radius)
                        .fill(accent)
                        .display(component::<Checked, _>(
                            node,
                            |checked| match checked {
                                Some(_) => Display::Flex,
                                None => Display::None,
                            },
                        )),
                );
            });
            node
        })
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::resource::Resource;
    use bevy::ui::{BackgroundColor, Node};

    use super::*;
    use crate::tests::{self, Plain};
    use crate::{mount, resource};

    const HOVER: Color = Color::srgb(0.3, 0.3, 0.3);
    const ACCENT: Color = Color::srgb(0.9, 0.5, 0.1);

    /// The state the app says the box is in.
    #[derive(Resource)]
    struct Flag(bool);

    /// The states the box asked for, in order.
    #[derive(Resource, Default)]
    struct Asked(Vec<bool>);

    fn app() -> App {
        let mut app = tests::app_with(Plain {
            radius: 4.0,
            duration: Duration::ZERO,
            accent: Some(ACCENT),
            ..Plain::default()
        });
        app.insert_resource(Flag(false)).init_resource::<Asked>();
        app
    }

    fn mark(app: &App, node: Entity) -> Entity {
        app.world().get::<Children>(node).unwrap()[0]
    }

    fn shown(app: &App, node: Entity) -> bool {
        let mark = mark(app, node);
        app.world().get::<Node>(mark).unwrap().display
            != Display::None
    }

    fn checked(app: &App, node: Entity) -> bool {
        app.world().get::<Checked>(node).is_some()
    }

    fn ask(app: &mut App, node: Entity, value: bool) {
        app.world_mut().trigger(ValueChange {
            source: node,
            value,
            is_final: true,
        });
        app.update();
    }

    #[test]
    fn a_bound_value_decides_whether_it_is_checked_and_marked() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            checkbox(resource::<Flag, _>(|flag| flag.0)),
        );
        assert!(!checked(&app, node));
        assert!(!shown(&app, node));

        app.world_mut().resource_mut::<Flag>().0 = true;
        app.update();
        assert!(checked(&app, node));
        assert!(shown(&app, node));

        app.world_mut().resource_mut::<Flag>().0 = false;
        app.update();
        assert!(!checked(&app, node));
        assert!(!shown(&app, node));
    }

    #[test]
    fn a_value_that_starts_true_is_checked_at_once() {
        let mut app = app();
        let node = mount::<Plain>(app.world_mut(), checkbox(true));

        assert!(checked(&app, node));
        assert!(shown(&app, node));
    }

    #[test]
    fn activating_it_calls_the_handler_with_the_state_asked_for() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            checkbox(false).on_change(|world, value| {
                world.resource_mut::<Asked>().0.push(value);
            }),
        );

        ask(&mut app, node, true);
        ask(&mut app, node, false);

        assert_eq!(app.world().resource::<Asked>().0, [true, false]);
        assert!(!checked(&app, node), "it does not toggle itself");
    }

    #[test]
    fn a_handler_that_writes_the_bound_value_checks_the_box() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            checkbox(resource::<Flag, _>(|flag| flag.0)).on_change(
                |world, value| {
                    world.resource_mut::<Flag>().0 = value;
                },
            ),
        );

        ask(&mut app, node, true);
        assert!(checked(&app, node));
        assert!(shown(&app, node));

        ask(&mut app, node, false);
        assert!(!checked(&app, node));
    }

    #[test]
    fn hovering_lights_the_box_even_over_a_call_site_fill() {
        let mut app = app();
        let node = mount::<Plain>(app.world_mut(), checkbox(false));
        let own = mount::<Plain>(
            app.world_mut(),
            checkbox(false).fill(Color::BLACK),
        );

        for node in [node, own] {
            app.world_mut().entity_mut(node).insert(Hovered);
        }
        app.update();

        let fill = |node| {
            app.world().get::<BackgroundColor>(node).unwrap().0
        };
        assert_eq!(fill(node), HOVER);
        assert_eq!(
            fill(own),
            HOVER,
            "a state rule beats the call site"
        );
    }
}

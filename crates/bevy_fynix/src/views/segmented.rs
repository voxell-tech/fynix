//! A row of mutually exclusive options.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{QueryState, With};
use bevy::ecs::system::Local;
use bevy::ecs::world::World;
use bevy::ui::{BorderRadius, Overflow, Val, percent, px};
use fynix::Condition;

use crate::prop::Prop;
use crate::state::DirtyNodes;
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
};
use crate::views::button::segment;
use crate::views::frame::{Frame, FrameProps};
use crate::views::{BehaviorExt, Label, button, label};
use crate::{Bevy, Cx, ScopedExt, Styled, View};

/// A handler run with the world and the index of the segment picked.
type Select = Box<dyn Fn(&mut World, usize) + Send + Sync>;

/// A [`Frame`] holding one button per option, the one at the index
/// `active` names filled with the theme's accent.
pub struct Segmented {
    pub frame: Frame,
    pub options: Vec<String>,
    pub active: Prop<usize>,
    on_select: Select,
}

/// A [`Segmented`] row of `options`. Activating a segment calls
/// `on_select` with its index. `active` is not changed by the row
/// itself, so a bound one follows whatever `on_select` writes to the
/// world, and the segments are restyled in place.
pub fn segmented<S: Into<String>>(
    options: impl IntoIterator<Item = S>,
    active: impl Into<Prop<usize>>,
    on_select: impl Fn(&mut World, usize) + Send + Sync + 'static,
) -> Segmented {
    Segmented {
        frame: Frame::unset(),
        options: options.into_iter().map(Into::into).collect(),
        active: active.into(),
        on_select: Box::new(on_select),
    }
}

impl FrameProps for Segmented {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// On a [`Segmented`]'s root node: where its active index comes
/// from, and the segment it names now.
#[derive(Component)]
pub(crate) struct Segments {
    active: Prop<usize>,
    chosen: usize,
    on_select: Select,
}

/// A segment is the one its row's root names as active.
struct Chosen;

impl Condition<Bevy> for Chosen {
    fn holds(world: &World, node: Entity) -> bool {
        let Some(root) =
            world.get::<ChildOf>(node).map(ChildOf::parent)
        else {
            return false;
        };
        let (Some(segments), Some(children)) = (
            world.get::<Segments>(root),
            world.get::<Children>(root),
        ) else {
            return false;
        };
        children.iter().position(|&child| child == node)
            == Some(segments.chosen)
    }

    // A segment is reported by the system that moves the choice.
    fn watch(_: &mut World, _: Entity) {}
}

/// What a segment looks like while it is the active one.
fn lit<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + 'static,
{
    cx.root(|cx| {
        cx.set::<Frame>(|frame, theme: &T| {
            frame.fill(theme.accent())
        });
    });
    cx.set::<Label>(|label, _| label.bold(true));
}

/// Rounded on the sides of a row's own ends, square between.
fn corners(index: usize, count: usize, radius: Val) -> BorderRadius {
    match (index == 0, index + 1 == count) {
        (true, true) => BorderRadius::all(radius),
        (true, false) => BorderRadius::left(radius),
        (false, true) => BorderRadius::right(radius),
        (false, false) => BorderRadius::ZERO,
    }
}

fn select(world: &mut World, root: Entity, index: usize) {
    let Some(segments) = world
        .get_entity_mut(root)
        .ok()
        .and_then(|mut entity| entity.take::<Segments>())
    else {
        return;
    };
    (segments.on_select)(world, index);
    if let Ok(mut entity) = world.get_entity_mut(root) {
        entity.insert(segments);
    }
}

impl<T> View<Bevy, T> for Segmented
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
        let count = self.options.len();
        let radius = px(cx.theme().radius());
        let chosen = self.active.get(cx.world).unwrap_or(usize::MAX);
        cx.scope(|cx| {
            cx.defaults(|cx| {
                cx.root(|cx| {
                    cx.set::<Frame>(|frame, theme: &T| {
                        frame
                            .width(percent(100.0))
                            .gap(1.0)
                            .radius(theme.radius())
                            .overflow(Overflow::clip())
                    });
                });
            });
            let root = cx.build(self.frame);
            cx.world.entity_mut(root).insert(Segments {
                active: self.active,
                chosen,
                on_select: self.on_select,
            });
            cx.under(root, |cx| {
                for (index, text) in
                    self.options.into_iter().enumerate()
                {
                    cx.build(
                        button(label(text))
                            .corners(corners(index, count, radius))
                            .rules(segment(false))
                            .when_in::<Chosen, _>(lit)
                            .on_activate(move |world| {
                                select(world, root, index);
                            }),
                    );
                }
            });
            root
        })
    }
}

/// Moves the choice of every [`Segmented`] whose bound `active`
/// changed, and restyles the segment it leaves and the one it lands
/// on.
pub(crate) fn sync_segments(
    world: &mut World,
    mut roots: Local<QueryState<Entity, With<Segments>>>,
) {
    let roots = roots.iter(world).collect::<Vec<_>>();
    for root in roots {
        let Some(mut segments) = world
            .get_entity_mut(root)
            .ok()
            .and_then(|mut entity| entity.take::<Segments>())
        else {
            continue;
        };
        let moved = segments
            .active
            .changed(world)
            .then(|| segments.active.get(world))
            .flatten()
            .filter(|&now| now != segments.chosen);
        let before = segments.chosen;
        if let Some(now) = moved {
            segments.chosen = now;
        }
        world.entity_mut(root).insert(segments);
        let Some(now) = moved else {
            continue;
        };
        let touched = [before, now]
            .into_iter()
            .filter_map(|index| {
                world.get::<Children>(root)?.get(index).copied()
            })
            .collect::<Vec<_>>();
        world.resource_mut::<DirtyNodes>().0.extend(touched);
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::text::{FontWeight, TextFont};
    use bevy::time::TimePlugin;
    use bevy::ui::widget::Text;
    use bevy::ui::{BackgroundColor, Node};
    use bevy::ui_widgets::Activate;

    use super::*;
    use crate::tokens::{Curve, Motion, Tone};
    use crate::{FynixPlugin, Theme, mount, resource};

    struct Plain;

    impl SpacingTokens for Plain {
        fn gap(&self) -> f32 {
            6.0
        }

        fn row(&self) -> f32 {
            20.0
        }

        fn radius(&self) -> f32 {
            3.0
        }
    }

    impl SurfaceTokens for Plain {
        fn fill(&self) -> Color {
            REST
        }

        fn hover(&self) -> Color {
            Color::srgb(0.3, 0.3, 0.3)
        }

        fn panel(&self) -> Color {
            Color::BLACK
        }

        fn accent(&self) -> Color {
            ACCENT
        }
    }

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

    impl MotionTokens for Plain {
        fn motion(&self, _: Motion) -> Curve {
            Curve {
                duration: core::time::Duration::ZERO,
                ease: |t| t,
            }
        }
    }

    const REST: Color = Color::srgb(0.2, 0.2, 0.2);
    const ACCENT: Color = Color::srgb(0.9, 0.5, 0.1);

    /// The segment the app says is active.
    #[derive(Resource)]
    struct Which(usize);

    /// The indices the row reported, in order.
    #[derive(Resource, Default)]
    struct Picked(Vec<usize>);

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain))
        .insert_resource(Which(1))
        .init_resource::<Picked>();
        app
    }

    fn row_of_three() -> Segmented {
        segmented(
            ["a", "b", "c"],
            resource::<Which, _>(|which| which.0),
            |world, index| {
                world.resource_mut::<Picked>().0.push(index);
            },
        )
    }

    fn segments(app: &App, root: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(root)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    fn fills(app: &App, root: Entity) -> Vec<Color> {
        segments(app, root)
            .into_iter()
            .map(|node| {
                app.world().get::<BackgroundColor>(node).unwrap().0
            })
            .collect()
    }

    fn weight(app: &App, segment: Entity) -> FontWeight {
        let label = segments(app, segment)[0];
        app.world().get::<TextFont>(label).unwrap().weight
    }

    #[test]
    fn it_builds_one_segment_per_option_with_the_bound_one_active() {
        let mut app = app();
        let root = mount::<Plain>(app.world_mut(), row_of_three());

        let nodes = segments(&app, root);
        let texts = nodes
            .iter()
            .map(|&node| {
                let label = segments(&app, node)[0];
                app.world().get::<Text>(label).unwrap().0.clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(texts, ["a", "b", "c"]);
        assert_eq!(fills(&app, root), [REST, ACCENT, REST]);
        assert_eq!(weight(&app, nodes[1]), FontWeight::BOLD);
        assert_eq!(weight(&app, nodes[0]), FontWeight::NORMAL);
    }

    #[test]
    fn the_ends_are_rounded_and_the_row_is_clipped() {
        let mut app = app();
        let root = mount::<Plain>(app.world_mut(), row_of_three());

        let radius = |node| {
            app.world().get::<Node>(node).unwrap().border_radius
        };
        let nodes = segments(&app, root);
        assert_eq!(radius(nodes[0]), BorderRadius::left(px(3.0)));
        assert_eq!(radius(nodes[1]), BorderRadius::ZERO);
        assert_eq!(radius(nodes[2]), BorderRadius::right(px(3.0)));
        let ui = app.world().get::<Node>(root).unwrap();
        assert_eq!(ui.overflow, Overflow::clip());
    }

    #[test]
    fn activating_a_segment_calls_the_handler_with_its_index() {
        let mut app = app();
        let root = mount::<Plain>(app.world_mut(), row_of_three());
        let nodes = segments(&app, root);

        for index in [2, 0, 1] {
            app.world_mut().trigger(Activate {
                entity: nodes[index],
            });
        }
        app.update();

        assert_eq!(app.world().resource::<Picked>().0, [2, 0, 1]);
    }

    #[test]
    fn the_active_look_moves_with_the_bound_value_without_a_rebuild()
    {
        let mut app = app();
        let root = mount::<Plain>(app.world_mut(), row_of_three());
        let before = segments(&app, root);

        app.world_mut().resource_mut::<Which>().0 = 2;
        app.update();

        assert_eq!(segments(&app, root), before, "same nodes");
        assert_eq!(fills(&app, root), [REST, REST, ACCENT]);
        assert_eq!(weight(&app, before[1]), FontWeight::NORMAL);
        assert_eq!(weight(&app, before[2]), FontWeight::BOLD);
    }

    #[test]
    fn a_handler_that_writes_the_bound_value_moves_the_look() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            segmented(
                ["a", "b", "c"],
                resource::<Which, _>(|which| which.0),
                |world, index| {
                    world.resource_mut::<Which>().0 = index
                },
            ),
        );
        let nodes = segments(&app, root);

        app.world_mut().trigger(Activate { entity: nodes[0] });
        app.update();

        assert_eq!(fills(&app, root), [ACCENT, REST, REST]);
    }

    #[test]
    fn a_fixed_active_index_is_lit_and_none_when_out_of_range() {
        let mut app = app();
        let first = mount::<Plain>(
            app.world_mut(),
            segmented(["a", "b"], 0, |_, _| {}),
        );
        let none = mount::<Plain>(
            app.world_mut(),
            segmented(["a", "b"], 7, |_, _| {}),
        );

        assert_eq!(fills(&app, first), [ACCENT, REST]);
        assert_eq!(fills(&app, none), [REST, REST]);
    }
}

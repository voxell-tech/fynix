//! A surface floating over everything else, hung off what opened it.

use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::world::World;
use bevy::math::Rect;
use bevy::picking::Pickable;
use bevy::picking::events::{Pointer, Press};
use bevy::ui::{OverrideClip, PositionType, UiRect, Val, px};
use bevy::ui_widgets::popover::{
    Popover, PopoverAlign, PopoverPlacement, PopoverSide,
};

use crate::tokens::{SpacingTokens, SurfaceTokens};
use crate::views::frame::{Frame, FrameProps};
use crate::views::menu::{MENU_Z, menu_surface};
use crate::views::stack::{column, overlay, row};
use crate::{Bevy, Cx, ScopedExt, Styled, View, ViewSeq};

/// What runs when a press lands outside a [`Popup`], given its root.
type Dismiss = fn(&mut World, Entity);

/// `content` in a column on a menu surface, hung off an anchor in its
/// parent and placed by [`Popover`] where the window cuts off least
/// of it, over a backdrop filling the parent. A press on the backdrop
/// runs the dismiss handler, which despawns the popup unless set. Its
/// frame styles the surface.
pub struct Popup<C> {
    pub frame: Frame,
    content: C,
    anchor: Rect,
    placements: Vec<PopoverPlacement>,
    on_dismiss: Dismiss,
}

/// A [`Popup`] of `content` against `anchor`, in its parent's logical
/// pixels: the rect of what opened it, or a point as a zero-size
/// rect.
pub fn popup<C>(anchor: Rect, content: C) -> Popup<C> {
    Popup {
        frame: Frame::unset(),
        content,
        anchor,
        placements: corners(0.0),
        on_dismiss: |world, root| {
            if let Ok(entity) = world.get_entity_mut(root) {
                entity.despawn();
            }
        },
    }
}

impl<C> Popup<C> {
    /// Where the surface may sit against its anchor, the one the
    /// window cuts off least winning. [`corners`] when unset.
    pub fn placements(
        mut self,
        placements: Vec<PopoverPlacement>,
    ) -> Self {
        self.placements = placements;
        self
    }

    /// What a press on the backdrop runs instead of despawning the
    /// popup.
    pub fn on_dismiss(mut self, on_dismiss: Dismiss) -> Self {
        self.on_dismiss = on_dismiss;
        self
    }
}

impl<C> FrameProps for Popup<C> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// Below the anchor, then above it, each lined up with its left edge,
/// then its right, `gap` away from it.
pub fn corners(gap: f32) -> Vec<PopoverPlacement> {
    use PopoverAlign::{End, Start};
    use PopoverSide::{Bottom, Top};

    [(Bottom, Start), (Bottom, End), (Top, Start), (Top, End)]
        .into_iter()
        .map(|(side, align)| PopoverPlacement { side, align, gap })
        .collect()
}

impl<T, C> View<Bevy, T> for Popup<C>
where
    T: SurfaceTokens + SpacingTokens + Send + Sync + 'static,
    C: ViewSeq<Bevy, T> + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let Self {
            frame,
            content,
            anchor,
            placements,
            on_dismiss,
        } = self;
        let window_margin = cx.theme().menu_margin();
        let root = cx.build(
            overlay(()).with(Pickable::default()).z(Some(MENU_Z - 1)),
        );
        cx.world.entity_mut(root).observe(
            move |press: On<Pointer<Press>>,
                  mut commands: Commands| {
                if press.original_event_target()
                    != press.event_target()
                {
                    return;
                }
                let root = press.event_target();
                commands.queue(move |world: &mut World| {
                    on_dismiss(world, root);
                });
            },
        );
        cx.under(root, |cx| {
            let anchor = cx.build(
                row(())
                    .position(PositionType::Absolute)
                    .inset(UiRect::new(
                        px(anchor.min.x),
                        Val::Auto,
                        px(anchor.min.y),
                        Val::Auto,
                    ))
                    .width(px(anchor.width()))
                    .height(px(anchor.height())),
            );
            cx.under(anchor, |cx| {
                let mut surface = column(content);
                surface.frame = frame;
                cx.build(
                    surface
                        .with((
                            Popover {
                                positions: placements,
                                window_margin,
                            },
                            OverrideClip,
                        ))
                        .rules(|cx: &mut Cx<'_, Bevy, T>| {
                            cx.defaults(menu_surface);
                        }),
                )
            });
        });
        root
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::math::Vec2;
    use bevy::picking::pointer::PointerButton;
    use bevy::ui::Node;

    use super::*;
    use crate::mount;
    use crate::tests::{self, Plain};
    use crate::views::label;

    #[derive(Resource, Default)]
    struct Dismissed(u32);

    fn kids(app: &App, node: Entity) -> Vec<Entity> {
        app.world().get::<Children>(node).unwrap().iter().collect()
    }

    fn press(app: &mut App, node: Entity) {
        tests::pointer_press(
            app,
            node,
            PointerButton::Primary,
            Vec2::ZERO,
        );
        app.update();
    }

    fn mounted(app: &mut App) -> (Entity, Entity) {
        app.init_resource::<Dismissed>();
        let root = mount::<Plain>(
            app.world_mut(),
            popup(Rect::new(10.0, 20.0, 40.0, 44.0), (label("Cut"),))
                .on_dismiss(|world, _| {
                    world.resource_mut::<Dismissed>().0 += 1;
                }),
        );
        let anchor = kids(app, root)[0];
        (root, kids(app, anchor)[0])
    }

    #[test]
    fn the_surface_hangs_off_its_anchor() {
        let mut app = tests::app();
        let (root, _) = mounted(&mut app);

        let anchor = kids(&app, root)[0];
        let ui = app.world().get::<Node>(anchor).unwrap();
        assert_eq!((ui.left, ui.top), (px(10.0), px(20.0)));
        assert_eq!((ui.width, ui.height), (px(30.0), px(24.0)));
    }

    #[test]
    fn a_press_on_the_backdrop_dismisses_it_and_one_inside_does_not()
    {
        let mut app = tests::app();
        let (root, surface) = mounted(&mut app);

        press(&mut app, surface);
        assert_eq!(app.world().resource::<Dismissed>().0, 0);

        press(&mut app, root);
        assert_eq!(app.world().resource::<Dismissed>().0, 1);
    }

    #[test]
    fn unset_dismissal_despawns_it() {
        let mut app = tests::app();
        let root = mount::<Plain>(
            app.world_mut(),
            popup(Rect::default(), (label("Cut"),)),
        );

        press(&mut app, root);

        assert!(app.world().get_entity(root).is_err());
    }
}

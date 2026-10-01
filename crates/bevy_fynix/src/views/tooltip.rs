//! A view shown near the pointer after it rests on another.

use core::marker::PhantomData;
use core::time::Duration;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::Has;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::Vec2;
use bevy::picking::pointer::PointerLocation;
use bevy::time::Time;
use bevy::ui::{OverrideClip, UiRect, UiScale, px};
use bevy::ui_widgets::popover::{
    Popover, PopoverAlign, PopoverPlacement, PopoverSide,
};
use fynix::Condition;

use crate::prop::component;
use crate::state::State;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens,
};
use crate::views::menu::{TOOLTIP_Z, float, menu_surface};
use crate::views::{FrameProps, row};
use crate::{
    AnyView, Bevy, Cx, Hovered, ScopedExt, StateExt, View, hidden,
};

/// How long the pointer rests on a source before its tooltip shows,
/// and how long it stays away from the source and the tooltip before
/// the tooltip goes.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooltipTiming {
    pub show: Duration,
    pub hide: Duration,
}

impl Default for TooltipTiming {
    fn default() -> Self {
        Self {
            show: Duration::from_millis(500),
            hide: Duration::from_millis(150),
        }
    }
}

/// A view whose root node shows a tooltip while the pointer rests on
/// it.
pub struct Tooltip<V, F, W> {
    inner: V,
    build: F,
    content: PhantomData<fn() -> W>,
}

/// What any view can be given a tooltip with.
pub trait TooltipExt: Sized {
    /// This view, showing the view `build` returns near the pointer
    /// on a menu surface, once the pointer has rested on it for
    /// [`TooltipTiming::show`]. It stays while the pointer is on this
    /// view or on the tooltip, and fades out
    /// [`TooltipTiming::hide`] after the pointer is on neither.
    /// `build` runs each time the tooltip shows.
    ///
    /// ```ignore
    /// button(label("Save")).tooltip(|| label("Ctrl+S"))
    /// ```
    fn tooltip<F, W>(self, build: F) -> Tooltip<Self, F, W>
    where
        F: Fn() -> W + Send + Sync + 'static,
    {
        Tooltip {
            inner: self,
            build,
            content: PhantomData,
        }
    }
}

impl<V> TooltipExt for V {}

/// On a source node: the node its tooltip hangs from, and how long
/// the pointer has been resting on it, or away.
#[derive(Component)]
pub(crate) struct Source {
    anchor: Entity,
    resting: Duration,
    away: Duration,
}

/// On a source node while its tooltip is shown: where the pointer was
/// when it showed, in logical pixels.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShownAt(Vec2);

/// Shows and hides each source's tooltip as the pointer rests on it
/// and leaves it.
pub(crate) fn tick(
    time: Res<Time>,
    timing: Res<TooltipTiming>,
    scale: Option<Res<UiScale>>,
    pointers: Query<&PointerLocation>,
    hovers: Query<Has<Hovered>>,
    mut sources: Query<(Entity, &mut Source, Has<ShownAt>)>,
    mut commands: Commands,
) {
    let delta = time.delta();
    let scale = scale.map_or(1.0, |scale| scale.0);
    let pointer = pointers
        .iter()
        .find_map(PointerLocation::location)
        .map(|location| location.position / scale);
    let hovered =
        |node| hovers.get(node).is_ok_and(|hovered| hovered);

    for (node, mut source, shown) in &mut sources {
        let over = hovered(node);
        if !shown {
            if !over {
                source.resting = Duration::ZERO;
                continue;
            }
            source.resting += delta;
            if source.resting >= timing.show
                && let Some(at) = pointer
            {
                commands.entity(node).insert(ShownAt(at));
                source.away = Duration::ZERO;
            }
        } else if over || hovered(source.anchor) {
            source.away = Duration::ZERO;
        } else {
            source.away += delta;
            if source.away >= timing.hide {
                commands.entity(node).remove::<ShownAt>();
                source.resting = Duration::ZERO;
            }
        }
    }
}

fn placement(
    side: PopoverSide,
    align: PopoverAlign,
) -> PopoverPlacement {
    PopoverPlacement {
        side,
        align,
        gap: 16.0,
    }
}

/// `content` on a menu surface below the pointer, or above it, or
/// beside it where the window has no room. It fades in and out.
fn surface<T, W>(content: W) -> AnyView<Bevy, T>
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    W: View<Bevy, T> + 'static,
{
    use PopoverAlign::{End, Start};
    use PopoverSide::{Bottom, Top};

    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let padding = cx.theme().menu_padding();
        let window_margin = cx.theme().menu_margin();
        cx.build(
            row((content,))
                .padding(UiRect::axes(px(padding * 2.0), px(padding)))
                .z(Some(TOOLTIP_Z))
                .with((
                    Popover {
                        positions: vec![
                            placement(Bottom, Start),
                            placement(Bottom, End),
                            placement(Top, Start),
                            placement(Top, End),
                        ],
                        window_margin,
                    },
                    OverrideClip,
                ))
                .rules(|cx: &mut Cx<'_, Bevy, T>| {
                    cx.defaults(menu_surface);
                })
                .appear::<T>(hidden)
                .transition(Motion::Interact),
        )
    })
}

impl<T, V, F, W> View<Bevy, T> for Tooltip<V, F, W>
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    V: View<Bevy, T>,
    F: Fn() -> W + Send + Sync + 'static,
    W: View<Bevy, T> + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let source = self.inner.build(cx);
        let build = self.build;
        let anchor = float::<T, ShownAt, ShownAt, ()>(
            cx,
            source,
            |shown| shown.0,
            component::<ShownAt, _>(source, |shown| {
                shown.map(|shown| vec![*shown]).unwrap_or_default()
            }),
            |_| (),
            move |_| surface(build()),
        );
        // The tooltip's own hover, from the surface in it.
        for node in [source, anchor] {
            <State<Hovered> as Condition<Bevy>>::watch(
                cx.world, node,
            );
        }
        cx.world.entity_mut(source).insert(Source {
            anchor,
            resting: Duration::ZERO,
            away: Duration::ZERO,
        });
        source
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::pointer::Location;
    use bevy::ui::GlobalZIndex;
    use bevy::ui::widget::Text;

    use super::*;
    use crate::mount;
    use crate::testing::{self, Plain, hover, kids};
    use crate::views::label;

    fn app() -> App {
        let mut app = testing::app();
        app.insert_resource(TooltipTiming {
            show: Duration::from_millis(100),
            hide: Duration::from_millis(100),
        });
        app.world_mut().spawn(PointerLocation::new(Location {
            target: NormalizedRenderTarget::None {
                width: 800,
                height: 600,
            },
            position: Vec2::new(30.0, 40.0),
        }));
        app
    }

    fn source(app: &mut App) -> Entity {
        mount::<Plain>(
            app.world_mut(),
            label("Save").tooltip(|| label("Ctrl+S")),
        )
    }

    fn anchor(app: &App, source: Entity) -> Entity {
        app.world().get::<Source>(source).unwrap().anchor
    }

    #[test]
    fn nothing_shows_before_the_pointer_has_rested() {
        let mut app = app();
        let source = source(&mut app);
        let anchor = anchor(&app, source);

        hover(&mut app, source, true);
        // 50ms of the 100ms delay.
        app.update();

        assert!(kids(&app, anchor).is_empty());
        assert!(app.world().get::<ShownAt>(source).is_none());

        app.update();
        app.update();
        assert_eq!(kids(&app, anchor).len(), 1);
    }

    #[test]
    fn it_shows_near_the_pointer_after_the_delay() {
        let mut app = app();
        let source = source(&mut app);
        let anchor = anchor(&app, source);

        hover(&mut app, source, true);
        for _ in 0..3 {
            app.update();
        }

        assert_eq!(
            app.world().get::<ShownAt>(source),
            Some(&ShownAt(Vec2::new(30.0, 40.0)))
        );
        let surface = kids(&app, anchor);
        assert_eq!(surface.len(), 1);
        let text = kids(&app, surface[0])[0];
        assert_eq!(
            app.world().get::<Text>(text).unwrap().0,
            "Ctrl+S"
        );
        assert_eq!(
            app.world().get::<GlobalZIndex>(surface[0]),
            Some(&GlobalZIndex(TOOLTIP_Z))
        );
        assert!(app.world().get::<Popover>(surface[0]).is_some());
        let ui = app.world().get::<bevy::ui::Node>(anchor).unwrap();
        assert_eq!((ui.left, ui.top), (px(30.0), px(40.0)));
    }

    #[test]
    fn leaving_before_the_delay_starts_it_over() {
        let mut app = app();
        let source = source(&mut app);

        hover(&mut app, source, true);
        app.update();
        hover(&mut app, source, false);
        app.update();
        hover(&mut app, source, true);
        app.update();
        assert!(app.world().get::<ShownAt>(source).is_none());

        app.update();
        app.update();
        assert!(app.world().get::<ShownAt>(source).is_some());
    }

    #[test]
    fn it_goes_when_the_pointer_leaves_the_source() {
        let mut app = app();
        let source = source(&mut app);
        let anchor = anchor(&app, source);
        hover(&mut app, source, true);
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(kids(&app, anchor).len(), 1);

        hover(&mut app, source, false);
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<ShownAt>(source).is_none());

        // It fades out, then its space closes.
        for _ in 0..8 {
            app.update();
        }
        assert!(kids(&app, anchor).is_empty());
    }

    #[test]
    fn it_stays_while_the_pointer_is_on_the_tooltip() {
        let mut app = app();
        let source = source(&mut app);
        let anchor = anchor(&app, source);
        hover(&mut app, source, true);
        for _ in 0..3 {
            app.update();
        }

        hover(&mut app, source, false);
        hover(&mut app, anchor, true);
        for _ in 0..6 {
            app.update();
        }
        assert!(app.world().get::<ShownAt>(source).is_some());
        assert_eq!(kids(&app, anchor).len(), 1);

        hover(&mut app, anchor, false);
        for _ in 0..3 {
            app.update();
        }
        assert!(app.world().get::<ShownAt>(source).is_none());
    }

    #[test]
    fn it_can_show_again() {
        let mut app = app();
        let source = source(&mut app);
        let anchor = anchor(&app, source);
        hover(&mut app, source, true);
        for _ in 0..3 {
            app.update();
        }
        hover(&mut app, source, false);
        for _ in 0..12 {
            app.update();
        }
        assert!(kids(&app, anchor).is_empty());

        hover(&mut app, source, true);
        for _ in 0..4 {
            app.update();
        }

        assert_eq!(kids(&app, anchor).len(), 1);
    }

    #[test]
    fn it_goes_with_its_source() {
        let mut app = app();
        let source = source(&mut app);
        let anchor = anchor(&app, source);

        app.world_mut().entity_mut(source).despawn();
        app.update();
        app.update();

        assert!(app.world().get_entity(anchor).is_err());
    }
}

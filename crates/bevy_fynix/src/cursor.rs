//! A cursor per entity, shown while the mouse is over it.

use bevy::app::{App, Plugin, PreUpdate};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::picking::PickingSystems;
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

/// The cursor shown while the mouse is over this entity or one of its
/// descendants.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityCursor(pub SystemCursorIcon);

/// A cursor that wins over every hover while it is `Some`, such as
/// for the length of a drag.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverrideCursor(pub Option<SystemCursorIcon>);

/// Sets the primary window's [`CursorIcon`] from the hovered entity's
/// [`EntityCursor`], or from [`OverrideCursor`].
pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OverrideCursor>().add_systems(
            PreUpdate,
            update_cursor.after(PickingSystems::Hover),
        );
    }
}

fn update_cursor(
    hover_map: Option<Res<HoverMap>>,
    forced: Res<OverrideCursor>,
    parents: Query<&ChildOf>,
    cursors: Query<&EntityCursor>,
    windows: Query<
        (Entity, Option<&CursorIcon>),
        With<PrimaryWindow>,
    >,
    mut commands: Commands,
) {
    let hovered = hover_map.as_deref().and_then(topmost);
    let icon = forced.0.unwrap_or_else(|| {
        hovered
            .and_then(|entity| cursor_for(entity, &parents, &cursors))
            .unwrap_or_default()
    });
    let wanted = CursorIcon::System(icon);

    for (window, current) in &windows {
        if current.unwrap_or(&CursorIcon::default()) != &wanted {
            commands.entity(window).insert(wanted.clone());
        }
    }
}

/// The mouse's nearest hit.
fn topmost(hover_map: &HoverMap) -> Option<Entity> {
    hover_map
        .get(&PointerId::Mouse)?
        .iter()
        .min_by(|(_, a), (_, b)| a.depth.total_cmp(&b.depth))
        .map(|(entity, _)| *entity)
}

/// The cursor of `hovered` or its nearest ancestor that has one.
fn cursor_for(
    hovered: Entity,
    parents: &Query<&ChildOf>,
    cursors: &Query<&EntityCursor>,
) -> Option<SystemCursorIcon> {
    core::iter::once(hovered)
        .chain(parents.iter_ancestors(hovered))
        .find_map(|entity| cursors.get(entity).ok())
        .map(|cursor| cursor.0)
}

#[cfg(test)]
mod tests {
    use bevy::ecs::hierarchy::Children;
    use bevy::picking::backend::HitData;
    use bevy::window::Window;

    use super::*;
    use crate::mount;
    use crate::testing::{self, Plain};
    use crate::views::{button, frame};

    /// A window and a button over a frame.
    fn setup() -> (App, Entity, Entity, Entity) {
        let mut app = testing::app();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let node = mount::<Plain>(app.world_mut(), button(frame()));
        let child = app.world().get::<Children>(node).unwrap()[0];
        (app, window, node, child)
    }

    fn hover(app: &mut App, hovered: Option<Entity>) {
        let mut map = HoverMap::default();
        if let Some(entity) = hovered {
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

    fn cursor(app: &App, window: Entity) -> Option<CursorIcon> {
        app.world().get::<CursorIcon>(window).cloned()
    }

    #[test]
    fn a_button_carries_the_pointer_cursor() {
        let (app, _, node, _) = setup();

        assert_eq!(
            app.world().get::<EntityCursor>(node),
            Some(&EntityCursor(SystemCursorIcon::Pointer))
        );
    }

    #[test]
    fn hovering_a_button_or_its_child_shows_the_pointer() {
        let pointer =
            Some(CursorIcon::System(SystemCursorIcon::Pointer));
        let (mut app, window, node, child) = setup();

        hover(&mut app, Some(node));
        assert_eq!(cursor(&app, window), pointer);

        hover(&mut app, None);
        assert_eq!(cursor(&app, window), Some(CursorIcon::default()));

        hover(&mut app, Some(child));
        assert_eq!(cursor(&app, window), pointer);
    }

    #[test]
    fn hovering_nothing_leaves_the_default() {
        let (mut app, window, _, _) = setup();

        hover(&mut app, None);

        assert!(
            cursor(&app, window)
                .is_none_or(|icon| icon == CursorIcon::default())
        );
    }

    #[test]
    fn an_entity_with_no_cursor_shows_the_default() {
        let (mut app, window, _, _) = setup();
        let other = app.world_mut().spawn_empty().id();

        hover(&mut app, Some(other));

        assert!(
            cursor(&app, window)
                .is_none_or(|icon| icon == CursorIcon::default())
        );
    }

    #[test]
    fn an_override_wins_over_the_hover_until_cleared() {
        let pointer =
            Some(CursorIcon::System(SystemCursorIcon::Pointer));
        let resize =
            Some(CursorIcon::System(SystemCursorIcon::EwResize));
        let (mut app, window, node, _) = setup();
        hover(&mut app, Some(node));
        assert_eq!(cursor(&app, window), pointer);

        app.world_mut().resource_mut::<OverrideCursor>().0 =
            Some(SystemCursorIcon::EwResize);
        hover(&mut app, Some(node));
        assert_eq!(cursor(&app, window), resize);

        hover(&mut app, None);
        assert_eq!(cursor(&app, window), resize, "off the field");

        app.world_mut().resource_mut::<OverrideCursor>().0 = None;
        hover(&mut app, Some(node));
        assert_eq!(cursor(&app, window), pointer);
    }
}

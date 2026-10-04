//! Moving the focus between fields with Tab.

use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy::input_focus::{FocusCause, FocusedInput, InputFocus};
use bevy::ui::{Display, Node};

/// What a node is to a Tab: whether it takes the focus, and whether
/// it and what is under it are on screen.
type Stop<'a> = (
    Option<&'a TabIndex>,
    Option<&'a Node>,
    Option<&'a Visibility>,
    Option<&'a Children>,
);

/// Moves the focus on Tab to the next node with a [`TabIndex`], and
/// on Shift+Tab to the one before, among those on screen under the
/// nearest [`TabGroup`] around the focus, in tree order.
pub(crate) fn tab(
    mut event: On<FocusedInput<KeyboardInput>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    groups: Query<(), With<TabGroup>>,
    parents: Query<&ChildOf>,
    stops: Query<Stop>,
    mut focus: ResMut<InputFocus>,
) {
    let focused = event.focused_entity;
    let key = &event.input;
    // The event bubbles, and this answers it once.
    if event.event_target() != focused
        || key.key_code != KeyCode::Tab
        || key.state != ButtonState::Pressed
        || key.repeat
    {
        return;
    }
    let Some(group) = core::iter::once(focused)
        .chain(parents.iter_ancestors(focused))
        .find(|&node| groups.contains(node))
    else {
        return;
    };
    let mut found = Vec::new();
    gather(group, &groups, &stops, &mut found);
    let Some(at) = found.iter().position(|&node| node == focused)
    else {
        return;
    };
    let back = keys.is_some_and(|keys| {
        keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
    });
    let step = if back { found.len() - 1 } else { 1 };
    let next = found[(at + step) % found.len()];
    event.propagate(false);
    if next != focused {
        focus.set(next, FocusCause::Navigated);
    }
}

/// The nodes under `node` that take the focus, in tree order. What
/// is out of the layout or hidden is skipped with all under it, and
/// so is a group of its own.
fn gather(
    node: Entity,
    groups: &Query<(), With<TabGroup>>,
    stops: &Query<Stop>,
    found: &mut Vec<Entity>,
) {
    let Ok((index, ui, visible, children)) = stops.get(node) else {
        return;
    };
    if ui.is_some_and(|ui| ui.display == Display::None)
        || visible == Some(&Visibility::Hidden)
    {
        return;
    }
    if index.is_some_and(|index| index.0 >= 0) {
        found.push(node);
    }
    for &child in children.into_iter().flatten() {
        if !groups.contains(child) {
            gather(child, groups, stops, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::input::keyboard::Key;

    use super::*;
    use crate::mount;
    use crate::tests::{self, Plain, kids};
    use crate::views::{BehaviorExt, FrameProps, column, frame};

    #[test]
    fn tab_moves_to_the_next_field_on_screen_and_wraps() {
        let mut app = tests::app();
        tests::keyboard(&mut app);
        let field = || frame().tagged(TabIndex(0));
        let root = mount::<Plain>(
            app.world_mut(),
            column((
                field(),
                column((field(),)).display(Display::None),
                field(),
            )),
        );
        app.update();
        let [first, _, last] = kids(&app, root)[..] else {
            panic!("three children");
        };
        let focused =
            |app: &App| app.world().resource::<InputFocus>().get();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(first, FocusCause::Navigated);

        tests::key_down(&mut app, KeyCode::Tab, Key::Tab);
        assert_eq!(focused(&app), Some(last), "past the hidden one");

        tests::key_down(&mut app, KeyCode::Tab, Key::Tab);
        assert_eq!(focused(&app), Some(first));
    }
}

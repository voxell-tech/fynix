//! [`Interactive`] for [`Bevy`]: the states are this crate's state
//! components, activation is
//! [`Activate`](bevy::ui_widgets::Activate), and semantics go to
//! AccessKit through [`AccessibilityNode`].

use accesskit::Role as AccessRole;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::ui_widgets::{Button, MenuItem};
use fynix_interact::{Action, Interactive, Role, Semantics};

use crate::Bevy;
use crate::state::{Focused, Hovered, Pressed, State};
use crate::views::behavior::add_activate;

impl Interactive for Bevy {
    type Hovered = State<Hovered>;
    type Pressed = State<Pressed>;
    type Focused = State<Focused>;

    fn on_activate(
        world: &mut World,
        node: Entity,
        action: Action<Self>,
    ) {
        // Bevy only activates a button or a menu item, from a click
        // or from Enter or Space while it has focus.
        if let Ok(mut entity) = world.get_entity_mut(node)
            && !entity.contains::<Button>()
            && !entity.contains::<MenuItem>()
        {
            entity.insert(Button);
        }
        add_activate(
            world,
            node,
            Box::new(move |world, _| action(world)),
        );
    }

    fn describe(
        world: &mut World,
        node: Entity,
        semantics: Semantics,
    ) {
        let Ok(mut entity) = world.get_entity_mut(node) else {
            return;
        };
        let role = access_role(semantics.role);
        // A widget of Bevy's own already has one.
        let mut accessible =
            entity.take::<AccessibilityNode>().unwrap_or_else(|| {
                AccessibilityNode(accesskit::Node::new(role))
            });
        accessible.set_role(role);
        match semantics.label {
            Some(label) => accessible.set_label(label),
            None => accessible.clear_label(),
        }
        entity.insert(accessible);
    }
}

/// AccessKit's name for `role`.
fn access_role(role: Role) -> AccessRole {
    match role {
        Role::Button => AccessRole::Button,
        Role::CheckBox => AccessRole::CheckBox,
        Role::RadioButton => AccessRole::RadioButton,
        Role::Switch => AccessRole::Switch,
        Role::Slider => AccessRole::Slider,
        Role::TextInput => AccessRole::TextInput,
        Role::ComboBox => AccessRole::ComboBox,
        Role::Label => AccessRole::Label,
        Role::Image => AccessRole::Image,
        Role::List => AccessRole::List,
        Role::ListItem => AccessRole::ListItem,
        Role::Menu => AccessRole::Menu,
        Role::MenuItem => AccessRole::MenuItem,
        Role::TabList => AccessRole::TabList,
        Role::Tab => AccessRole::Tab,
        Role::TabPanel => AccessRole::TabPanel,
        Role::Dialog => AccessRole::Dialog,
        Role::Tooltip => AccessRole::Tooltip,
        Role::Group => AccessRole::Group,
        _ => AccessRole::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use bevy::color::Color;
    use bevy::ecs::resource::Resource;
    use bevy::math::Vec2;
    use bevy::picking::pointer::PointerButton;
    use bevy::text::TextColor;
    use bevy::ui_widgets::{Activate, ButtonPlugin};
    use fynix::{Cx, ScopedExt, View};
    use fynix_interact::{Hovered as AnyHovered, InteractExt};

    use super::*;
    use crate::mount;
    use crate::tests::{Plain, app, pointer_click, pointer_press};
    use crate::tokens::Tone;
    use crate::views::{Label, label};

    #[derive(Resource, Default)]
    struct Activations(u32);

    /// A view that does something when activated, named for screen
    /// readers and restyled while hovered, written for any
    /// interactive backend.
    fn pressable<B: Interactive, T: 'static>(
        view: impl View<B, T>,
        hover: impl FnOnce(&mut Cx<'_, B, T>) + 'static,
        action: impl Fn(&mut B::World) + Send + Sync + 'static,
    ) -> impl View<B, T> {
        view.when_in::<AnyHovered, _>(hover)
            .described(Role::Button, "Save")
            .on_activate(action)
    }

    fn color(app: &bevy::app::App, node: Entity) -> Color {
        app.world().get::<TextColor>(node).expect("a label").0
    }

    #[test]
    fn a_view_written_for_any_interactive_backend_works_on_bevy() {
        let mut app = app();
        app.add_plugins(ButtonPlugin).init_resource::<Activations>();
        let node = mount::<Plain>(
            app.world_mut(),
            pressable(
                label("Save"),
                |cx: &mut Cx<'_, Bevy, Plain>| {
                    cx.set::<Label>(|label, _| {
                        label.tone(Tone::Accent)
                    });
                },
                |world: &mut World| {
                    world.resource_mut::<Activations>().0 += 1;
                },
            ),
        );
        app.update();

        let accessible = app
            .world()
            .get::<AccessibilityNode>(node)
            .expect("described");
        assert_eq!(accessible.role(), AccessRole::Button);
        assert_eq!(accessible.label(), Some("Save"));

        let rest = color(&app, node);
        app.world_mut().entity_mut(node).insert(Hovered);
        app.update();
        assert_ne!(color(&app, node), rest, "restyled while hovered");

        // A plain label is activated by a click like a button.
        pointer_press(
            &mut app,
            node,
            PointerButton::Primary,
            Vec2::ZERO,
        );
        app.update();
        pointer_click(&mut app, node);
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, 1);
    }

    #[test]
    fn an_action_given_while_activating_runs_from_the_next_activation()
     {
        let mut app = app();
        app.init_resource::<Activations>();
        let node = mount::<Plain>(app.world_mut(), label("x"));
        let first = |world: &mut World| {
            world.resource_mut::<Activations>().0 += 1;
        };
        <Bevy as Interactive>::on_activate(
            app.world_mut(),
            node,
            Box::new(move |world: &mut World| {
                first(world);
                <Bevy as Interactive>::on_activate(
                    world,
                    node,
                    Box::new(|world: &mut World| {
                        world.resource_mut::<Activations>().0 += 10;
                    }),
                );
            }),
        );
        let activate = |app: &mut bevy::app::App| {
            app.world_mut().trigger(Activate { entity: node });
            app.update();
        };

        activate(&mut app);
        assert_eq!(app.world().resource::<Activations>().0, 1);
        activate(&mut app);
        // The first again, giving a third, and the second once.
        assert_eq!(
            app.world().resource::<Activations>().0,
            1 + 1 + 10
        );
    }
}

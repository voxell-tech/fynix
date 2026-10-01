//! What the tests share: a theme to build under, a headless app for
//! it, and the lookups every test module reaches for.

use core::time::Duration;

use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::relationship::RelationshipTarget;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonState, InputPlugin};
use bevy::input_focus::InputDispatchPlugin;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use bevy::window::{PrimaryWindow, Window};

use crate::state::{Hovered, Pressed};
use crate::tokens::{
    Curve, Motion, MotionTokens, SpacingTokens, SurfaceTokens,
    TextTokens, Tone,
};
use crate::{FynixPlugin, Theme};

const ORANGE: Color = Color::srgb(1.0, 0.5, 0.0);

/// A theme with a value for every token, none of them special.
/// A test that reads a token sets it here, and leaves the rest.
#[derive(Clone, Debug)]
pub struct Plain {
    /// The corner radius.
    pub radius: f32,
    /// How long a transition takes.
    pub duration: Duration,
    /// Panels and popups.
    pub panel: Color,
    /// The theme's own accent surface, or the trait's default.
    pub accent: Option<Color>,
    /// The theme's own hairline, or the trait's default.
    pub hairline: Option<Color>,
    /// The theme's own menu corner radius, or the trait's default.
    pub menu_radius: Option<f32>,
    /// The colour of [`Tone::Accent`] text, or orange.
    pub accent_tone: Option<Color>,
    /// The colour of [`Tone::OnAccent`] text, or white.
    pub on_accent: Option<Color>,
}

impl Default for Plain {
    fn default() -> Self {
        Self {
            radius: 3.0,
            duration: Duration::from_millis(100),
            panel: Color::BLACK,
            accent: None,
            hairline: None,
            menu_radius: None,
            accent_tone: None,
            on_accent: None,
        }
    }
}

impl TextTokens for Plain {
    fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Body => Color::WHITE,
            Tone::Dim | Tone::Faint => Color::srgb(0.5, 0.5, 0.5),
            Tone::Accent => self.accent_tone.unwrap_or(ORANGE),
            Tone::OnAccent => self.on_accent.unwrap_or(Color::WHITE),
            Tone::Critical => ORANGE,
        }
    }

    fn body_size(&self) -> f32 {
        14.0
    }

    fn small_size(&self) -> f32 {
        11.0
    }
}

impl SurfaceTokens for Plain {
    fn fill(&self) -> Color {
        Color::srgb(0.2, 0.2, 0.2)
    }

    fn hover(&self) -> Color {
        Color::srgb(0.3, 0.3, 0.3)
    }

    fn panel(&self) -> Color {
        self.panel
    }

    fn hairline(&self) -> Color {
        self.hairline.unwrap_or_else(|| self.hover())
    }

    fn accent(&self) -> Color {
        self.accent.unwrap_or_else(|| self.selection())
    }
}

impl SpacingTokens for Plain {
    fn gap(&self) -> f32 {
        6.0
    }

    fn row(&self) -> f32 {
        20.0
    }

    fn radius(&self) -> f32 {
        self.radius
    }

    fn menu_radius(&self) -> f32 {
        self.menu_radius.unwrap_or(self.radius)
    }
}

impl MotionTokens for Plain {
    fn motion(&self, _: Motion) -> Curve {
        Curve {
            duration: self.duration,
            ease: |t| t,
        }
    }
}

/// A headless app under the default [`Plain`].
pub fn app() -> App {
    app_with(Plain::default())
}

/// A headless app under `theme`, whose clock moves 50ms per update.
pub fn app_with(theme: Plain) -> App {
    let mut app = App::new();
    app.add_plugins((TimePlugin, FynixPlugin::<Plain>::default()))
        .insert_resource(Theme(theme))
        .insert_resource(TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(50),
        ));
    // The first update only starts the clock.
    app.update();
    app
}

/// The nodes directly under `node`, in order.
pub fn kids(app: &App, node: Entity) -> Vec<Entity> {
    app.world()
        .get::<Children>(node)
        .map(|children| children.iter().collect())
        .unwrap_or_default()
}

/// Puts the pointer over `node` or takes it off.
pub fn hover(app: &mut App, node: Entity, on: bool) {
    let mut node = app.world_mut().entity_mut(node);
    if on {
        node.insert(Hovered);
    } else {
        node.remove::<Hovered>();
    }
}

/// Sends the event picking sends for `button` going down at `at` on
/// `on`, which bubbles up from it as a real one does. Nothing else
/// reacts to it but what listens for it.
pub fn pointer_press(
    app: &mut App,
    on: Entity,
    button: PointerButton,
    at: Vec2,
) {
    let location = Location {
        target: NormalizedRenderTarget::None {
            width: 800,
            height: 600,
        },
        position: at,
    };
    let hit = HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
    app.world_mut().trigger(Pointer::new(
        PointerId::Mouse,
        location,
        Press {
            button,
            hit,
            count: 1,
        },
        on,
    ));
}

/// Gives `app` a primary window and what routes key presses to the
/// node holding the focus, as `DefaultPlugins` does.
pub fn keyboard(app: &mut App) {
    app.add_plugins((InputPlugin, InputDispatchPlugin));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
}

/// Presses `code` and runs a frame, for an app given [`keyboard`].
pub fn key_down(app: &mut App, code: KeyCode, key: Key) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

/// Puts a pointer button down on `node` or lets it up.
pub fn press(app: &mut App, node: Entity, on: bool) {
    let mut node = app.world_mut().entity_mut(node);
    if on {
        node.insert(Pressed);
    } else {
        node.remove::<Pressed>();
    }
}

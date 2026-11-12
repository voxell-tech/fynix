//! A header with a body that can be folded away.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::ui::{AlignItems, Display, FlexDirection, percent};

use crate::prop::component;
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
};
use crate::views::{
    BehaviorExt, FrameProps, button, frame, icon, row, tint,
};
use crate::{Bevy, Cx, ScopedExt, View, icons};

/// On a [`Foldable`]'s root node while its body is shown.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Open;

/// How far the up-pointing chevron turns while the body is shown, in
/// degrees, so it points down.
const OPEN_TURN: f32 = 180.0;
/// How far it turns while the body is folded, so it points right.
const SHUT_TURN: f32 = 90.0;

/// A chevron and `header` in a row, over a `body` shown while the
/// root node holds [`Open`]. The chevron is a [`tint`] button.
pub struct Foldable<H, B> {
    pub header: H,
    pub body: B,
    pub open: bool,
}

pub fn foldable<H, B>(header: H, body: B) -> Foldable<H, B> {
    Foldable {
        header,
        body,
        open: true,
    }
}

impl<H, B> Foldable<H, B> {
    /// Whether the body starts shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
}

fn toggle(world: &mut World, root: Entity) {
    let Ok(mut entity) = world.get_entity_mut(root) else {
        return;
    };
    if entity.contains::<Open>() {
        entity.remove::<Open>();
    } else {
        entity.insert(Open);
    }
}

impl<T, H, B> View<Bevy, T> for Foldable<H, B>
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    H: View<Bevy, T>,
    B: View<Bevy, T>,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let root = cx.build(frame().direction(FlexDirection::Column));
        if self.open {
            cx.world.entity_mut(root).insert(Open);
        }
        cx.under(root, |cx| {
            let turn = component::<Open, _>(root, |open| {
                if open.is_some() { OPEN_TURN } else { SHUT_TURN }
            });
            let chevron = button(
                icon(icons::CHEVRON)
                    .size(cx.theme().small_size())
                    .rotation(turn)
                    .transition(T::interact),
            )
            .rules(tint)
            .on_activate(move |world| toggle(world, root));
            cx.build(
                row((chevron, self.header)).align(AlignItems::Center),
            );
            // Hidden, and out of the layout, while the root is shut.
            let body = cx.build(
                frame()
                    .direction(FlexDirection::Column)
                    .width(percent(100.0))
                    .display(component::<Open, _>(root, |open| {
                        match open {
                            Some(_) => Display::Flex,
                            None => Display::None,
                        }
                    })),
            );
            cx.under(body, |cx| self.body.build(cx));
        });
        root
    }
}

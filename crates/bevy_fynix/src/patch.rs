//! How each prop is written onto its entity: one [`Patch`] per prop,
//! named by the prop's `#[elem(patch = ..)]`.
//!
//! [`Patch`]: fynix::Patch

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Rot2, Vec2};
use bevy::text::TextColor;
use bevy::ui::widget::ImageNode;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display,
    FlexDirection, GlobalZIndex, JustifyContent, Overflow,
    PositionType, UiRect, UiTransform, Val, px,
};

use crate::visual::faded;

/// A [`Patch`](fynix::Patch) that edits the entity, if it is still
/// there.
macro_rules! patch {
    (
        $(#[$meta:meta])*
        $name:ident, $ty:ty, |$entity:ident, $value:ident| $body:expr
    ) => {
        $(#[$meta])*
        pub struct $name;

        impl fynix::Patch<$crate::Bevy, $ty> for $name {
            fn patch(
                world: &mut bevy::ecs::world::World,
                node: bevy::ecs::entity::Entity,
                $value: &$ty,
            ) {
                if let Ok(mut $entity) = world.get_entity_mut(node) {
                    $body
                }
            }
        }
    };
}

/// A [`patch!`] that sets one field of the entity's
/// [`Node`](bevy::ui::Node).
macro_rules! node_patch {
    (
        $(#[$meta:meta])*
        $name:ident, $ty:ty, |$ui:ident, $value:ident| $body:expr
    ) => {
        $crate::patch::patch!(
            $(#[$meta])*
            $name, $ty, |entity, $value| {
                if let Some(mut $ui) =
                    entity.get_mut::<bevy::ui::Node>()
                {
                    $body
                }
            }
        );
    };
}

/// A [`node_patch!`] for a field that decides how much space the node
/// takes, which a hold or a collapse writes instead while it runs. A
/// held node keeps the value for when it is released.
macro_rules! size_patch {
    (
        $(#[$meta:meta])*
        $name:ident, $ty:ty, |$ui:ident, $value:ident| $body:expr
    ) => {
        $crate::patch::patch!(
            $(#[$meta])*
            $name, $ty, |entity, $value| {
                let held = $crate::leave::write_held(
                    &mut entity,
                    |$ui: &mut bevy::ui::Node| {
                        $body;
                    },
                );
                if !held
                    && !entity.contains::<$crate::leave::Collapsing>()
                    && let Some(mut $ui) =
                        entity.get_mut::<bevy::ui::Node>()
                {
                    $body
                }
            }
        );
    };
}

pub(crate) use patch;

node_patch!(PatchDirection, FlexDirection, |ui, v| ui
    .flex_direction =
    *v);
node_patch!(PatchGap, f32, |ui, v| {
    ui.row_gap = px(*v);
    ui.column_gap = px(*v);
});
size_patch!(PatchPadding, UiRect, |ui, v| ui.padding = *v);
size_patch!(PatchMargin, UiRect, |ui, v| ui.margin = *v);
size_patch!(PatchWidth, Val, |ui, v| ui.width = *v);
size_patch!(PatchHeight, Val, |ui, v| ui.height = *v);
size_patch!(PatchMinWidth, Val, |ui, v| ui.min_width = *v);
size_patch!(PatchMinHeight, Val, |ui, v| ui.min_height = *v);
node_patch!(PatchMaxWidth, Val, |ui, v| ui.max_width = *v);
node_patch!(PatchMaxHeight, Val, |ui, v| ui.max_height = *v);
node_patch!(PatchGrow, f32, |ui, v| ui.flex_grow = *v);
size_patch!(PatchShrink, f32, |ui, v| ui.flex_shrink = *v);
node_patch!(PatchJustify, JustifyContent, |ui, v| ui
    .justify_content =
    *v);
node_patch!(PatchAlign, AlignItems, |ui, v| ui.align_items = *v);
size_patch!(PatchPosition, PositionType, |ui, v| ui.position_type =
    *v);
size_patch!(PatchInset, UiRect, |ui, v| {
    ui.left = v.left;
    ui.right = v.right;
    ui.top = v.top;
    ui.bottom = v.bottom;
});
size_patch!(PatchOverflow, Overflow, |ui, v| ui.overflow = *v);
node_patch!(PatchDisplay, Display, |ui, v| ui.display = *v);
node_patch!(PatchBorder, f32, |ui, v| ui.border =
    UiRect::all(px(*v)));
size_patch!(
    /// A square: one value for both sides.
    PatchSquare,
    f32,
    |ui, v| {
        ui.width = px(*v);
        ui.height = px(*v);
    }
);

patch!(
    /// `None` leaves the node in its parent's stack.
    PatchZ,
    Option<i32>,
    |entity, v| {
        match v {
            Some(z) => entity.insert(GlobalZIndex(*z)),
            None => entity.remove::<GlobalZIndex>(),
        };
    }
);

/// The value of a corners prop that is left unset.
pub const NO_CORNERS: BorderRadius = BorderRadius::all(Val::Auto);

/// The rounding a node is given, kept so `radius` and `corners` can
/// be written in any order: the corners win while they are set.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Rounding {
    pub radius: f32,
    pub corners: Option<BorderRadius>,
}

/// Edits the entity's [`Rounding`], then writes it to the node.
fn round(
    entity: &mut EntityWorldMut,
    edit: impl FnOnce(&mut Rounding),
) {
    let mut rounding =
        entity.get::<Rounding>().copied().unwrap_or_default();
    edit(&mut rounding);
    entity.insert(rounding);
    if let Some(mut ui) = entity.get_mut::<bevy::ui::Node>() {
        ui.border_radius = rounding
            .corners
            .unwrap_or(BorderRadius::all(px(rounding.radius)));
    }
}

patch!(PatchRadius, f32, |entity, v| {
    round(&mut entity, |rounding| rounding.radius = *v);
});
patch!(
    /// [`NO_CORNERS`] leaves the node to its `radius`.
    PatchCorners,
    BorderRadius,
    |entity, v| {
        round(&mut entity, |rounding| {
            rounding.corners = (*v != NO_CORNERS).then_some(*v);
        });
    }
);

/// The colours an element draws with, before its opacity. Bevy has no
/// opacity of its own for a node, so each colour is written with the
/// opacity multiplied into its alpha, and kept here to be written
/// again when the opacity changes.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub opacity: f32,
    pub fill: Color,
    pub border: Color,
    /// The text colour, or an image's tint.
    pub ink: Color,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            fill: Color::NONE,
            border: Color::NONE,
            ink: Color::NONE,
        }
    }
}

/// Edits the entity's [`Paint`], then writes every colour it draws
/// with at the paint's opacity.
fn repaint(
    entity: &mut EntityWorldMut,
    edit: impl FnOnce(&mut Paint),
) {
    let mut paint =
        entity.get::<Paint>().copied().unwrap_or_default();
    edit(&mut paint);
    entity.insert(paint);
    let Paint {
        opacity,
        fill,
        border,
        ink,
    } = paint;
    if let Some(mut background) = entity.get_mut::<BackgroundColor>()
    {
        background.0 = faded(fill, opacity);
    }
    if let Some(mut edge) = entity.get_mut::<BorderColor>() {
        *edge = BorderColor::all(faded(border, opacity));
    }
    if let Some(mut text) = entity.get_mut::<TextColor>() {
        text.0 = faded(ink, opacity);
    }
    if let Some(mut image) = entity.get_mut::<ImageNode>() {
        image.color = faded(ink, opacity);
    }
}

patch!(PatchFill, Color, |entity, v| {
    repaint(&mut entity, |paint| paint.fill = *v);
});
patch!(PatchBorderColor, Color, |entity, v| {
    repaint(&mut entity, |paint| paint.border = *v);
});
patch!(
    /// A text's colour, or an image's tint.
    PatchInk,
    Color,
    |entity, v| {
        repaint(&mut entity, |paint| paint.ink = *v);
    }
);
patch!(
    /// An image's tint, left to its tone while `None`.
    PatchTint,
    Option<Color>,
    |entity, v| {
        if let Some(tint) = v {
            repaint(&mut entity, |paint| paint.ink = *tint);
        }
    }
);
patch!(PatchOpacity, f32, |entity, v| {
    repaint(&mut entity, |paint| paint.opacity = *v);
});

/// Edits the entity's [`UiTransform`], adding one first if it has
/// none.
fn transform(
    entity: &mut EntityWorldMut,
    edit: impl FnOnce(&mut UiTransform),
) {
    let mut transform =
        entity.get::<UiTransform>().copied().unwrap_or_default();
    edit(&mut transform);
    entity.insert(transform);
}

patch!(PatchScale, f32, |entity, v| {
    transform(&mut entity, |t| t.scale = Vec2::splat(*v));
});
patch!(
    /// Clockwise, in degrees.
    PatchRotation,
    f32,
    |entity, v| {
        transform(&mut entity, |t| t.rotation = Rot2::degrees(*v));
    }
);

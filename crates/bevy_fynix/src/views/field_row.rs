//! A label column and a value column.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ui::{AlignItems, UiRect, percent, px};

use crate::modifier::ModifierExt;
use crate::tokens::{SpacingTokens, TextTokens, Tone};
use crate::views::{BehaviorExt, Label, row};
use crate::{AnyView, Bevy, Cx, View};

/// Indent per level of `depth`, in pixels.
const INDENT: f32 = 12.0;

/// The state of a field that already drives an action.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HasAction;

/// On a node whose field can be dragged out to animate it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimatedField(pub &'static str);

/// `label` over `value`, the label taking 40% of the row.
pub struct FieldRow<L, V> {
    pub label: L,
    pub value: V,
    pub depth: u32,
}

pub fn field_row<L, V>(label: L, value: V) -> FieldRow<L, V> {
    FieldRow {
        label,
        value,
        depth: 0,
    }
}

impl<L, V> FieldRow<L, V> {
    /// How many folds deep the row sits, each indenting it.
    pub fn depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }
}

impl<T, L, V> View<Bevy, T> for FieldRow<L, V>
where
    T: SpacingTokens + Send + Sync + 'static,
    L: View<Bevy, T> + 'static,
    V: View<Bevy, T>,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        // Only the label column is scoped: a label in the value
        // keeps wrapping.
        let label = self.label;
        let label = AnyView::<Bevy, T>::new(move |cx| {
            cx.scope(|cx| {
                cx.set::<Label>(|label, _| label.wrap(false));
                label.build(cx)
            })
        });
        cx.build(
            row((label.width(percent(40.0)), self.value.grow(1.0)))
                .gap(8.0)
                .align(AlignItems::Center)
                .padding(UiRect::left(
                    px(self.depth as f32 * INDENT),
                )),
        )
    }
}

impl Label {
    /// This label with the accent tone while the node holds
    /// [`HasAction`], and [`AnimatedField`] recording the field.
    pub fn animatable<T>(
        self,
        field: &'static str,
    ) -> impl View<Bevy, T>
    where
        T: TextTokens + Send + Sync + 'static,
    {
        self.when::<HasAction, T>(|label, _| label.tone(Tone::Accent))
            .tagged(AnimatedField(field))
    }
}

//! An element's props, written once.

/// An element struct whose fields are all props, with everything that
/// only repeats the list: a builder method per prop, [`Styled`] and
/// [`Layered`], and the checks [`Element`] asks for.
///
/// ```ignore
/// props! {
///     /// A run of text.
///     pub struct Label {
///         text: String,
///         size: f32,
///     }
/// }
/// ```
///
/// [`Styled`]: crate::Styled
/// [`Layered`]: crate::Layered
/// [`Element`]: crate::Element
macro_rules! props {
    (
        $(#[$meta:meta])*
        pub struct $view:ident {
            $($(#[$doc:meta])* $prop:ident: $ty:ty),* $(,)?
        }
    ) => {
        $(#[$meta])*
        pub struct $view {
            $($(#[$doc])* pub $prop: $crate::prop::Prop<$ty>),*
        }

        impl $view {
            $(
                $(#[$doc])*
                pub fn $prop(
                    mut self,
                    $prop: impl Into<$crate::prop::Prop<$ty>>,
                ) -> Self {
                    self.$prop = $prop.into();
                    self
                }
            )*

            /// Whether any prop is bound to the world.
            fn any_bound(&self) -> bool {
                false $(|| self.$prop.is_bound())*
            }

            /// Whether any bound prop may have changed. Every check
            /// runs, as each keeps its own memory.
            fn any_changed(
                &mut self,
                world: &bevy::ecs::world::World,
            ) -> bool {
                false $(| self.$prop.changed(world))*
            }
        }

        fynix::styled!($view { $($prop),* });
    };
}

pub(crate) use props;

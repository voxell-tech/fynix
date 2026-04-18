use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Data;
use syn::DeriveInput;
use syn::Fields;
use syn::Ident;
use syn::parse_macro_input;

fn element_slot_tokens(name: &Ident) -> TokenStream2 {
    quote! {
        const _: () = {
            static __SLOT: ::typeslot::AtomicSlot =
                ::typeslot::AtomicSlot::new();

            impl ::typeslot::TypeSlot<
                ::fynix::element::ElementGroup,
            > for #name {
                #[inline]
                fn try_slot() -> ::core::option::Option<usize> {
                    __SLOT.get()
                }

                #[inline]
                fn dyn_try_slot(
                    &self,
                ) -> ::core::option::Option<usize> {
                    __SLOT.get()
                }
            }

            ::typeslot::inventory::submit! {
                ::typeslot::TypeSlotEntry {
                    type_id: ::core::any::TypeId::of::<#name>(),
                    group_id: ::core::any::TypeId::of::<
                        ::fynix::element::ElementGroup,
                    >(),
                    slot: &__SLOT,
                }
            }
        };
    }
}

/// Derives `typeslot::TypeSlot<fynix::element::ElementGroup>`
/// for the annotated type.
///
/// Equivalent to writing:
///
/// ```ignore
/// #[derive(::typeslot::TypeSlot)]
/// #[slot(::fynix::element::ElementGroup)]
/// ```
#[proc_macro_derive(ElementSlot)]
pub fn derive_element_slot(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    element_slot_tokens(&input.ident).into()
}

/// Derives a default `fynix::element::Element` implementation
/// for a single-child element.
///
/// Also derives `ElementSlot` - no need to add it separately.
///
/// The struct must have exactly one field marked `#[child]`,
/// typed as `Option<ElementId>`. The generated impl:
///
/// - `new` - constructs via `Default::default`.
/// - `children` - yields the `#[child]` field.
/// - `build` - returns the child's computed size, or
///   `Size::ZERO` when no child is set.
///
/// ```ignore
/// #[derive(Element, Default)]
/// pub struct MyElement {
///     #[child]
///     child: Option<ElementId>,
/// }
/// ```
#[proc_macro_derive(Element, attributes(child))]
pub fn derive_element(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(s) => &s.fields,
        _ => {
            return syn::Error::new_spanned(
                name,
                "#[derive(Element)] only supports structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let named = match fields {
        Fields::Named(f) => &f.named,
        _ => {
            return syn::Error::new_spanned(
                name,
                "#[derive(Element)] requires named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let child_fields: Vec<_> = named
        .iter()
        .filter(|f| {
            f.attrs.iter().any(|a| a.path().is_ident("child"))
        })
        .collect();

    let child_field = match child_fields.len() {
        1 => child_fields[0],
        0 => {
            return syn::Error::new_spanned(
                name,
                "#[derive(Element)] requires exactly one \
                 #[child] field",
            )
            .to_compile_error()
            .into();
        }
        _ => {
            return syn::Error::new_spanned(
                child_fields[1],
                "#[derive(Element)] found multiple #[child] \
                 fields; only one is allowed",
            )
            .to_compile_error()
            .into();
        }
    };

    let child_ident = child_field.ident.as_ref().unwrap();
    let slot_tokens = element_slot_tokens(name);

    quote! {
        #slot_tokens

        impl ::fynix::element::Element for #name {
            fn new() -> Self
            where
                Self: Sized,
            {
                ::core::default::Default::default()
            }

            fn children(
                &self,
            ) -> impl ::core::iter::IntoIterator<
                Item = &::fynix::element::ElementId,
            >
            where
                Self: Sized,
            {
                self.#child_ident.iter()
            }

            fn build(
                &self,
                _id: &::fynix::element::ElementId,
                constraint: ::fynix::rectree::Constraint,
                nodes: &mut ::fynix::element::ElementNodes,
            ) -> ::fynix::rectree::Size {
                use ::fynix::rectree::NodeContext as _;
                self.#child_ident
                    .as_ref()
                    .map(|c| nodes.get_size(c))
                    .unwrap_or(::fynix::rectree::Size::ZERO)
            }
        }
    }
    .into()
}

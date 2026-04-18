use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::parse_macro_input;

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
    let name = &input.ident;

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
    .into()
}

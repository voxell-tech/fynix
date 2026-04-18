use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro_crate::crate_name;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Data;
use syn::DeriveInput;
use syn::Expr;
use syn::Fields;
use syn::parse_macro_input;

fn fynix_crate() -> TokenStream2 {
    match crate_name("fynix") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::fynix),
    }
}

fn element_slot_tokens(
    name: &Ident,
    fynix: &TokenStream2,
) -> TokenStream2 {
    quote! {
        const _: () = {
            static __SLOT: #fynix::typeslot::AtomicSlot =
                #fynix::typeslot::AtomicSlot::new();

            impl #fynix::typeslot::TypeSlot<
                #fynix::element::ElementGroup,
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

            #fynix::typeslot::inventory::submit! {
                #fynix::typeslot::TypeSlotEntry {
                    type_id: ::core::any::TypeId::of::<#name>(),
                    group_id: ::core::any::TypeId::of::<
                        #fynix::element::ElementGroup,
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
    let fynix = fynix_crate();
    element_slot_tokens(&input.ident, &fynix).into()
}

struct ElementAttrs {
    new_fn: Option<Expr>,
    children_fn: Option<Expr>,
}

fn parse_element_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<ElementAttrs> {
    let mut new_fn = None;
    let mut children_fn = None;

    for attr in attrs {
        if attr.path().is_ident("element") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("new") {
                    new_fn = Some(meta.value()?.parse::<Expr>()?);
                } else if meta.path.is_ident("children") {
                    children_fn =
                        Some(meta.value()?.parse::<Expr>()?);
                }
                Ok(())
            })?;
        }
    }

    Ok(ElementAttrs {
        new_fn,
        children_fn,
    })
}

/// Derives `ElementNew` and `ElementChildren` for the annotated
/// struct. Also derives `ElementSlot` — no need to add it
/// separately.
///
/// ## `ElementNew`
///
/// By default calls `Default::default()`, requiring the struct to
/// also `#[derive(Default)]`. Override with:
///
/// ```ignore
/// #[element(new = my_constructor_fn)]
/// ```
///
/// where `my_constructor_fn` is a `fn() -> Self`.
///
/// ## `ElementChildren`
///
/// Mark one field `#[children]` for the standard iterator impl:
///
/// ```ignore
/// #[derive(Element, Default)]
/// pub struct MyElement {
///     #[children]
///     child: ElementId,  // or Option<ElementId>, or Vec<ElementId>
/// }
/// ```
///
/// Or override entirely with:
///
/// ```ignore
/// #[element(children = my_children_fn)]
/// ```
///
/// where `my_children_fn` is a `fn(&Self) -> impl IntoIterator<Item = &ElementId>`.
///
/// If neither is present the default (no children) is used.
///
/// ## `build`
///
/// Not generated — implement `Element::build` manually.
#[proc_macro_derive(Element, attributes(children, element))]
pub fn derive_element(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fynix = fynix_crate();

    let attrs = match parse_element_attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

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

    let children_fields: Vec<_> = named
        .iter()
        .filter(|f| {
            f.attrs.iter().any(|a| a.path().is_ident("children"))
        })
        .collect();

    if children_fields.len() > 1 {
        return syn::Error::new_spanned(
            children_fields[1],
            "#[derive(Element)] found multiple #[children] \
             fields; only one is allowed",
        )
        .to_compile_error()
        .into();
    }

    let slot_tokens = element_slot_tokens(name, &fynix);

    let new_impl = match &attrs.new_fn {
        Some(f) => quote! {
            impl #fynix::element::ElementNew for #name {
                fn new() -> Self
                where
                    Self: ::core::marker::Sized,
                {
                    #f()
                }
            }
        },
        None => quote! {
            impl #fynix::element::ElementNew for #name {
                fn new() -> Self
                where
                    Self: ::core::marker::Sized,
                {
                    ::core::default::Default::default()
                }
            }
        },
    };

    let children_impl = if let Some(f) = &attrs.children_fn {
        Some(quote! {
            impl #fynix::element::ElementChildren for #name {
                fn children(
                    &self,
                ) -> impl ::core::iter::IntoIterator<
                    Item = &(#fynix::element::ElementId),
                >
                where
                    Self: ::core::marker::Sized,
                {
                    #f(self)
                }
            }
        })
    } else if let Some(field) = children_fields.first() {
        let ident = field.ident.as_ref().unwrap();
        Some(quote! {
            impl #fynix::element::ElementChildren for #name {
                fn children(
                    &self,
                ) -> impl ::core::iter::IntoIterator<
                    Item = &(#fynix::element::ElementId),
                >
                where
                    Self: ::core::marker::Sized,
                {
                    (&self.#ident).into_iter()
                }
            }
        })
    } else {
        Some(quote! {
            impl #fynix::element::ElementChildren for #name {}
        })
    };

    quote! {
        #slot_tokens
        #new_impl
        #children_impl
    }
    .into()
}

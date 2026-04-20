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

/// Derives `ElementNew`, `ElementChildren`, `ElementSlot`, and `Element` for the annotated struct.
/// Implement `ElementBuild` manually.
#[proc_macro_derive(Element, attributes(children, element))]
pub fn derive_element(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fynix = fynix_crate();

    let attrs = match parse_element_attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let Data::Struct(s) = &input.data else {
        return syn::Error::new_spanned(
            name,
            "#[derive(Element)] only supports structs",
        )
        .to_compile_error()
        .into();
    };

    let Fields::Named(f) = &s.fields else {
        return syn::Error::new_spanned(
            name,
            "#[derive(Element)] requires named fields",
        )
        .to_compile_error()
        .into();
    };

    let children_fields: Vec<_> = f
        .named
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

    let new_body = attrs
        .new_fn
        .as_ref()
        .map(|f| quote! { #f() })
        .unwrap_or_else(
            || quote! { ::core::default::Default::default() },
        );

    let new_impl = quote! {
        impl #fynix::element::ElementNew for #name {
            fn new() -> Self
            where
                Self: ::core::marker::Sized,
            {
                #new_body
            }
        }
    };

    let children_body = attrs
        .children_fn
        .as_ref()
        .map(|f| quote! { #f(self) })
        .or_else(|| {
            children_fields.first().map(|field| {
                let ident = field.ident.as_ref().unwrap();
                quote! { (&self.#ident).into_iter() }
            })
        });

    let children_fn = children_body
        .map(|body| {
            quote! {
                fn children(
                    &self,
                ) -> impl ::core::iter::IntoIterator<
                    Item = &(#fynix::element::ElementId),
                >
                where
                    Self: ::core::marker::Sized,
                {
                    #body
                }
            }
        })
        .unwrap_or_default();

    let children_impl = quote! {
        impl #fynix::element::ElementChildren for #name {
            #children_fn
        }
    };

    let element_impl = quote! {
        impl #fynix::element::Element for #name {}
    };

    quote! {
        #slot_tokens
        #new_impl
        #children_impl
        #element_impl
    }
    .into()
}

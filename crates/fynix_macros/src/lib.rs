use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro_crate::crate_name;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::Data;
use syn::DataStruct;
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
    generics: &syn::Generics,
) -> Option<TokenStream2> {
    generics.params.is_empty().then_some(quote! {
        #fynix::typeslot::register!(
            #fynix::element::ElementGroup,
            #name
        );
    })
}

// TODO: Remove `[#derive(ElementSlot)]`?

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
    element_slot_tokens(&input.ident, &fynix, &input.generics)
        .unwrap_or_else(|| {
            syn::Error::new_spanned(
                input.ident,
                "#[derive(TypeSlot)] only supports\
                non-generic structs",
            )
            .to_compile_error()
        })
        .into()
}

struct ElementAttrs {
    new_body: Option<Expr>,
    children_fn: Option<Expr>,
}

fn parse_element_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<ElementAttrs> {
    let mut new_body = None;
    let mut children_fn = None;

    for attr in attrs {
        if attr.path().is_ident("element") {
            attr.parse_nested_meta(|meta| {
                let key = meta.path.get_ident().map(|i| i.to_string());
                match key.as_deref() {
                    Some("new") => {
                        new_body =
                            Some(meta.value()?.parse::<Expr>()?);
                    }
                    Some("children") => {
                        children_fn =
                            Some(meta.value()?.parse::<Expr>()?);
                    }
                    _ => {
                        return Err(meta.error(
                            "unknown `element` key; expected `new` or `children`",
                        ));
                    }
                }
                Ok(())
            })?;
        }
    }

    Ok(ElementAttrs {
        new_body,
        children_fn,
    })
}

struct FieldAttrs {
    is_children: bool,
    default: Option<Expr>,
}

fn parse_field_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<FieldAttrs> {
    let mut is_children = false;
    let mut default = None;

    for attr in attrs {
        if attr.path().is_ident("element") {
            attr.parse_nested_meta(|meta| {
                let key = meta.path.get_ident().map(|i| i.to_string());
                match key.as_deref() {
                    Some("children") => {
                        is_children = true;
                    }
                    Some("default") => {
                        default =
                            Some(meta.value()?.parse::<Expr>()?);
                    }
                    _ => {
                        return Err(meta.error(
                            "unknown `element` key; expected `children` or `default`",
                        ));
                    }
                }
                Ok(())
            })?;
        }
    }

    Ok(FieldAttrs {
        is_children,
        default,
    })
}

/// Derives `ElementNew`, `ElementChildren`, `ElementSlot`, and
/// `ElementTemplate` for the annotated struct. Implement
/// `ElementBuild` manually. Only works for non-generic structs -
/// use `#[derive(ElementTemplate)]` for generic structs.
#[proc_macro_derive(Element, attributes(element))]
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

    let Some(slot_tokens) =
        element_slot_tokens(name, &fynix, &input.generics)
    else {
        return syn::Error::new_spanned(
            name,
            "#[derive(Element)] only supports non-generic structs, \
            use #[derive(ElementTemplate)] instead",
        )
        .to_compile_error()
        .into();
    };

    let ElementTemplateImpls {
        new_impl,
        children_impl,
        template_impl,
    } = match element_template_impls(
        name,
        &fynix,
        &input.generics,
        s,
        attrs,
    ) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    quote! {
        #slot_tokens
        #new_impl
        #children_impl
        #template_impl
    }
    .into()
}

/// Derives `ElementNew`, `ElementChildren`, and `ElementTemplate`
/// for the annotated struct. Implement `ElementBuild` manually.
/// Call `typeslot::register!(ElementGroup, MyStruct<ConcreteType>)`
/// for each concrete instantiation to satisfy the `Element` bound.
#[proc_macro_derive(ElementTemplate, attributes(element))]
pub fn derive_element_template(input: TokenStream) -> TokenStream {
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
            "#[derive(ElementTemplate)] only supports structs",
        )
        .to_compile_error()
        .into();
    };

    let ElementTemplateImpls {
        new_impl,
        children_impl,
        template_impl,
    } = match element_template_impls(
        name,
        &fynix,
        &input.generics,
        s,
        attrs,
    ) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    quote! {
        #new_impl
        #children_impl
        #template_impl
    }
    .into()
}

fn build_new_body(
    name: &Ident,
    fields: &Fields,
) -> syn::Result<TokenStream2> {
    match fields {
        Fields::Unit => Ok(quote! { #name }),
        Fields::Named(f) => {
            let inits = f
                .named
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let val = parse_field_attrs(&field.attrs)?.default
                        .map(|e| quote! { #e })
                        .unwrap_or_else(|| {
                            quote! { ::core::default::Default::default() }
                        });
                    Ok(quote! { #ident: #val })
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! { #name { #(#inits),* } })
        }
        Fields::Unnamed(f) => {
            let inits = f
                .unnamed
                .iter()
                .map(|field| {
                    let val = parse_field_attrs(&field.attrs)?.default
                        .map(|e| quote! { #e })
                        .unwrap_or_else(|| {
                            quote! { ::core::default::Default::default() }
                        });
                    Ok(val)
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! { #name(#(#inits),*) })
        }
    }
}

struct ElementTemplateImpls {
    new_impl: TokenStream2,
    children_impl: TokenStream2,
    template_impl: TokenStream2,
}

fn element_template_impls(
    name: &Ident,
    fynix: &TokenStream2,
    generics: &syn::Generics,
    s: &DataStruct,
    attrs: ElementAttrs,
) -> syn::Result<ElementTemplateImpls> {
    let children_field = match &s.fields {
        Fields::Named(f) => f.named.iter().find_map(|f| {
            parse_field_attrs(&f.attrs)
                .ok()
                .filter(|a| a.is_children)
                .map(|_| f.ident.as_ref().unwrap())
        }),
        _ => None,
    };

    let (impl_generics, ty_generics, where_clause) =
        generics.split_for_impl();

    let new_body = attrs
        .new_body
        .map(|f| quote! { #f })
        .map(Ok)
        .unwrap_or_else(|| build_new_body(name, &s.fields))?;

    let new_impl = quote! {
        impl #impl_generics #fynix::element::ElementNew
            for #name #ty_generics #where_clause
        {
            #[inline]
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
        .map(|f| quote! { #f(self) })
        .or_else(|| {
            children_field
                .map(|ident| quote! { (&self.#ident).into_iter() })
        });

    let children_fn = children_body
        .map(|body| {
            quote! {
                #[inline]
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
        impl #impl_generics #fynix::element::ElementChildren
            for #name #ty_generics #where_clause
        {
            #children_fn
        }
    };

    let template_impl = quote! {
        impl #impl_generics #fynix::element::ElementTemplate
            for #name #ty_generics #where_clause {}
    };

    Ok(ElementTemplateImpls {
        new_impl,
        children_impl,
        template_impl,
    })
}

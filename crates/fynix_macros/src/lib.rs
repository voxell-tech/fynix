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

/// Derives `fynix::Init` for the annotated struct.
///
/// Each field defaults to `Default::default()` unless annotated with
/// `#[init(expr)]`, which substitutes `expr` as the initial value.
///
/// # Example
///
/// ```ignore
/// #[derive(Init)]
/// struct Label {
///     #[init("hello")]
///     text: &'static str,
///     #[init(16.0)]
///     font_size: f32,
/// }
/// ```
#[proc_macro_derive(Init, attributes(init))]
pub fn derive_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fynix = fynix_crate();

    let Data::Struct(s) = &input.data else {
        return syn::Error::new_spanned(
            name,
            "#[derive(Init)] only supports structs",
        )
        .to_compile_error()
        .into();
    };

    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    let init_body = match build_init_body(name, &s.fields) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error().into(),
    };

    quote! {
        impl #impl_generics #fynix::Init
            for #name #ty_generics #where_clause
        {
            #[inline]
            fn init() -> Self
            where
                Self: ::core::marker::Sized,
            {
                #init_body
            }
        }
    }
    .into()
}

/// Parses `#[init(expr)]` from a field's attributes and returns the
/// expression, falling back to `Default::default()`.
fn init_val_for_field(
    attrs: &[syn::Attribute],
) -> syn::Result<TokenStream2> {
    for attr in attrs {
        if attr.path().is_ident("init") {
            let expr: Expr = attr.parse_args()?;
            return Ok(quote! { #expr });
        }
    }
    Ok(quote! { ::core::default::Default::default() })
}

fn build_init_body(
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
                    let val = init_val_for_field(&field.attrs)?;
                    Ok(quote! { #ident: #val })
                })
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! { #name { #(#inits),* } })
        }
        Fields::Unnamed(f) => {
            let inits = f
                .unnamed
                .iter()
                .map(|field| init_val_for_field(&field.attrs))
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote! { #name(#(#inits),*) })
        }
    }
}

struct ElementAttrs {
    children_fn: Option<Expr>,
}

fn parse_element_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<ElementAttrs> {
    let mut children_fn = None;

    for attr in attrs {
        if attr.path().is_ident("elem") {
            attr.parse_nested_meta(|meta| {
                let key =
                    meta.path.get_ident().map(|i| i.to_string());
                match key.as_deref() {
                    Some("children") => {
                        children_fn =
                            Some(meta.value()?.parse::<Expr>()?);
                    }
                    _ => {
                        return Err(meta.error(
                            "unknown `elem` key; expected `children`",
                        ));
                    }
                }
                Ok(())
            })?;
        }
    }

    Ok(ElementAttrs { children_fn })
}

struct FieldAttrs {
    is_children: bool,
}

fn parse_field_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<FieldAttrs> {
    let mut is_children = false;

    for attr in attrs {
        if attr.path().is_ident("elem") {
            attr.parse_nested_meta(|meta| {
                let key =
                    meta.path.get_ident().map(|i| i.to_string());
                match key.as_deref() {
                    Some("children") => {
                        is_children = true;
                    }
                    _ => {
                        return Err(meta.error(
                            "unknown `elem` key; expected `children`",
                        ));
                    }
                }
                Ok(())
            })?;
        }
    }

    Ok(FieldAttrs { is_children })
}

/// Derives `ElementChildren`, `ElementSlot`, and `ElementTemplate`
/// for the annotated struct. Also derive `Init` for element
/// initialization. Implement `ElementBuild` manually.
///
/// Only works for non-generic structs; use `#[derive(ElementTemplate)]`
/// for generic structs.
#[proc_macro_derive(Element, attributes(elem))]
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
        #children_impl
        #template_impl
    }
    .into()
}

/// Derives `ElementChildren` and `ElementTemplate` for the annotated
/// struct. Also derive `Init` for element initialization. Implement
/// `ElementBuild` manually.
///
/// Call `typeslot::register!(ElementGroup, MyStruct<ConcreteType>)`
/// for each concrete instantiation to satisfy the `Element` bound.
#[proc_macro_derive(ElementTemplate, attributes(elem))]
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
        #children_impl
        #template_impl
    }
    .into()
}

struct FieldInfo {
    ident: Option<Ident>,
    is_children: bool,
}

fn parse_fields(fields: &Fields) -> syn::Result<Vec<FieldInfo>> {
    let iter = match fields {
        Fields::Unit => return Ok(vec![]),
        Fields::Named(f) => f.named.iter(),
        Fields::Unnamed(f) => f.unnamed.iter(),
    };
    iter.map(|field| {
        let fa = parse_field_attrs(&field.attrs)?;
        Ok(FieldInfo {
            ident: field.ident.clone(),
            is_children: fa.is_children,
        })
    })
    .collect()
}

struct ElementTemplateImpls {
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
    let field_infos = parse_fields(&s.fields)?;

    let mut children_field: Option<&Ident> = None;
    for ident in field_infos
        .iter()
        .filter(|f| f.is_children)
        .filter_map(|f| f.ident.as_ref())
    {
        if children_field.is_some() {
            return Err(syn::Error::new_spanned(
                ident,
                "`#[elem(children)]` can only be used on one field",
            ));
        }
        children_field = Some(ident);
    }

    let (impl_generics, ty_generics, where_clause) =
        generics.split_for_impl();

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
        children_impl,
        template_impl,
    })
}

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Data, DataStruct, DeriveInput, Expr, Fields, FnArg, ItemFn, Type,
    parse_macro_input,
};

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
        impl #impl_generics #fynix::init::Init
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

/// Derives `ElementChildren` for the annotated struct.
///
/// Works for both generic and non-generic structs. Implement
/// `ElementBuild` manually; `Element` is satisfied automatically via
/// the blanket impl once `Init`, `ElementChildren`, and
/// `ElementBuild` are all in scope.
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

    match element_children_impl(
        name,
        &fynix,
        &input.generics,
        s,
        attrs,
    ) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
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

fn element_children_impl(
    name: &Ident,
    fynix: &TokenStream2,
    generics: &syn::Generics,
    s: &DataStruct,
    attrs: ElementAttrs,
) -> syn::Result<TokenStream2> {
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

    Ok(quote! {
        impl #impl_generics #fynix::element::ElementChildren
            for #name #ty_generics #where_clause
        {
            #children_fn
        }
    })
}

/// Registers an interaction handler from a free function.
///
/// ```ignore
/// #[fynix(interaction)]
/// fn on_click(e: &mut Button, click: Click, events: &mut Events) {
///     (e.on_click)(events);
/// }
/// ```
///
/// The element type comes from the first parameter (`&mut E`), the
/// interaction type from the second (`I`, by value), and outgoing
/// messages flow through the third (`&mut Events`). The handler is
/// collected at startup and dispatched on its `(E, I)` pair.
#[proc_macro_attribute]
pub fn fynix(attr: TokenStream, item: TokenStream) -> TokenStream {
    let kind = parse_macro_input!(attr as Ident);
    if kind == "interaction" {
        return fynix_interaction(item);
    }

    syn::Error::new_spanned(
        kind,
        "unknown `fynix` attribute; expected `interaction`",
    )
    .to_compile_error()
    .into()
}

/// Expands `#[fynix(interaction)]` on a handler function.
fn fynix_interaction(item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    match interaction_impl(func) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Returns the `E` in a `&mut E` parameter type.
fn ref_mut_target(ty: &Type) -> syn::Result<&Type> {
    if let Type::Reference(reference) = ty
        && reference.mutability.is_some()
    {
        return Ok(&reference.elem);
    }

    Err(syn::Error::new_spanned(
        ty,
        "expected `&mut Element` as the first parameter",
    ))
}

fn interaction_impl(func: ItemFn) -> syn::Result<TokenStream2> {
    let fynix = fynix_crate();
    let fn_name = func.sig.ident.clone();

    let mut inputs = func.sig.inputs.iter();

    let elem_ty = match inputs.next() {
        Some(FnArg::Typed(pat)) => ref_mut_target(&pat.ty)?,
        _ => {
            return Err(syn::Error::new_spanned(
                &func.sig,
                "interaction handler needs `&mut Element` as its \
                 first parameter",
            ));
        }
    };

    let interaction_ty = match inputs.next() {
        Some(FnArg::Typed(pat)) => &*pat.ty,
        _ => {
            return Err(syn::Error::new_spanned(
                &func.sig,
                "interaction handler needs the interaction value as \
                 its second parameter",
            ));
        }
    };

    let registrar = format_ident!("__fynix_register_{}", fn_name);

    Ok(quote! {
        #func

        const _: () = {
            fn #registrar(
                interactions: &mut #fynix::interaction::Interactions,
            ) {
                interactions.register::<#elem_ty, #interaction_ty>(
                    #fn_name,
                );
            }

            #fynix::__private::inventory::submit! {
                #fynix::interaction::InteractionRegistrar {
                    register: #registrar,
                }
            }
        };
    })
}

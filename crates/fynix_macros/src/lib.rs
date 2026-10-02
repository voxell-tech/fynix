#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Error, Expr, Field, Fields, GenericArgument, Ident, ItemStruct,
    Meta, Path, PathArguments, Result, Token, Type, TypeParamBound,
    parse_macro_input,
};

/// Makes a struct of props an element of one backend.
///
/// See the crate's docs for what goes on the struct and on each
/// field, and what is written.
#[proc_macro_attribute]
pub fn element(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as ElementArgs);
    let item = parse_macro_input!(item as ItemStruct);
    expand(args, item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// What `#[element(..)]` takes on the struct.
#[derive(Default)]
struct ElementArgs {
    backend: Option<Type>,
    theme: Option<Punctuated<TypeParamBound, Token![+]>>,
    prepare: Option<Path>,
}

impl Parse for ElementArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut args = Self::default();
        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "backend" => args.backend = Some(input.parse()?),
                "theme" => {
                    args.theme = Some(
                        Punctuated::parse_separated_nonempty(input)?,
                    );
                }
                "prepare" => args.prepare = Some(input.parse()?),
                _ => {
                    return Err(Error::new(
                        key.span(),
                        "expected `backend`, `theme` or `prepare`",
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}

/// What `#[elem(..)]` takes on a field.
#[derive(Default)]
struct PropArgs {
    patch: Option<Type>,
    default: Option<Expr>,
    blend: Option<Expr>,
    with: Option<Expr>,
    shown: Option<Type>,
}

impl PropArgs {
    fn parse(field: &Field) -> Result<Self> {
        let mut args = Self::default();
        let attrs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("elem"));
        for attr in attrs {
            attr.parse_nested_meta(|meta| {
                let value = meta.value()?;
                if meta.path.is_ident("patch") {
                    args.patch = Some(value.parse()?);
                } else if meta.path.is_ident("default") {
                    args.default = Some(value.parse()?);
                } else if meta.path.is_ident("blend") {
                    args.blend = Some(value.parse()?);
                } else if meta.path.is_ident("with") {
                    args.with = Some(value.parse()?);
                } else if meta.path.is_ident("shown") {
                    args.shown = Some(value.parse()?);
                } else {
                    return Err(meta.error(
                        "expected `patch`, `default`, `blend`, \
                         `with` or `shown`",
                    ));
                }
                Ok(())
            })?;
        }
        Ok(args)
    }
}

/// One prop: its field, the value type inside its `Prop<..>`, and
/// what its `#[elem(..)]` said.
struct Prop {
    name: Ident,
    field: Type,
    value: Type,
    args: PropArgs,
    patch: Type,
    docs: Vec<syn::Attribute>,
}

/// The last generic argument of a field's type: the `P` of `Prop<P>`
/// or of `Prop<W, P>`.
fn value_type(field: &Type) -> Result<Type> {
    let error = || {
        Error::new(field.span(), "a prop's type must be `Prop<..>`")
    };
    let Type::Path(path) = field else {
        return Err(error());
    };
    let last = path.path.segments.last().ok_or_else(error)?;
    let PathArguments::AngleBracketed(generics) = &last.arguments
    else {
        return Err(error());
    };
    generics
        .args
        .iter()
        .rev()
        .find_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .ok_or_else(error)
}

/// The path to the `fynix` crate from where the macro is used.
fn fynix() -> Tokens {
    match crate_name("fynix") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, Span::call_site());
            quote!(::#name)
        }
        Err(_) => quote!(::fynix),
    }
}

/// The path to the `lenz` crate: the using crate's own, or the one
/// `fynix` re-exports.
fn lenz(fynix: &Tokens) -> Tokens {
    match crate_name("lenz") {
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, Span::call_site());
            quote!(::#name)
        }
        _ => quote!(#fynix::lenz),
    }
}

/// `SomeName` as `some_name`.
fn snake(name: &Ident) -> String {
    let mut snake = String::new();
    for (index, letter) in name.to_string().chars().enumerate() {
        if letter.is_uppercase() && index > 0 {
            snake.push('_');
        }
        snake.extend(letter.to_lowercase());
    }
    snake
}

fn expand(args: ElementArgs, mut item: ItemStruct) -> Result<Tokens> {
    let fynix = fynix();
    let lenz = lenz(&fynix);
    let backend = args.backend.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "`#[element]` needs `backend = <type>`",
        )
    })?;
    let Fields::Named(fields) = &mut item.fields else {
        return Err(Error::new(
            item.span(),
            "`#[element]` goes on a struct with named fields",
        ));
    };

    let mut props = Vec::new();
    for field in &mut fields.named {
        let args = PropArgs::parse(field)?;
        let patch = args.patch.clone().ok_or_else(|| {
            Error::new(
                field.span(),
                "a prop needs `#[elem(patch = <type>)]`",
            )
        })?;
        if args.shown.is_some() && args.with.is_none() {
            return Err(Error::new(
                field.span(),
                "`shown = <type>` needs `with = <fn>` to make it",
            ));
        }
        field.attrs.retain(|attr| !attr.path().is_ident("elem"));
        let docs = field
            .attrs
            .iter()
            .filter(|attr| {
                matches!(&attr.meta, Meta::NameValue(meta)
                    if meta.path.is_ident("doc"))
            })
            .cloned()
            .collect();
        field.attrs.push(syn::parse_quote!(#[lenz(tag = #patch)]));
        props.push(Prop {
            name: field.ident.clone().expect("a named field"),
            value: value_type(&field.ty)?,
            field: field.ty.clone(),
            args,
            patch,
            docs,
        });
    }
    if props.len() > 64 {
        return Err(Error::new(
            item.span(),
            "an element takes at most 64 props",
        ));
    }

    let name = &item.ident;
    let vis = &item.vis;
    let shown = format_ident!("{name}Shown");
    let props_trait = format_ident!("{name}Props");
    let accessor = format_ident!("{}_mut", snake(name));
    let names =
        props.iter().map(|prop| &prop.name).collect::<Vec<_>>();
    let bits = (0..props.len() as u32).collect::<Vec<_>>();
    let theme = Ident::new("theme", Span::call_site());
    let world = Ident::new("world", Span::call_site());

    let builders = props.iter().map(|prop| {
        let Prop {
            name, field, docs, ..
        } = prop;
        quote! {
            #(#docs)*
            pub fn #name(mut self, #name: impl Into<#field>) -> Self {
                self.#name = #name.into();
                self
            }
        }
    });
    let forwards = props.iter().map(|prop| {
        let Prop {
            name, field, docs, ..
        } = prop;
        quote! {
            #(#docs)*
            fn #name(mut self, #name: impl Into<#field>) -> Self {
                self.#accessor().#name = #name.into();
                self
            }
        }
    });
    let slots = props.iter().map(|prop| {
        let name = &prop.name;
        let shown = prop.args.shown.as_ref().unwrap_or(&prop.value);
        quote!(#name: #fynix::Slot<#shown>)
    });
    let aims = props.iter().zip(&bits).map(|(prop, bit)| {
        let name = &prop.name;
        let value = &prop.value;
        let default = match &prop.args.default {
            Some(default) => quote!(#default),
            None => quote!(::core::default::Default::default()),
        };
        let with = prop.args.with.as_ref().map(|with| {
            quote! {
                let value = #fynix::slot::with_theme(#with, value, #theme);
            }
        });
        let tween = match &prop.args.blend {
            Some(blend) => quote! {
                curve.map(|curve| #fynix::Tween {
                    curve,
                    interp: #blend,
                })
            },
            None => quote!(::core::option::Option::None),
        };
        quote! {
            if dirty & (1u64 << #bit) != 0 {
                let value: #value = match self.#name.get(#world) {
                    ::core::option::Option::Some(value) => value,
                    ::core::option::Option::None => #default,
                };
                #with
                shown.#name.aim(value, #tween, tick);
            }
        }
    });
    let steps = props.iter().map(|prop| {
        let name = &prop.name;
        let patch = &prop.patch;
        let shown = prop.args.shown.as_ref().unwrap_or(&prop.value);
        quote! {
            moving |= shown.#name.step(tick, |value| {
                <#patch as #fynix::Patch<#backend, #shown>>::patch(
                    #world, node, value,
                );
            });
        }
    });

    let bit_of = |wanted: &str| {
        props
            .iter()
            .position(|prop| prop.name == wanted)
            .map(|at| at as u32)
    };
    let visual = match (bit_of("opacity"), bit_of("scale")) {
        (Some(opacity), Some(scale)) => quote! {
            fn visual(
                &mut self,
            ) -> ::core::option::Option<
                #fynix::VisualMut<
                    '_,
                    <#backend as #fynix::Backend>::World,
                >,
            > {
                ::core::option::Option::Some(#fynix::VisualMut {
                    opacity: &mut self.opacity,
                    scale: &mut self.scale,
                })
            }

            fn visual_bits() -> [u64; 2] {
                [1u64 << #opacity, 1u64 << #scale]
            }
        },
        _ => quote!(),
    };

    let bounds = match &args.theme {
        Some(bounds) => quote!(: #bounds),
        None => quote!(),
    };
    let prepare = match &args.prepare {
        Some(prepare) => quote!(#prepare(#world, node);),
        None => quote!(let _ = (#world, node);),
    };
    let shown_doc =
        format!("Each prop of a [`{name}`] as written on its node.");
    let trait_doc = format!(
        "The props of a [`{name}`], for anything holding one."
    );
    let accessor_doc =
        format!("The [`{name}`] the props are set on.");

    Ok(quote! {
        #[derive(#lenz::Lenz)]
        #item

        #[allow(dead_code)]
        impl #name {
            #(#builders)*
        }

        #[doc = #trait_doc]
        #[allow(dead_code)]
        #vis trait #props_trait: Sized {
            #[doc = #accessor_doc]
            fn #accessor(&mut self) -> &mut #name;

            #(#forwards)*
        }

        impl #props_trait for #name {
            fn #accessor(&mut self) -> &mut #name {
                self
            }
        }

        #fynix::styled!(#name { #(#names),* });

        #[doc = #shown_doc]
        #[derive(Default)]
        #vis struct #shown {
            #(#slots),*
        }

        impl<__Theme #bounds> #fynix::Element<#backend, __Theme>
            for #name
        {
            type Shown = #shown;

            fn prepare(
                #world: &mut <#backend as #fynix::Backend>::World,
                node: <#backend as #fynix::Backend>::Node,
            ) {
                #prepare
            }

            fn update(
                &self,
                shown: &mut #shown,
                dirty: u64,
                #world: &mut <#backend as #fynix::Backend>::World,
                node: <#backend as #fynix::Backend>::Node,
                #theme: &__Theme,
                tick: #fynix::Tick,
                curve: ::core::option::Option<#fynix::Curve>,
            ) -> bool {
                #(#aims)*
                let mut moving = false;
                #(#steps)*
                moving
            }

            fn is_live(&self) -> bool {
                false #(|| self.#names.is_bound())*
            }

            fn changed(
                &mut self,
                #world: &<#backend as #fynix::Backend>::World,
            ) -> u64 {
                0 #(| ((self.#names.changed(#world) as u64) << #bits))*
            }

            #visual
        }
    })
}

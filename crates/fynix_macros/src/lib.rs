#![no_std]

extern crate alloc;
extern crate proc_macro;

use alloc::format;
use alloc::string::ToString;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    FnArg, GenericArgument, Ident, ItemFn, PathArguments, Result,
    Type, TypePath, TypeReference,
};

struct FynixAttr {
    ident: Ident,
}

impl Parse for FynixAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        Ok(FynixAttr { ident })
    }
}

/// Extracts type E from `&mut E` pattern.
fn extract_element_type(ty: &Type) -> Result<&Type> {
    if let Type::Reference(TypeReference {
        mutability: Some(_),
        elem,
        ..
    }) = ty
    {
        Ok(&**elem)
    } else {
        Err(syn::Error::new_spanned(
            ty,
            "First parameter must be `&mut E`",
        ))
    }
}

/// Extracts type W from `FynixCtx<W>` pattern.
fn extract_context_type(ty: &Type) -> Result<&Type> {
    // just want to get AngleBracketedGenericArguments from the path segments
    if let Type::Reference(TypeReference {
        mutability: Some(_),
        elem,
        ..
    }) = ty
        && let Type::Path(TypePath { path, .. }) = &**elem
        && let Some(seg) = path.segments.last()
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(GenericArgument::Type(w)) = args.args.first()
    {
        return Ok(w);
    }
    Err(syn::Error::new_spanned(
        ty,
        "Second parameter must be `&mut FynixCtx<W>`",
    ))
}

#[proc_macro_attribute]
pub fn fynix(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = syn::parse_macro_input!(attr as FynixAttr);
    let item = syn::parse_macro_input!(item as ItemFn);

    match attr.ident.to_string().as_str() {
        "compose" => match process_fynix_compose(item) {
            Ok(i) => i.into(),
            Err(e) => e.to_compile_error().into(),
        },
        i => syn::Error::new_spanned(
            attr.ident,
            format!("'{}' is not supported. Supported keywords include: 'compose'", i),
        )
        .to_compile_error()
        .into(),
    }
}

/// Creates TokenStream based on the compose function
fn process_fynix_compose(
    item: ItemFn,
) -> Result<proc_macro2::TokenStream> {
    // 1. take the fn name
    let fn_name: &Ident = &item.sig.ident;
    let static_name: Ident = Ident::new(
        &format!(
            "_ELEMENT_COMPOSER_{}",
            fn_name.to_string().to_uppercase()
        ),
        fn_name.span(),
    );

    // 2. get E and W types
    let params = &item.sig.inputs;
    if params.len() != 2 {
        return Err(syn::Error::new_spanned(
            params,
            "Expected pattern `fn(e: &mut E, ctx: &mut FynixCtx<W>) { ... }`",
        ));
    }

    let first_param = params.first().expect("params len is 2");
    let second_param = params.last().expect("params len is 2");

    let element_ty = if let FnArg::Typed(pat_ty) = first_param {
        extract_element_type(&pat_ty.ty)?
    } else {
        return Err(syn::Error::new_spanned(
            first_param,
            "Expected typed parameter",
        ));
    };

    let context_ty = if let FnArg::Typed(pat_ty) = second_param {
        extract_context_type(&pat_ty.ty)?
    } else {
        return Err(syn::Error::new_spanned(
            second_param,
            "Expected typed parameter",
        ));
    };

    let output = quote! {
        #item

        #[::fynix::linkme::distributed_slice(
            ::fynix::ELEMENT_COMPOSERS
        )]
        static #static_name:
            ::fynix::UntypedElementComposer =
            ::fynix::UntypedElementComposer::new::<
                #element_ty,
                #context_ty
            >(#fn_name);
    };

    Ok(output)
}

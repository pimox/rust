use std::convert::TryFrom;
use std::iter::IntoIterator;

use anyhow::Error;

use proc_macro2::TokenStream;

use quote::quote;
use syn::punctuated::Punctuated;
use syn::{AttributeArgs, Token};

use crate::attribs::FunctionAttrs;
use crate::package::Package;

pub fn handle_module(attr: AttributeArgs, mut module: syn::ItemMod) -> Result<TokenStream, Error> {
    let mut package = Package::with_attrs(attr)?;
    let mangled_package_name = package.mangle_package_name();

    if let Some((_brace, ref mut items)) = module.content {
        for item in items.iter_mut() {
            match core::mem::replace(item, syn::Item::Verbatim(TokenStream::new())) {
                syn::Item::Fn(mut func) => {
                    let mut attribs = None;
                    for attr in std::mem::take(&mut func.attrs) {
                        if attr.path.is_ident("export") {
                            if attribs.is_some() {
                                bail!(attr => "multiple 'export' attributes not allowed");
                            }

                            let args: AttributeArgs = if attr.tokens.is_empty() {
                                Default::default()
                            } else {
                                attr.parse_args_with(
                                    Punctuated::<syn::NestedMeta, Token![,]>::parse_terminated,
                                )?
                                .into_iter()
                                .collect()
                            };

                            attribs = Some(FunctionAttrs::try_from(args)?);
                        } else {
                            // retain the attribute
                            func.attrs.push(attr);
                        }
                    }

                    // if we removed an #[export] macro this is an exported function:
                    if let Some(attribs) = attribs {
                        let func = crate::function::handle_function(
                            attribs,
                            func,
                            Some(&mangled_package_name),
                        )?;
                        *item = syn::Item::Verbatim(func.tokens);

                        package.export_named(
                            func.rust_name,
                            func.perl_name,
                            func.xs_name,
                            "src/FIXME.rs".to_string(),
                        );
                    } else {
                        *item = syn::Item::Fn(func);
                    }
                }
                other => *item = other,
            }
        }
    }

    package.write()?;

    Ok(quote! { #module })
}

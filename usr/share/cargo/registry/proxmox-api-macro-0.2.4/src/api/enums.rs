use std::convert::{TryFrom, TryInto};

use anyhow::Error;

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote_spanned;

use super::Schema;
use crate::serde;
use crate::util::{self, FieldName, JSONObject, JSONValue, Maybe};

/// Enums, provided they're simple enums, simply get an enum string schema attached to them.
pub fn handle_enum(
    mut attribs: JSONObject,
    mut enum_ty: syn::ItemEnum,
) -> Result<TokenStream, Error> {
    if !attribs.contains_key("type") {
        attribs.insert(
            FieldName::new("type".to_string(), Span::call_site()),
            JSONValue::new_ident(Ident::new("String", enum_ty.enum_token.span)),
        );
    }

    if let Some(fmt) = attribs.remove("format") {
        error!(fmt.span(), "illegal key 'format', will be autogenerated");
    }

    let schema = {
        let mut schema: Schema = attribs.try_into()?;

        if schema.description.is_none() {
            let (comment, span) = util::get_doc_comments(&enum_ty.attrs)?;
            schema.description = Maybe::Derived(syn::LitStr::new(comment.trim(), span));
        }

        let mut ts = TokenStream::new();
        schema.to_typed_schema(&mut ts)?;
        ts
    };

    let container_attrs = serde::ContainerAttrib::try_from(&enum_ty.attrs[..])?;

    let mut variants = TokenStream::new();
    for variant in &mut enum_ty.variants {
        match &variant.fields {
            syn::Fields::Unit => (),
            _ => bail!(variant => "api macro does not support enums with fields"),
        }

        let (mut comment, _doc_span) = util::get_doc_comments(&variant.attrs)?;
        if comment.is_empty() {
            error!(&variant => "enum variant needs a description");
            comment = "<missing description>".to_string();
        }

        let attrs = serde::SerdeAttrib::try_from(&variant.attrs[..])?;
        let variant_string = if let Some(renamed) = attrs.rename {
            renamed.into_lit_str()
        } else if let Some(rename_all) = container_attrs.rename_all {
            let name = rename_all.apply_to_variant(&variant.ident.to_string());
            syn::LitStr::new(&name, variant.ident.span())
        } else {
            let name = &variant.ident;
            syn::LitStr::new(&name.to_string(), name.span())
        };

        variants.extend(quote_spanned! { variant.ident.span() =>
            ::proxmox::api::schema::EnumEntry {
                value: #variant_string,
                description: #comment,
            },
        });
    }

    let name = &enum_ty.ident;

    Ok(quote_spanned! { name.span() =>
        #enum_ty
        impl #name {
            pub const API_SCHEMA: ::proxmox::api::schema::Schema =
                #schema
                .format(&::proxmox::api::schema::ApiStringFormat::Enum(&[#variants]))
                .schema();
        }
    })
}

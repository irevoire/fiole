//! The impls of `#[derive(Fiole)]`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, GenericParam, Ident, Member, Type};

use crate::attrs::{self, Container};

/// The most fields a struct can have: the arity of the largest `TupleErrorN`.
const MAX_FIELDS: usize = 19;

/// A field of the struct, and the codec it's written with.
struct Field {
    member: Member,
    ty: Type,
    codec: Type,
    /// Where the errors about its codec point: the codec in its `#[fiole(codec = ...)]`.
    span: Span,
}

pub fn codec(input: &DeriveInput) -> syn::Result<TokenStream> {
    let container = attrs::container(&input.attrs)?;
    let fields = fields(input)?;

    let struct_ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let item = quote!(#struct_ident #ty_generics);
    let (declaration, codec) = match &container.name {
        None => (quote!(), item.clone()),
        Some(name) => (declare(name, input), quote!(#name #ty_generics)),
    };
    let existing = || {
        where_clause
            .map(|clause| clause.predicates.iter())
            .into_iter()
            .flatten()
    };

    let encode = encode(&fields, &container, &item);
    let decode = decode(&fields, &container, &item, struct_ident);
    let (encode_bounds, decode_bounds) = bounds(&fields, &container);
    let self_delimiting = fields.iter().map(|Field { codec, span, .. }| {
        // Higher-ranked, for a struct without generics not to be refused
        // when its last field isn't self-delimiting: the struct then isn't.
        quote_spanned!(*span=> for<'__fiole> #codec: ::fiole::codec::SelfDelimiting)
    });
    let existing_split = existing();
    let split = split(&fields).map(|body| {
        quote! {
            #[automatically_derived]
            impl #impl_generics ::fiole::codec::Split for #codec
            where #(#existing_split,)* #body
        }
    });
    let existing_encode = existing();
    let existing_decode = existing();
    let existing_self_delimiting = existing();

    Ok(quote! {
        #declaration

        #[automatically_derived]
        impl #impl_generics ::fiole::codec::Encode for #codec
        where #(#existing_encode,)* #(#encode_bounds,)*
        {
            #encode
        }

        #[automatically_derived]
        impl #impl_generics ::fiole::codec::Decode for #codec
        where #(#existing_decode,)* #(#decode_bounds,)*
        {
            #decode
        }

        #[automatically_derived]
        impl #impl_generics ::fiole::codec::SelfDelimiting for #codec
        where #(#existing_self_delimiting,)* #(#self_delimiting,)*
        {}

        #split
    })
}

fn fields(input: &DeriveInput) -> syn::Result<Vec<Field>> {
    const ONLY_STRUCTS: &str =
        "`Fiole` can only be derived for a struct, whose fields are written one after the other";
    let data = match &input.data {
        Data::Struct(data) => data,
        Data::Enum(data) => return Err(syn::Error::new(data.enum_token.span, ONLY_STRUCTS)),
        Data::Union(data) => return Err(syn::Error::new(data.union_token.span, ONLY_STRUCTS)),
    };
    if data.fields.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "a struct without fields has nothing to write: use the `Unit` codec instead",
        ));
    }
    if let Some(field) = data.fields.iter().nth(MAX_FIELDS) {
        return Err(syn::Error::new_spanned(
            field,
            format!("a struct deriving `Fiole` can have at most {MAX_FIELDS} fields"),
        ));
    }
    data.fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let codec = attrs::field(field)?;
            Ok(Field {
                member: field
                    .ident
                    .clone()
                    .map_or_else(|| Member::from(index), Member::Named),
                ty: field.ty.clone(),
                span: codec.span(),
                codec,
            })
        })
        .collect()
}

/// The codec type of the struct, when it's named.
fn declare(name: &Ident, input: &DeriveInput) -> TokenStream {
    let vis = &input.vis;
    let generics = &input.generics;
    let where_clause = &generics.where_clause;
    let doc = format!(
        "The codec of [`{}`], derived with `fiole::Fiole`.",
        input.ident
    );
    if generics.params.is_empty() {
        return quote! {
            #[doc = #doc]
            #vis enum #name {}
        };
    }
    // Only there for the codec to have the generics of the struct
    let marker = generics.params.iter().filter_map(|param| match param {
        GenericParam::Type(param) => {
            let ident = &param.ident;
            Some(quote!(#ident))
        }
        GenericParam::Lifetime(param) => {
            let lifetime = &param.lifetime;
            Some(quote!(&#lifetime ()))
        }
        GenericParam::Const(_) => None,
    });
    quote! {
        #[doc = #doc]
        #vis struct #name #generics (::core::marker::PhantomData<(#(#marker,)*)>) #where_clause;
    }
}

/// The bounds of `Encode` and `Decode`, on the codec of each field rather
/// than on the generics: they hold for generics through a codec, like
/// `Text<T>`, as well as for a field whose codec is a generic, like `T`.
fn bounds(fields: &[Field], container: &Container) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let last = fields.len() - 1;
    let mut encode = Vec::new();
    let mut decode = Vec::new();
    for (
        index,
        Field {
            ty, codec, span, ..
        },
    ) in fields.iter().enumerate()
    {
        encode.push(quote_spanned!(*span=> #codec: ::fiole::codec::Encode));
        encode.push(quote_spanned!(*span=> #ty: ::fiole::derive_support::EncodedBy<#codec>));
        decode.push(quote_spanned!(*span=> #codec: ::fiole::codec::Decode));
        decode.push(quote_spanned!(*span=> #ty: ::fiole::derive_support::DecodedBy<#codec>));
        if index != last {
            encode.push(quote_spanned!(*span=> #codec: ::fiole::codec::SelfDelimiting));
            decode.push(quote_spanned!(*span=> #codec: ::fiole::codec::SelfDelimiting));
        }
        if let Some(error) = &container.error {
            encode.push(quote_spanned!(*span=>
                #error: ::core::convert::From<<#codec as ::fiole::codec::Encode>::Error>
            ));
            decode.push(quote_spanned!(*span=>
                #error: ::core::convert::From<<#codec as ::fiole::codec::Decode>::Error>
            ));
        }
    }
    (encode, decode)
}

/// The error of the codec, and how the error of each field becomes it: a
/// variant of the tuple error, or `?` converting it into the user's error.
fn error(
    fields: &[Field],
    container: &Container,
    of: &TokenStream,
) -> (TokenStream, Vec<TokenStream>) {
    match &container.error {
        Some(error) => (quote!(#error), fields.iter().map(|_| quote!()).collect()),
        None => {
            let tuple_error = format_ident!("TupleError{}", fields.len());
            let errors = fields
                .iter()
                .map(|Field { codec, .. }| quote!(<#codec as #of>::Error));
            let wraps = (1..=fields.len())
                .map(|index| {
                    let variant = format_ident!("C{index}");
                    quote!(.map_err(::fiole::codec::#tuple_error::#variant))
                })
                .collect();
            (quote!(::fiole::codec::#tuple_error<#(#errors),*>), wraps)
        }
    }
}

fn encode(fields: &[Field], container: &Container, item: &TokenStream) -> TokenStream {
    let (error, wraps) = error(fields, container, &quote!(::fiole::codec::Encode));
    let encodes = fields.iter().zip(wraps).map(
        |(
            Field {
                member,
                ty,
                codec,
                span,
            },
            wrap,
        )| {
            // With the span of the field, for an error in it to be the same as
            // the one of its bound, and shown once
            let item = quote_spanned!(*span=>
                <#ty as ::fiole::derive_support::EncodedBy<#codec>>::borrow_item(&item.#member)
            );
            quote! {
                let into = <#codec as ::fiole::codec::Encode>::encode(into, #item) #wrap?;
            }
        },
    );
    quote! {
        type Item = #item;
        type Error = #error;

        fn encode(
            into: ::fiole::codec::EncodingVec<::fiole::codec::Fresh>,
            item: &Self::Item,
        ) -> ::core::result::Result<::fiole::codec::EncodingVec<::fiole::codec::Fresh>, Self::Error> {
            #(#encodes)*
            ::core::result::Result::Ok(into)
        }
    }
}

fn decode(
    fields: &[Field],
    container: &Container,
    item: &TokenStream,
    struct_ident: &Ident,
) -> TokenStream {
    let (error, wraps) = error(fields, container, &quote!(::fiole::codec::Decode));
    let vars: Vec<Ident> = (0..fields.len())
        .map(|index| format_ident!("__fiole_field_{index}"))
        .collect();
    let decodes = fields
        .iter()
        .zip(&vars)
        .zip(wraps)
        .map(|((Field { codec, .. }, var), wrap)| {
            quote! {
                let #var = <#codec as ::fiole::codec::Decode>::decode(bytes) #wrap?;
            }
        });
    let members = fields.iter().zip(&vars).map(
        |(
            Field {
                member,
                ty,
                codec,
                span,
            },
            var,
        )| {
            let field = quote_spanned!(*span=>
                <#ty as ::fiole::derive_support::DecodedBy<#codec>>::from_item(#var)
            );
            quote!(#member: #field)
        },
    );
    quote! {
        type Item = #item;
        type Error = #error;

        fn decode(
            bytes: &mut ::fiole::codec::DecodingVec,
        ) -> ::core::result::Result<Self::Item, Self::Error> {
            #(#decodes)*
            ::core::result::Result::Ok(#struct_ident { #(#members,)* })
        }
    }
}

/// The body of `Split` when there are two fields or more: the codec of the
/// first field, then the one of the rest.
fn split(fields: &[Field]) -> Option<TokenStream> {
    let (
        Field {
            codec: head, span, ..
        },
        rest,
    ) = fields.split_first()?;
    let tail = match rest {
        [] => return None,
        [Field { codec, .. }] => quote!(#codec),
        rest => {
            let codecs = rest.iter().map(|Field { codec, .. }| codec);
            quote!((#(#codecs),*))
        }
    };
    let bound = quote_spanned!(*span=>
        for<'__fiole> #head: ::fiole::codec::Encode + ::fiole::codec::SelfDelimiting
    );
    Some(quote! {
        #bound
        {
            type Head = #head;
            type Tail = #tail;
        }
    })
}

//! What the `#[fiole(...)]` attributes say: `name` and `error` on the struct,
//! `codec` on its fields.

use syn::meta::ParseNestedMeta;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::{Attribute, Ident, Meta, Token, Type};

/// What `#[fiole(...)]` on the struct says.
#[derive(Default)]
pub struct Container {
    /// The codec type to generate. Without it, the struct is its own codec.
    pub name: Option<Ident>,
    /// The error of both `Encode` and `Decode`. Without it, the errors are
    /// the ones of the tuple codec.
    pub error: Option<Type>,
}

pub fn container(attrs: &[Attribute]) -> syn::Result<Container> {
    let mut container = Container::default();
    for attr in fiole(attrs) {
        if !matches!(attr.meta, Meta::List(_)) {
            return Err(syn::Error::new_spanned(
                attr,
                "expected the keys of the struct, like `#[fiole(name = MyCodec)]`",
            ));
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if container.name.is_some() {
                    return Err(meta.error("`name` is given twice"));
                }
                container.name = Some(parse(
                    value(&meta, "name", "name = MyCodec")?,
                    "`name` is the name of the codec type to generate, like `name = MyCodec`",
                )?);
            } else if meta.path.is_ident("error") {
                if container.error.is_some() {
                    return Err(meta.error("`error` is given twice"));
                }
                container.error = Some(parse(
                    value(&meta, "error", "error = MyError")?,
                    "`error` is the error type of the codec, like `error = MyError`",
                )?);
            } else if meta.path.is_ident("codec") {
                return Err(meta.error(
                    "`codec` can only be given on a field, not on the struct: \
                     the struct takes `name` and `error`",
                ));
            } else {
                return Err(meta.error("unknown key: the struct takes `name` and `error`"));
            }
            Ok(())
        })?;
    }
    Ok(container)
}

/// The codec of `field`, which its `#[fiole(codec = ...)]` must give: what a
/// field is written with is never implicit.
pub fn field(field: &syn::Field) -> syn::Result<Type> {
    let mut codec = None;
    for attr in fiole(&field.attrs) {
        if !matches!(attr.meta, Meta::List(_)) {
            return Err(syn::Error::new_spanned(
                attr,
                "expected the codec of the field, like `#[fiole(codec = Terminated<Str>)]`",
            ));
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("codec") {
                if codec.is_some() {
                    return Err(meta.error("`codec` is given twice"));
                }
                codec = Some(parse(
                    value(&meta, "codec", "codec = Terminated<Str>")?,
                    "`codec` is the codec the field is written with, like `codec = Terminated<Str>`",
                )?);
            } else if meta.path.is_ident("name") || meta.path.is_ident("error") {
                let key = meta.path.get_ident().expect("checked just above");
                return Err(meta.error(format!(
                    "`{key}` can only be given on the struct, not on a field: \
                     a field takes `codec`, like `#[fiole(codec = Terminated<Str>)]`"
                )));
            } else {
                return Err(meta.error(
                    "unknown key: a field takes `codec`, like `#[fiole(codec = Terminated<Str>)]`",
                ));
            }
            Ok(())
        })?;
    }
    codec.ok_or_else(|| {
        let at = field
            .ident
            .as_ref()
            .map_or_else(|| field.ty.span(), Ident::span);
        syn::Error::new(
            at,
            "this field has no codec: every field must say what it's written with, \
             like `#[fiole(codec = Terminated<Str>)]`, or `#[fiole(codec = Inner)]` \
             for a struct deriving `Fiole`",
        )
    })
}

/// The `#[fiole(...)]` among `attrs`.
fn fiole(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|attr| attr.path().is_ident("fiole"))
}

/// What follows the `=` of the key `key`.
fn value<'a>(meta: &ParseNestedMeta<'a>, key: &str, example: &str) -> syn::Result<ParseStream<'a>> {
    if !meta.input.peek(Token![=]) {
        return Err(meta.error(format!("`{key}` needs a value, like `{example}`")));
    }
    meta.value()
}

/// The value of a key, or `message` where it isn't one.
fn parse<T: syn::parse::Parse>(value: ParseStream, message: &str) -> syn::Result<T> {
    value
        .parse()
        .map_err(|error| syn::Error::new(error.span(), message))
}

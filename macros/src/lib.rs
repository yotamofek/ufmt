//! `μfmt` macros

#![deny(warnings)]

extern crate proc_macro;

use proc_macro::TokenStream;
use std::{borrow::Cow, cmp::Ordering};

use proc_macro2::{Literal, Span};
use quote::{quote, ToTokens};
use syn::{
    parse::{self, Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    Data, DeriveInput, Expr, Fields, GenericParam, Ident, LitStr, Token,
};

/// Automatically derive the `uDebug` trait for a `struct` or `enum`
///
/// Supported items
///
/// - all kind of `struct`-s
/// - all kind of `enum`-s
///
/// `union`-s are not supported
#[proc_macro_derive(uDebug)]
pub fn debug(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let mut generics = input.generics;

    for param in &mut generics.params {
        if let GenericParam::Type(type_param) = param {
            type_param.bounds.push(parse_quote!(ufmt::uDebug));
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let ident = &input.ident;
    let ts = match input.data {
        Data::Struct(data) => {
            let ident_s = ident.to_string();

            let body = match data.fields {
                Fields::Named(fields) => {
                    let fields = fields
                        .named
                        .iter()
                        .map(|field| {
                            let ident = field.ident.as_ref().expect("UNREACHABLE");
                            let name = ident.to_string();

                            quote!(field(#name, &self.#ident)?)
                        })
                        .collect::<Vec<_>>();

                    quote!(f.debug_struct(#ident_s)?#(.#fields)*.finish())
                }

                Fields::Unnamed(fields) => {
                    let fields = (0..fields.unnamed.len())
                        .map(|i| {
                            let i = Literal::u64_unsuffixed(i as u64);

                            quote!(field(&self.#i)?)
                        })
                        .collect::<Vec<_>>();

                    quote!(f.debug_tuple(#ident_s)?#(.#fields)*.finish())
                }

                Fields::Unit => quote!(f.write_str(#ident_s)),
            };

            quote!(
                impl #impl_generics ufmt::uDebug for #ident #ty_generics #where_clause {
                    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
                    where
                        W: ufmt::uWrite + ?Sized,
                    {
                        #body
                    }
                }

            )
        }

        Data::Enum(data) => {
            let arms = data
                .variants
                .iter()
                .map(|var| {
                    let variant = &var.ident;
                    let variant_s = variant.to_string();

                    match &var.fields {
                        Fields::Named(fields) => {
                            let mut pats = Vec::with_capacity(fields.named.len());
                            let mut methods = Vec::with_capacity(fields.named.len());
                            for field in &fields.named {
                                let ident = field.ident.as_ref().unwrap();
                                let ident_s = ident.to_string();

                                pats.push(quote!(#ident));
                                methods.push(quote!(field(#ident_s, #ident)?));
                            }

                            quote!(
                                #ident::#variant { #(#pats),* } => {
                                    f.debug_struct(#variant_s)?#(.#methods)*.finish()
                                }
                            )
                        }

                        Fields::Unnamed(fields) => {
                            let pats = &(0..fields.unnamed.len())
                                .map(|i| Ident::new(&format!("_{}", i), Span::call_site()))
                                .collect::<Vec<_>>();

                            quote!(
                                #ident::#variant(#(#pats),*) => {
                                    f.debug_tuple(#variant_s)?#(.field(#pats)?)*.finish()
                                }
                            )
                        }

                        Fields::Unit => quote!(
                            #ident::#variant => {
                                f.write_str(#variant_s)
                            }
                        ),
                    }
                })
                .collect::<Vec<_>>();

            quote!(
                impl #impl_generics ufmt::uDebug for #ident #ty_generics #where_clause {
                    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
                        where
                        W: ufmt::uWrite + ?Sized,
                    {
                        match self {
                            #(#arms),*
                        }
                    }
                }
            )
        }

        Data::Union(..) => {
            return parse::Error::new(Span::call_site(), "this trait cannot be derived for unions")
                .to_compile_error()
                .into();
        }
    };

    ts.into()
}

#[proc_macro]
pub fn uwrite(input: TokenStream) -> TokenStream {
    write(input, false)
}

#[proc_macro]
pub fn uwriteln(input: TokenStream) -> TokenStream {
    write(input, true)
}

fn write(input: TokenStream, newline: bool) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    let formatter = &input.formatter;
    let literal = input.literal;

    let mut format = literal.value();
    if newline {
        format.push('\n');
    }
    let pieces = match parse(&format, literal.span()) {
        Err(e) => return e.to_compile_error().into(),
        Ok(pieces) => pieces,
    };

    let required_args = pieces
        .iter()
        .filter(|piece| piece.is_positional_arg())
        .count();

    let supplied_args = input.args.len();
    match supplied_args.cmp(&required_args) {
        Ordering::Less => {
            return parse::Error::new(
                literal.span(),
                &format!(
                    "format string requires {} arguments but {} {} supplied",
                    required_args,
                    supplied_args,
                    if supplied_args == 1 { "was" } else { "were" }
                ),
            )
            .to_compile_error()
            .into();
        }
        Ordering::Greater => {
            return parse::Error::new(input.args[required_args].span(), "argument never used")
                .to_compile_error()
                .into();
        }
        _ => {}
    }

    let mut args = vec![];
    let mut pats = vec![];
    let mut pat_idents = (0..).map(mk_ident);
    let mut arg_exprs = input.args.into_iter();
    let exprs = pieces
        .into_iter()
        .map(|piece| match piece {
            Piece::Literal(s) => {
                quote!(f.write_str(#s)?;)
            }
            Piece::Arg {
                arg_type,
                implicit_capture,
            } => {
                let pat = pat_idents.next().unwrap();
                let arg = if let Some(arg) = implicit_capture {
                    Ident::new(arg, Span::call_site()).into_token_stream()
                } else {
                    arg_exprs.next().unwrap().into_token_stream()
                };

                args.push(quote!(&(#arg)));
                pats.push(quote!(#pat));

                match arg_type {
                    FormatArgType::Debug { pretty } => {
                        let expr = quote!(ufmt::uDebug::fmt(#pat, f));
                        if pretty {
                            quote!(f.pretty(|f| #expr)?;)
                        } else {
                            quote!(#expr?;)
                        }
                    }
                    FormatArgType::Display => {
                        quote!(ufmt::uDisplay::fmt(#pat, f)?;)
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote!(match (#(#args),*) {
        (#(#pats),*) => {
            use ufmt::UnstableDoAsFormatter as _;

            (#formatter).do_as_formatter(|f| {
                #(#exprs)*
                Ok(())
            })
        }
    })
    .into()
}

struct Input {
    formatter: Expr,
    _comma: Token![,],
    literal: LitStr,
    _comma2: Option<Token![,]>,
    args: Punctuated<Expr, Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> parse::Result<Self> {
        let formatter = input.parse()?;
        let _comma = input.parse()?;
        let literal: LitStr = input.parse()?;

        if input.is_empty() {
            Ok(Input {
                formatter,
                _comma,
                literal,
                _comma2: None,
                args: Punctuated::new(),
            })
        } else {
            Ok(Input {
                formatter,
                _comma,
                literal,
                _comma2: input.parse()?,
                args: Punctuated::parse_terminated(input)?,
            })
        }
    }
}

#[derive(Debug, PartialEq)]
enum FormatArgType {
    Debug { pretty: bool },
    Display,
}

#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Arg {
        arg_type: FormatArgType,
        implicit_capture: Option<&'a str>,
    },
    Literal(Cow<'a, str>),
}

impl Piece<'_> {
    fn is_positional_arg(&self) -> bool {
        matches!(
            self,
            Piece::Arg {
                implicit_capture: None,
                ..
            }
        )
    }
}

fn mk_ident(i: usize) -> Ident {
    Ident::new(&format!("__{}", i), Span::call_site())
}

// `}}` -> `}`
fn unescape(mut literal: &str, span: Span) -> parse::Result<Cow<'_, str>> {
    const ERR: &str = "format string contains an unmatched right brace";

    if !literal.contains('}') {
        return Ok(Cow::Borrowed(literal));
    }

    let mut buf = String::new();

    while let Some((left, right)) = literal.split_once('}') {
        const ESCAPED_BRACE: &str = "}";

        literal = if let Some(literal) = right.strip_prefix(ESCAPED_BRACE) {
            buf.push_str(left);
            buf.push('}');

            literal
        } else {
            return Err(parse::Error::new(span, ERR));
        }
    }

    buf.push_str(literal);

    Ok(buf.into())
}

fn parse(mut literal: &str, span: Span) -> parse::Result<Vec<Piece>> {
    let mut pieces = vec![];

    let mut buf = String::new();

    while let Some((head, tail)) = literal.split_once('{') {
        const DEBUG: &str = ":?}";
        const DEBUG_PRETTY: &str = ":#?}";
        const DISPLAY: &str = "}";
        const ESCAPED_BRACE: &str = "{";

        let (implicit_capture, tail) = tail
            .find(|c: char| !c.is_alphanumeric())
            .filter(|tail_idx| *tail_idx > 0 && !tail.starts_with(char::is_numeric))
            .map_or((None, tail), |tail_idx| {
                let (ident, tail) = tail.split_at(tail_idx);
                (Some(ident), tail)
            });

        let arg_type;
        (arg_type, literal) = None
            .or_else(|| {
                tail.strip_prefix(DEBUG)
                    .map(|tail| (FormatArgType::Debug { pretty: false }, tail))
            })
            .or_else(|| {
                tail.strip_prefix(DEBUG_PRETTY)
                    .map(|tail| (FormatArgType::Debug { pretty: true }, tail))
            })
            .or_else(|| {
                tail.strip_prefix(DISPLAY)
                    .map(|tail| (FormatArgType::Display, tail))
            })
            .map(|(arg_type, tail)| (Some(arg_type), tail))
            .or_else(|| tail.strip_prefix(ESCAPED_BRACE).map(|tail| (None, tail)))
            .ok_or_else(|| {
                parse::Error::new(
                    span,
                    "invalid format string: expected `{{`, `{}`, `{:?}` or `{:#?}`",
                )
            })?;

        match arg_type {
            Some(arg_type) => {
                match (buf.is_empty(), head.is_empty()) {
                    (true, false) => {
                        pieces.push(Piece::Literal(unescape(head, span)?));
                    }
                    (false, _) => {
                        buf.push_str(&unescape(head, span)?);
                        pieces.push(Piece::Literal(Cow::Owned(buf.split_off(0))));
                    }
                    _ => {}
                }

                pieces.push(Piece::Arg {
                    arg_type,
                    implicit_capture,
                });
            }
            // escaped brace
            None => {
                buf.push_str(&unescape(head, span)?);
                buf.push('{');
            }
        };
    }

    // end of the string literal
    if !literal.is_empty() {
        if buf.is_empty() {
            pieces.push(Piece::Literal(unescape(literal, span)?));
        } else {
            buf.push_str(&unescape(literal, span)?);

            pieces.push(Piece::Literal(Cow::Owned(buf)));
        }
    }

    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use proc_macro2::Span;

    use super::*;

    fn literal(lit: &str) -> Piece {
        Piece::Literal(Cow::Borrowed(lit))
    }

    fn display<'a>() -> Piece<'a> {
        Piece::Arg {
            arg_type: FormatArgType::Display,
            implicit_capture: None,
        }
    }

    fn display_capture(ident: &str) -> Piece {
        Piece::Arg {
            arg_type: FormatArgType::Display,
            implicit_capture: Some(ident),
        }
    }

    fn debug<'a>() -> Piece<'a> {
        Piece::Arg {
            arg_type: FormatArgType::Debug { pretty: false },
            implicit_capture: None,
        }
    }

    fn debug_capture(ident: &str) -> Piece {
        Piece::Arg {
            arg_type: FormatArgType::Debug { pretty: false },
            implicit_capture: Some(ident),
        }
    }

    fn debug_pretty<'a>() -> Piece<'a> {
        Piece::Arg {
            arg_type: FormatArgType::Debug { pretty: true },
            implicit_capture: None,
        }
    }

    fn debug_pretty_capture(ident: &str) -> Piece {
        Piece::Arg {
            arg_type: FormatArgType::Debug { pretty: true },
            implicit_capture: Some(ident),
        }
    }

    #[test]
    fn test_pieces() -> syn::parse::Result<()> {
        let span = Span::call_site();

        assert_eq!(parse("{}", span)?, vec![display()]);
        assert_eq!(parse("{ident}", span)?, vec![display_capture("ident")]);

        assert_eq!(parse("{:?}", span)?, vec![debug()]);
        assert_eq!(parse("{ident:?}", span)?, vec![debug_capture("ident")]);

        assert_eq!(parse("{:#?}", span)?, vec![debug_pretty()]);
        assert_eq!(
            parse("{ident:#?}", span)?,
            vec![debug_pretty_capture("ident")]
        );

        // escaped braces
        assert_eq!(
            parse("This {{}} is not an argument", span)?,
            vec![literal("This {} is not an argument")],
        );

        // complex example
        assert_eq!(
            parse(
                "Hello {name}, and welcome to {:?}! Hope you have {emotion:#?}!",
                span
            )?,
            vec![
                literal("Hello "),
                display_capture("name"),
                literal(", and welcome to "),
                debug(),
                literal("! Hope you have "),
                debug_pretty_capture("emotion"),
                literal("!")
            ]
        );

        // left brace & junk
        assert!(parse("{", span).is_err());
        assert!(parse(" {", span).is_err());
        assert!(parse("{ ", span).is_err());
        assert!(parse("{ {", span).is_err());
        assert!(parse("{:x}", span).is_err());

        Ok(())
    }

    #[test]
    fn test_unescape() {
        let span = Span::call_site();

        // no right brace
        assert_eq!(unescape("", span).ok(), Some(Cow::Borrowed("")));
        assert_eq!(unescape("Hello", span).ok(), Some(Cow::Borrowed("Hello")));

        // unmatched right brace
        assert!(unescape(" }", span).is_err());
        assert!(unescape("} ", span).is_err());
        assert!(unescape("}", span).is_err());

        // escaped right brace
        assert_eq!(unescape("}}", span).ok(), Some(Cow::Borrowed("}")));
        assert_eq!(unescape("}} ", span).ok(), Some(Cow::Borrowed("} ")));
    }
}

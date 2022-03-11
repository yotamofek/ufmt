mod format_str;
mod input;

use std::cmp::Ordering;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{parse, parse_macro_input, spanned::Spanned, Ident};

use self::{
    format_str::{parse_format_str, FormatArgType, Piece},
    input::Input,
};

fn mk_ident(i: usize) -> Ident {
    Ident::new(&format!("__{}", i), Span::call_site())
}

pub(super) fn write(input: TokenStream, newline: bool) -> TokenStream {
    let input = parse_macro_input!(input as Input);

    let formatter = &input.formatter;
    let literal = input.literal;

    let mut format = literal.value();
    if newline {
        format.push('\n');
    }
    let pieces = match parse_format_str(&format, literal.span()) {
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

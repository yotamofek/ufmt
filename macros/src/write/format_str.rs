use std::borrow::Cow;

use proc_macro2::Span;
use syn::parse;

#[derive(Debug, PartialEq)]
pub(super) enum FormatArgType {
    Debug { pretty: bool },
    Display,
}

#[derive(Debug, PartialEq)]
pub(super) enum Piece<'a> {
    Arg {
        arg_type: FormatArgType,
        implicit_capture: Option<&'a str>,
    },
    Literal(Cow<'a, str>),
}

impl Piece<'_> {
    pub(super) fn is_positional_arg(&self) -> bool {
        matches!(
            self,
            Piece::Arg {
                implicit_capture: None,
                ..
            }
        )
    }
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

pub(super) fn parse_format_str(mut literal: &str, span: Span) -> parse::Result<Vec<Piece>> {
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

        assert_eq!(parse_format_str("{}", span)?, vec![display()]);
        assert_eq!(
            parse_format_str("{ident}", span)?,
            vec![display_capture("ident")]
        );

        assert_eq!(parse_format_str("{:?}", span)?, vec![debug()]);
        assert_eq!(
            parse_format_str("{ident:?}", span)?,
            vec![debug_capture("ident")]
        );

        assert_eq!(parse_format_str("{:#?}", span)?, vec![debug_pretty()]);
        assert_eq!(
            parse_format_str("{ident:#?}", span)?,
            vec![debug_pretty_capture("ident")]
        );

        // escaped braces
        assert_eq!(
            parse_format_str("This {{}} is not an argument", span)?,
            vec![literal("This {} is not an argument")],
        );

        // complex example
        assert_eq!(
            parse_format_str(
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
        assert!(parse_format_str("{", span).is_err());
        assert!(parse_format_str(" {", span).is_err());
        assert!(parse_format_str("{ ", span).is_err());
        assert!(parse_format_str("{ {", span).is_err());
        assert!(parse_format_str("{:x}", span).is_err());

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

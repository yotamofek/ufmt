use syn::{
    parse::{self, Parse, ParseStream},
    punctuated::Punctuated,
    Expr, LitStr, Token,
};

pub(super) struct Input {
    pub(super) formatter: Expr,
    _comma: Token![,],
    pub(super) literal: LitStr,
    _comma2: Option<Token![,]>,
    pub(super) args: Punctuated<Expr, Token![,]>,
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

//! `\verb`. Mirrors upstream `functions/verb.ts`.
//!
//! `\verb` is parsed by the lexer (delimiter is the next non-letter
//! character); the handler is only reached if the lexer's regex failed to
//! match a closing delimiter, which is always an error.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    _ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Err(ParseError::new(
        "\\verb ended by end of line instead of matching delimiter",
    ))
}

const NAMES: &[&str] = &["\\verb"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Verb,
    names: NAMES,
    num_args: 0,
    num_optional_args: 0,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
}];

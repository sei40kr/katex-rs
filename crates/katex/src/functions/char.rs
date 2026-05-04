//! `\@char{n}` — internal helper used by the user-facing `\char` macro to
//! produce a single character from a code point. Mirrors upstream
//! `functions/char.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = match &args[0] {
        ParseNode::OrdGroup { body, .. } => body,
        _ => return Err(ParseError::new("\\@char expected an ordgroup")),
    };
    let mut number = String::new();
    for node in body {
        match node {
            ParseNode::TextOrd { text, .. } => number.push_str(text),
            _ => return Err(ParseError::new("\\@char expected a textord run")),
        }
    }
    let code: u32 = number
        .parse()
        .map_err(|_| ParseError::new(format!("\\@char has non-numeric argument {number}")))?;
    if code >= 0x10ffff {
        return Err(ParseError::new(format!(
            "\\@char with invalid code point {number}"
        )));
    }
    let ch = char::from_u32(code)
        .ok_or_else(|| ParseError::new(format!("\\@char with invalid code point {number}")))?;
    Ok(ParseNode::TextOrd {
        mode: ctx.parser.mode,
        loc: None,
        text: SmolStr::new(ch.to_string()),
    })
}

const NAMES: &[&str] = &["\\@char"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::TextOrd,
    names: NAMES,
    num_args: 1,
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

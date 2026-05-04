//! Inline math toggle (`\(`, `$`) and the matching closing checks
//! (`\)`, `\]`). Mirrors upstream `functions/math.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::StyleStr;
use crate::types::Mode;

fn handler_open(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let parser = ctx.parser;
    let outer_mode = parser.mode;
    parser.switch_mode(Mode::Math);
    let close = if ctx.func_name.as_str() == "\\(" {
        "\\)"
    } else {
        "$"
    };
    let body = parser.parse_expression(false, None)?;
    parser.expect(close, true)?;
    parser.switch_mode(outer_mode);
    Ok(ParseNode::Styling {
        mode: parser.mode,
        loc: None,
        style: StyleStr::Text,
        body,
    })
}

fn handler_mismatch(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Err(ParseError::new(format!("Mismatched {}", ctx.func_name)))
}

const OPEN_NAMES: &[&str] = &["\\(", "$"];
const CLOSE_NAMES: &[&str] = &["\\)", "\\]"];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Styling,
        names: OPEN_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: false,
        infix: false,
        primitive: false,
        handler: Some(handler_open),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Text,
        names: CLOSE_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: false,
        infix: false,
        primitive: false,
        handler: Some(handler_mismatch),
        mathml_builder: None,
    },
];

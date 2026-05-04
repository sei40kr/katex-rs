//! `\text`, `\textrm`, `\textsf`, `\texttt`, `\textbf`, … and `\emph`.
//! Mirrors upstream `functions/text.ts`.

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::types::Mode;

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = ord_argument(args[0].clone());
    Ok(ParseNode::Text {
        mode: ctx.parser.mode,
        loc: None,
        body,
        font: Some(ctx.func_name.clone()),
    })
}

const NAMES: &[&str] = &[
    // Font families
    "\\text",
    "\\textrm",
    "\\textsf",
    "\\texttt",
    "\\textnormal",
    // Font weights
    "\\textbf",
    "\\textmd",
    // Font Shapes
    "\\textit",
    "\\textup",
    "\\emph",
];
const ARGS: &[ArgType] = &[ArgType::Mode(Mode::Text)];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Text,
    names: NAMES,
    num_args: 1,
    num_optional_args: 0,
    arg_types: ARGS,
    allowed_in_argument: true,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

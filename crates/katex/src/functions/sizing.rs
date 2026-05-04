//! `\tiny`, `\Large`, `\Huge`, … — implicit-body size directives. Mirrors
//! upstream `functions/sizing.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

const SIZE_FUNCS: &[&str] = &[
    "\\tiny",
    "\\sixptsize",
    "\\scriptsize",
    "\\footnotesize",
    "\\small",
    "\\normalsize",
    "\\large",
    "\\Large",
    "\\LARGE",
    "\\huge",
    "\\Huge",
];

fn handler(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let break_on = ctx.break_on_token_text;
    let parser = ctx.parser;
    let body = parser.parse_expression(false, break_on)?;
    let size = SIZE_FUNCS
        .iter()
        .position(|n| *n == ctx.func_name.as_str())
        .map(|i| (i + 1) as u8)
        .unwrap_or(0);
    Ok(ParseNode::Sizing {
        mode: parser.mode,
        loc: None,
        size,
        body,
    })
}

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Sizing,
    names: SIZE_FUNCS,
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
    html_builder: None,
}];

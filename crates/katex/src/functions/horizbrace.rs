//! `\overbrace`, `\underbrace`, `\overbracket`, `\underbracket`. Mirrors
//! upstream `functions/horizBrace.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let func_name = ctx.func_name.as_str();
    Ok(ParseNode::HorizBrace {
        mode: ctx.parser.mode,
        loc: None,
        label: ctx.func_name.clone(),
        is_over: func_name.contains("\\over"),
        base: Box::new(args[0].clone()),
    })
}

const NAMES: &[&str] = &[
    "\\overbrace",
    "\\underbrace",
    "\\overbracket",
    "\\underbracket",
];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::HorizBrace,
    names: NAMES,
    num_args: 1,
    num_optional_args: 0,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: false,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
}];

//! `\underleftarrow`, `\underrightarrow`, … — bottom accents. Mirrors
//! upstream `functions/accentunder.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let base = args[0].clone();
    Ok(ParseNode::AccentUnder {
        mode: ctx.parser.mode,
        loc: None,
        label: ctx.func_name.clone(),
        is_stretchy: true,
        is_shifty: false,
        base: Box::new(base),
    })
}

const NAMES: &[&str] = &[
    "\\underleftarrow",
    "\\underrightarrow",
    "\\underleftrightarrow",
    "\\undergroup",
    "\\underlinesegment",
    "\\utilde",
];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::AccentUnder,
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
    html_builder: None,
}];

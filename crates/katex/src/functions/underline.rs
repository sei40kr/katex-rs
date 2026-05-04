//! `\underline`. Mirrors upstream `functions/underline.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::Underline {
        mode: ctx.parser.mode,
        loc: None,
        body: Box::new(args[0].clone()),
    })
}

const NAMES: &[&str] = &["\\underline"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Underline,
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
    html_builder: None,
}];

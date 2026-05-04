//! `\sqrt[index]{body}`. Mirrors upstream `functions/sqrt.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let index = opt_args.first().cloned().flatten().map(Box::new);
    Ok(ParseNode::Sqrt {
        mode: ctx.parser.mode,
        loc: None,
        body: Box::new(args[0].clone()),
        index,
    })
}

const NAMES: &[&str] = &["\\sqrt"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Sqrt,
    names: NAMES,
    num_args: 1,
    num_optional_args: 1,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: false,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
}];

//! `\mathchoice{D}{T}{S}{SS}`. Mirrors upstream `functions/mathchoice.ts`.

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::MathChoice {
        mode: ctx.parser.mode,
        loc: None,
        display: ord_argument(args[0].clone()),
        text: ord_argument(args[1].clone()),
        script: ord_argument(args[2].clone()),
        scriptscript: ord_argument(args[3].clone()),
    })
}

const NAMES: &[&str] = &["\\mathchoice"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::MathChoice,
    names: NAMES,
    num_args: 4,
    num_optional_args: 0,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: false,
    allowed_in_math: true,
    infix: false,
    primitive: true,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

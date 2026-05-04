//! `\raisebox{dy}{body}`. Mirrors upstream `functions/raisebox.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let dy = match &args[0] {
        ParseNode::Size { value, .. } => *value,
        _ => return Err(ParseError::new("\\raisebox expected a size argument")),
    };
    Ok(ParseNode::RaiseBox {
        mode: ctx.parser.mode,
        loc: None,
        dy,
        body: Box::new(args[1].clone()),
    })
}

const NAMES: &[&str] = &["\\raisebox"];
const ARGS: &[ArgType] = &[ArgType::Size, ArgType::Hbox];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::RaiseBox,
    names: NAMES,
    num_args: 2,
    num_optional_args: 0,
    arg_types: ARGS,
    allowed_in_argument: false,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

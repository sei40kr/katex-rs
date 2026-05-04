//! `\rule[shift]{width}{height}`. Mirrors upstream `functions/rule.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let shift = match opt_args.first().and_then(|o| o.clone()) {
        Some(ParseNode::Size { value, .. }) => Some(value),
        _ => None,
    };
    let width = match &args[0] {
        ParseNode::Size { value, .. } => *value,
        _ => return Err(ParseError::new("\\rule expected size for width")),
    };
    let height = match &args[1] {
        ParseNode::Size { value, .. } => *value,
        _ => return Err(ParseError::new("\\rule expected size for height")),
    };
    Ok(ParseNode::Rule {
        mode: ctx.parser.mode,
        loc: None,
        shift,
        width,
        height,
    })
}

const NAMES: &[&str] = &["\\rule"];
const ARGS: &[ArgType] = &[ArgType::Size, ArgType::Size, ArgType::Size];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Rule,
    names: NAMES,
    num_args: 2,
    num_optional_args: 1,
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

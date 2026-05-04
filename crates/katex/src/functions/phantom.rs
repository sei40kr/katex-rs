//! `\phantom` and `\vphantom`. Mirrors upstream `functions/phantom.ts`.

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler_phantom(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::Phantom {
        mode: ctx.parser.mode,
        loc: None,
        body: ord_argument(args[0].clone()),
    })
}

fn handler_vphantom(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::VPhantom {
        mode: ctx.parser.mode,
        loc: None,
        body: Box::new(args[0].clone()),
    })
}

const PHANTOM_NAMES: &[&str] = &["\\phantom"];
const VPHANTOM_NAMES: &[&str] = &["\\vphantom"];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Phantom,
        names: PHANTOM_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_phantom),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::VPhantom,
        names: VPHANTOM_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_vphantom),
        mathml_builder: None,
        html_builder: None,
    },
];

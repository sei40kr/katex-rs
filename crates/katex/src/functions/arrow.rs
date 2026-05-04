//! `\xleftarrow`, `\xrightarrow`, … and CD-environment variants. Mirrors
//! upstream `functions/arrow.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::XArrow {
        mode: ctx.parser.mode,
        loc: None,
        label: ctx.func_name.clone(),
        body: Box::new(args[0].clone()),
        below: opt_args.first().cloned().flatten().map(Box::new),
    })
}

const NAMES: &[&str] = &[
    "\\xleftarrow",
    "\\xrightarrow",
    "\\xLeftarrow",
    "\\xRightarrow",
    "\\xleftrightarrow",
    "\\xLeftrightarrow",
    "\\xhookleftarrow",
    "\\xhookrightarrow",
    "\\xmapsto",
    "\\xrightharpoondown",
    "\\xrightharpoonup",
    "\\xleftharpoondown",
    "\\xleftharpoonup",
    "\\xrightleftharpoons",
    "\\xleftrightharpoons",
    "\\xlongequal",
    "\\xtwoheadrightarrow",
    "\\xtwoheadleftarrow",
    "\\xtofrom",
    // mhchem extension
    "\\xrightleftarrows",
    "\\xrightequilibrium",
    "\\xleftequilibrium",
    // CD environment
    "\\\\cdrightarrow",
    "\\\\cdleftarrow",
    "\\\\cdlongequal",
];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::XArrow,
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

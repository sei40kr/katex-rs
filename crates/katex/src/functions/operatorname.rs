//! `\operatorname@` and `\operatornamewithlimits` (the user-facing
//! `\operatorname` is wired up via a macro). Mirrors upstream
//! `functions/operatorname.ts`.

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = ord_argument(args[0].clone());
    let always_handle_supsub = ctx.func_name.as_str() == "\\operatornamewithlimits";
    Ok(ParseNode::OperatorName {
        mode: ctx.parser.mode,
        loc: None,
        body,
        always_handle_supsub,
        limits: false,
        parent_is_supsub: false,
    })
}

const NAMES: &[&str] = &["\\operatorname@", "\\operatornamewithlimits"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::OperatorName,
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

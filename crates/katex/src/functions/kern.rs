//! `\kern`, `\mkern`, `\hskip`, `\mskip`. Mirrors upstream
//! `functions/kern.ts`. Strict-mode reporting deferred to the strict-mode
//! milestone.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let dimension = match &args[0] {
        ParseNode::Size { value, .. } => *value,
        _ => return Err(ParseError::new("kern expected a size argument")),
    };
    // TODO(strict): mu-vs-non-mu unit checks via reportNonstrict.
    Ok(ParseNode::Kern {
        mode: ctx.parser.mode,
        loc: None,
        dimension,
    })
}

const NAMES: &[&str] = &["\\kern", "\\mkern", "\\hskip", "\\mskip"];
const ARGS: &[ArgType] = &[ArgType::Size];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Kern,
    names: NAMES,
    num_args: 1,
    num_optional_args: 0,
    arg_types: ARGS,
    allowed_in_argument: false,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: true,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

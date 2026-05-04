//! `\mathllap`, `\mathrlap`, `\mathclap`. Mirrors upstream
//! `functions/lap.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    // funcName is `\mathllap` / `\mathrlap` / `\mathclap`; alignment is
    // the suffix after the 5-char `\math` prefix.
    let alignment = SmolStr::new(&ctx.func_name.as_str()[5..]);
    Ok(ParseNode::Lap {
        mode: ctx.parser.mode,
        loc: None,
        alignment,
        body: Box::new(args[0].clone()),
    })
}

const NAMES: &[&str] = &["\\mathllap", "\\mathrlap", "\\mathclap"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Lap,
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

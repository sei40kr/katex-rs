//! `\displaystyle`, `\textstyle`, `\scriptstyle`, `\scriptscriptstyle`.
//! Mirrors upstream `functions/styling.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::StyleStr;

fn style_from_func_name(name: &str) -> Option<StyleStr> {
    match name {
        "\\displaystyle" => Some(StyleStr::Display),
        "\\textstyle" => Some(StyleStr::Text),
        "\\scriptstyle" => Some(StyleStr::Script),
        "\\scriptscriptstyle" => Some(StyleStr::ScriptScript),
        _ => None,
    }
}

fn handler(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let break_on = ctx.break_on_token_text;
    let style = style_from_func_name(ctx.func_name.as_str())
        .ok_or_else(|| ParseError::new("Unknown styling command"))?;
    let body = ctx.parser.parse_expression(true, break_on)?;
    Ok(ParseNode::Styling {
        mode: ctx.parser.mode,
        loc: None,
        style,
        body,
    })
}

const NAMES: &[&str] = &[
    "\\displaystyle",
    "\\textstyle",
    "\\scriptstyle",
    "\\scriptscriptstyle",
];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Styling,
    names: NAMES,
    num_args: 0,
    num_optional_args: 0,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: true,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

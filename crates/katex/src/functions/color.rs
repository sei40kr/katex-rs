//! `\textcolor` and `\color`. Mirrors upstream `functions/color.ts`.

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::macro_expander::MacroDefinition;
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;

fn handler_textcolor(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let color = match &args[0] {
        ParseNode::ColorToken { color, .. } => color.clone(),
        _ => return Err(ParseError::new("\\textcolor expected color-token")),
    };
    let body = ord_argument(args[1].clone());
    Ok(ParseNode::Color {
        mode: ctx.parser.mode,
        loc: None,
        color,
        body,
    })
}

fn handler_color(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let color = match &args[0] {
        ParseNode::ColorToken { color, .. } => color.clone(),
        _ => return Err(ParseError::new("\\color expected color-token")),
    };
    // Mirror color.sty: store the current color in the gullet so a later
    // `\right` can pick it up.
    ctx.parser.gullet.macros.set(
        "\\current@color",
        Some(MacroDefinition::Source(color.clone())),
        false,
    );
    let break_on = ctx.break_on_token_text;
    let body = ctx.parser.parse_expression(true, break_on)?;
    Ok(ParseNode::Color {
        mode: ctx.parser.mode,
        loc: None,
        color,
        body,
    })
}

const TEXTCOLOR_ARGS: &[ArgType] = &[ArgType::Color, ArgType::Original];
const COLOR_ARGS: &[ArgType] = &[ArgType::Color];

const TEXTCOLOR_NAMES: &[&str] = &["\\textcolor"];
const COLOR_NAMES: &[&str] = &["\\color"];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Color,
        names: TEXTCOLOR_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: TEXTCOLOR_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_textcolor),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Color,
        names: COLOR_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: COLOR_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_color),
        mathml_builder: None,
    },
];

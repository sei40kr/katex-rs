//! `\fbox`, `\colorbox`, `\fcolorbox`, `\cancel`, `\bcancel`, `\xcancel`,
//! `\sout`, `\phase`, `\angl`. Mirrors upstream `functions/enclose.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::types::Mode;

fn make(
    mode: Mode,
    label: &str,
    body: ParseNode,
    background_color: Option<SmolStr>,
    border_color: Option<SmolStr>,
) -> ParseNode {
    ParseNode::Enclose {
        mode,
        loc: None,
        label: SmolStr::new(label),
        background_color,
        border_color,
        body: Box::new(body),
    }
}

fn handler_colorbox(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let color = match &args[0] {
        ParseNode::ColorToken { color, .. } => color.clone(),
        _ => return Err(ParseError::new("\\colorbox expected color-token")),
    };
    Ok(make(
        ctx.parser.mode,
        ctx.func_name.as_str(),
        args[1].clone(),
        Some(color),
        None,
    ))
}

fn handler_fcolorbox(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let border_color = match &args[0] {
        ParseNode::ColorToken { color, .. } => color.clone(),
        _ => return Err(ParseError::new("\\fcolorbox expected color-token")),
    };
    let background_color = match &args[1] {
        ParseNode::ColorToken { color, .. } => color.clone(),
        _ => return Err(ParseError::new("\\fcolorbox expected color-token")),
    };
    Ok(make(
        ctx.parser.mode,
        ctx.func_name.as_str(),
        args[2].clone(),
        Some(background_color),
        Some(border_color),
    ))
}

fn handler_fbox(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(make(ctx.parser.mode, "\\fbox", args[0].clone(), None, None))
}

fn handler_cancel(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(make(
        ctx.parser.mode,
        ctx.func_name.as_str(),
        args[0].clone(),
        None,
        None,
    ))
}

fn handler_sout(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    // TODO(strict): reportNonstrict("mathVsSout", ...) when in math mode.
    Ok(make(
        ctx.parser.mode,
        ctx.func_name.as_str(),
        args[0].clone(),
        None,
        None,
    ))
}

fn handler_angl(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(make(ctx.parser.mode, "\\angl", args[0].clone(), None, None))
}

const COLORBOX_NAMES: &[&str] = &["\\colorbox"];
const FCOLORBOX_NAMES: &[&str] = &["\\fcolorbox"];
const FBOX_NAMES: &[&str] = &["\\fbox"];
const CANCEL_NAMES: &[&str] = &["\\cancel", "\\bcancel", "\\xcancel", "\\phase"];
const SOUT_NAMES: &[&str] = &["\\sout"];
const ANGL_NAMES: &[&str] = &["\\angl"];

const COLORBOX_ARGS: &[ArgType] = &[ArgType::Color, ArgType::Mode(Mode::Text)];
const FCOLORBOX_ARGS: &[ArgType] = &[ArgType::Color, ArgType::Color, ArgType::Mode(Mode::Text)];
const FBOX_ARGS: &[ArgType] = &[ArgType::Hbox];
const ANGL_ARGS: &[ArgType] = &[ArgType::Hbox];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Enclose,
        names: COLORBOX_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: COLORBOX_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_colorbox),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Enclose,
        names: FCOLORBOX_NAMES,
        num_args: 3,
        num_optional_args: 0,
        arg_types: FCOLORBOX_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_fcolorbox),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Enclose,
        names: FBOX_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: FBOX_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_fbox),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Enclose,
        names: CANCEL_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_cancel),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Enclose,
        names: SOUT_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_sout),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Enclose,
        names: ANGL_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: ANGL_ARGS,
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_angl),
        mathml_builder: None,
    },
];

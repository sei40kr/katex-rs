//! `\frac` / `\binom` / `\genfrac` and the infix `\over` / `\atop` / …
//! family. Mirrors upstream `functions/genfrac.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec, normalize_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::{ArgType, Atom, StyleStr};
use crate::types::Mode;
use crate::units::Measurement;

fn wrap_with_style(node: ParseNode, style: Option<StyleStr>, mode: Mode) -> ParseNode {
    match style {
        None => node,
        Some(s) => ParseNode::Styling {
            mode,
            loc: None,
            style: s,
            body: vec![node],
        },
    }
}

fn handler_frac(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let numer = args[0].clone();
    let denom = args[1].clone();
    let func_name = ctx.func_name.as_str();
    let (has_bar_line, left_delim, right_delim): (bool, Option<SmolStr>, Option<SmolStr>) =
        match func_name {
            "\\cfrac" | "\\dfrac" | "\\frac" | "\\tfrac" => (true, None, None),
            "\\\\atopfrac" => (false, None, None),
            "\\dbinom" | "\\binom" | "\\tbinom" => (
                false,
                Some(SmolStr::new_static("(")),
                Some(SmolStr::new_static(")")),
            ),
            "\\\\bracefrac" => (
                false,
                Some(SmolStr::new_static("\\{")),
                Some(SmolStr::new_static("\\}")),
            ),
            "\\\\brackfrac" => (
                false,
                Some(SmolStr::new_static("[")),
                Some(SmolStr::new_static("]")),
            ),
            _ => return Err(ParseError::new("Unrecognized genfrac command")),
        };
    let continued = func_name == "\\cfrac";
    let style = if continued || func_name.starts_with("\\d") {
        Some(StyleStr::Display)
    } else if func_name.starts_with("\\t") {
        Some(StyleStr::Text)
    } else {
        None
    };
    let frac = ParseNode::GenFrac {
        mode: ctx.parser.mode,
        loc: None,
        continued,
        numer: Box::new(numer),
        denom: Box::new(denom),
        has_bar_line,
        left_delim,
        right_delim,
        bar_size: None,
    };
    Ok(wrap_with_style(frac, style, ctx.parser.mode))
}

fn handler_infix(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let replace_with = match ctx.func_name.as_str() {
        "\\over" => "\\frac",
        "\\choose" => "\\binom",
        "\\atop" => "\\\\atopfrac",
        "\\brace" => "\\\\bracefrac",
        "\\brack" => "\\\\brackfrac",
        _ => return Err(ParseError::new("Unrecognized infix genfrac command")),
    };
    Ok(ParseNode::Infix {
        mode: ctx.parser.mode,
        loc: None,
        replace_with: SmolStr::new(replace_with),
        size: None,
        token: ctx.token,
    })
}

fn delim_from_value(s: &str) -> Option<SmolStr> {
    if s.is_empty() || s == "." {
        None
    } else {
        Some(SmolStr::new(s))
    }
}

fn handler_genfrac(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let numer = args[4].clone();
    let denom = args[5].clone();
    let left_delim = match normalize_argument(args[0].clone()) {
        ParseNode::Atom {
            family: Atom::Open,
            text,
            ..
        } => delim_from_value(&text),
        _ => None,
    };
    let right_delim = match normalize_argument(args[1].clone()) {
        ParseNode::Atom {
            family: Atom::Close,
            text,
            ..
        } => delim_from_value(&text),
        _ => None,
    };
    let bar_node = match &args[2] {
        ParseNode::Size {
            value, is_blank, ..
        } => (*value, *is_blank),
        _ => return Err(ParseError::new("\\genfrac expected a size argument")),
    };
    let (has_bar_line, bar_size) = if bar_node.1 {
        (true, None)
    } else {
        (bar_node.0.number > 0.0, Some(bar_node.0))
    };
    // Style: arg3 is either an `ordgroup` containing a textord digit, or a
    // bare textord with the digit. Empty ordgroup -> no style override.
    let size = match &args[3] {
        ParseNode::OrdGroup { body, .. } => {
            if body.is_empty() {
                None
            } else if let ParseNode::TextOrd { text, .. } = &body[0] {
                style_from_digit(text.as_str())
            } else {
                None
            }
        }
        ParseNode::TextOrd { text, .. } => style_from_digit(text.as_str()),
        _ => None,
    };
    let frac = ParseNode::GenFrac {
        mode: ctx.parser.mode,
        loc: None,
        continued: false,
        numer: Box::new(numer),
        denom: Box::new(denom),
        has_bar_line,
        left_delim,
        right_delim,
        bar_size,
    };
    Ok(wrap_with_style(frac, size, ctx.parser.mode))
}

fn style_from_digit(s: &str) -> Option<StyleStr> {
    match s {
        "0" => Some(StyleStr::Display),
        "1" => Some(StyleStr::Text),
        "2" => Some(StyleStr::Script),
        "3" => Some(StyleStr::ScriptScript),
        _ => None,
    }
}

fn handler_above(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let size = match &args[0] {
        ParseNode::Size { value, .. } => Some(*value),
        _ => return Err(ParseError::new("\\above expected a size argument")),
    };
    Ok(ParseNode::Infix {
        mode: ctx.parser.mode,
        loc: None,
        replace_with: SmolStr::new_static("\\\\abovefrac"),
        size,
        token: ctx.token,
    })
}

fn handler_abovefrac(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let numer = args[0].clone();
    let bar_size: Measurement = match &args[1] {
        ParseNode::Infix { size: Some(s), .. } => *s,
        _ => return Err(ParseError::new("\\\\abovefrac expected a size")),
    };
    let denom = args[2].clone();
    let has_bar_line = bar_size.number > 0.0;
    Ok(ParseNode::GenFrac {
        mode: ctx.parser.mode,
        loc: None,
        continued: false,
        numer: Box::new(numer),
        denom: Box::new(denom),
        has_bar_line,
        left_delim: None,
        right_delim: None,
        bar_size: Some(bar_size),
    })
}

const FRAC_NAMES: &[&str] = &[
    "\\cfrac",
    "\\dfrac",
    "\\frac",
    "\\tfrac",
    "\\dbinom",
    "\\binom",
    "\\tbinom",
    "\\\\atopfrac",
    "\\\\bracefrac",
    "\\\\brackfrac",
];
const INFIX_NAMES: &[&str] = &["\\over", "\\choose", "\\atop", "\\brace", "\\brack"];
const GENFRAC_NAMES: &[&str] = &["\\genfrac"];
const ABOVE_NAMES: &[&str] = &["\\above"];
const ABOVEFRAC_NAMES: &[&str] = &["\\\\abovefrac"];

const GENFRAC_ARGS: &[ArgType] = &[
    ArgType::Mode(Mode::Math),
    ArgType::Mode(Mode::Math),
    ArgType::Size,
    ArgType::Mode(Mode::Text),
    ArgType::Mode(Mode::Math),
    ArgType::Mode(Mode::Math),
];
const ABOVE_ARGS: &[ArgType] = &[ArgType::Size];
const ABOVEFRAC_ARGS: &[ArgType] = &[
    ArgType::Mode(Mode::Math),
    ArgType::Size,
    ArgType::Mode(Mode::Math),
];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::GenFrac,
        names: FRAC_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: true,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_frac),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Infix,
        names: INFIX_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: true,
        primitive: false,
        handler: Some(handler_infix),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::GenFrac,
        names: GENFRAC_NAMES,
        num_args: 6,
        num_optional_args: 0,
        arg_types: GENFRAC_ARGS,
        allowed_in_argument: true,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_genfrac),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Infix,
        names: ABOVE_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: ABOVE_ARGS,
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: true,
        primitive: false,
        handler: Some(handler_above),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::GenFrac,
        names: ABOVEFRAC_NAMES,
        num_args: 3,
        num_optional_args: 0,
        arg_types: ABOVEFRAC_ARGS,
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_abovefrac),
        mathml_builder: None,
    },
];

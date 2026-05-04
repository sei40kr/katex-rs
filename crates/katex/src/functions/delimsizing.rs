//! `\big`, `\Big`, `\bigg`, `\Bigg`, plus `\left` / `\right` / `\middle`.
//! Mirrors upstream `functions/delimsizing.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::macro_expander::MacroDefinition;
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;

const DELIMITERS: &[&str] = &[
    "(",
    "\\lparen",
    ")",
    "\\rparen",
    "[",
    "\\lbrack",
    "]",
    "\\rbrack",
    "\\{",
    "\\lbrace",
    "\\}",
    "\\rbrace",
    "\\lfloor",
    "\\rfloor",
    "\u{230a}",
    "\u{230b}",
    "\\lceil",
    "\\rceil",
    "\u{2308}",
    "\u{2309}",
    "<",
    ">",
    "\\langle",
    "\u{27e8}",
    "\\rangle",
    "\u{27e9}",
    "\\lt",
    "\\gt",
    "\\lvert",
    "\\rvert",
    "\\lVert",
    "\\rVert",
    "\\lgroup",
    "\\rgroup",
    "\u{27ee}",
    "\u{27ef}",
    "\\lmoustache",
    "\\rmoustache",
    "\u{23b0}",
    "\u{23b1}",
    "/",
    "\\backslash",
    "|",
    "\\vert",
    "\\|",
    "\\Vert",
    "\\uparrow",
    "\\Uparrow",
    "\\downarrow",
    "\\Downarrow",
    "\\updownarrow",
    "\\Updownarrow",
    ".",
];

fn check_delimiter(delim: &ParseNode, func_name: &str) -> Result<SmolStr, ParseError> {
    let text = match delim {
        ParseNode::Atom { text, .. } => text,
        ParseNode::MathOrd { text, .. } => text,
        ParseNode::TextOrd { text, .. } => text,
        ParseNode::Spacing { text, .. } => text,
        ParseNode::AccentToken { text, .. } => text,
        ParseNode::OpToken { text, .. } => text,
        other => {
            return Err(ParseError::new(format!(
                "Invalid delimiter type '{}'",
                other.node_type().as_str()
            )));
        }
    };
    if DELIMITERS.iter().any(|d| *d == text.as_str()) {
        Ok(text.clone())
    } else {
        Err(ParseError::new(format!(
            "Invalid delimiter '{}' after '{}'",
            text, func_name
        )))
    }
}

/// Returns `(mclass, size)` for one of the 16 `\bigl` / `\Bigl` / … names.
fn size_info(name: &str) -> (&'static str, u8) {
    match name {
        "\\bigl" => ("mopen", 1),
        "\\Bigl" => ("mopen", 2),
        "\\biggl" => ("mopen", 3),
        "\\Biggl" => ("mopen", 4),
        "\\bigr" => ("mclose", 1),
        "\\Bigr" => ("mclose", 2),
        "\\biggr" => ("mclose", 3),
        "\\Biggr" => ("mclose", 4),
        "\\bigm" => ("mrel", 1),
        "\\Bigm" => ("mrel", 2),
        "\\biggm" => ("mrel", 3),
        "\\Biggm" => ("mrel", 4),
        "\\big" => ("mord", 1),
        "\\Big" => ("mord", 2),
        "\\bigg" => ("mord", 3),
        "\\Bigg" => ("mord", 4),
        _ => ("mord", 1),
    }
}

fn handler_size(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let delim = check_delimiter(&args[0], ctx.func_name.as_str())?;
    let (mclass, size) = size_info(ctx.func_name.as_str());
    Ok(ParseNode::DelimSizing {
        mode: ctx.parser.mode,
        loc: None,
        size,
        mclass: SmolStr::new_static(mclass),
        delim,
    })
}

fn handler_right(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let color = match ctx.parser.gullet.macros.get("\\current@color") {
        Some(MacroDefinition::Source(s)) => Some(s.clone()),
        Some(_) => {
            return Err(ParseError::new(
                "\\current@color set to non-string in \\right",
            ));
        }
        None => None,
    };
    let delim = check_delimiter(&args[0], ctx.func_name.as_str())?;
    Ok(ParseNode::LeftRightRight {
        mode: ctx.parser.mode,
        loc: None,
        delim,
        color,
    })
}

fn handler_left(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let left_delim = check_delimiter(&args[0], ctx.func_name.as_str())?;
    let parser = ctx.parser;
    parser.leftright_depth += 1;
    let body = parser.parse_expression(false, None)?;
    parser.leftright_depth -= 1;
    parser.expect("\\right", false)?;
    let right = parser
        .parse_function(None, None)?
        .ok_or_else(|| ParseError::new("Expected \\right after \\left"))?;
    let (right_delim, right_color) = match right {
        ParseNode::LeftRightRight { delim, color, .. } => (delim, color),
        _ => return Err(ParseError::new("\\left expected matching \\right")),
    };
    Ok(ParseNode::LeftRight {
        mode: parser.mode,
        loc: None,
        body,
        left: left_delim,
        right: right_delim,
        right_color,
    })
}

fn handler_middle(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let delim = check_delimiter(&args[0], ctx.func_name.as_str())?;
    if ctx.parser.leftright_depth == 0 {
        return Err(ParseError::new("\\middle without preceding \\left"));
    }
    Ok(ParseNode::Middle {
        mode: ctx.parser.mode,
        loc: None,
        delim,
    })
}

const SIZE_NAMES: &[&str] = &[
    "\\bigl", "\\Bigl", "\\biggl", "\\Biggl", "\\bigr", "\\Bigr", "\\biggr", "\\Biggr", "\\bigm",
    "\\Bigm", "\\biggm", "\\Biggm", "\\big", "\\Big", "\\bigg", "\\Bigg",
];
const RIGHT_NAMES: &[&str] = &["\\right"];
const LEFT_NAMES: &[&str] = &["\\left"];
const MIDDLE_NAMES: &[&str] = &["\\middle"];
const PRIMITIVE_ARGS: &[ArgType] = &[ArgType::Primitive];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::DelimSizing,
        names: SIZE_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: PRIMITIVE_ARGS,
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_size),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::LeftRightRight,
        names: RIGHT_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: true,
        handler: Some(handler_right),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::LeftRight,
        names: LEFT_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: true,
        handler: Some(handler_left),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Middle,
        names: MIDDLE_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: true,
        handler: Some(handler_middle),
        mathml_builder: None,
    },
];

//! `\hat`, `\widehat`, …, plus the text-mode accent set. Mirrors upstream
//! `functions/accent.ts`. MathML/HTML builders land in Phase 6+.

use crate::define_function::{FunctionContext, FunctionSpec, normalize_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::types::Mode;

/// Math-mode accents that render as a fixed-width glyph (i.e. *not*
/// stretchy). Upstream encodes this as a regex; literal-set lookup is
/// equivalent and faster.
const NON_STRETCHY_ACCENTS: &[&str] = &[
    "\\acute",
    "\\grave",
    "\\ddot",
    "\\tilde",
    "\\bar",
    "\\breve",
    "\\check",
    "\\hat",
    "\\vec",
    "\\dot",
    "\\mathring",
];

fn handler_math(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let base = normalize_argument(args[0].clone());
    let func_name = ctx.func_name.as_str();
    let is_stretchy = !NON_STRETCHY_ACCENTS.contains(&func_name);
    let is_shifty = !is_stretchy
        || func_name == "\\widehat"
        || func_name == "\\widetilde"
        || func_name == "\\widecheck";
    Ok(ParseNode::Accent {
        mode: ctx.parser.mode,
        loc: None,
        label: ctx.func_name.clone(),
        is_stretchy,
        is_shifty,
        base: Box::new(base),
    })
}

fn handler_text(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let base = args[0].clone();
    let mode = if ctx.parser.mode == Mode::Math {
        // TODO(strict): reportNonstrict("mathVsTextAccents", ...)
        Mode::Text
    } else {
        ctx.parser.mode
    };
    Ok(ParseNode::Accent {
        mode,
        loc: None,
        label: ctx.func_name.clone(),
        is_stretchy: false,
        is_shifty: true,
        base: Box::new(base),
    })
}

const MATH_NAMES: &[&str] = &[
    "\\acute",
    "\\grave",
    "\\ddot",
    "\\tilde",
    "\\bar",
    "\\breve",
    "\\check",
    "\\hat",
    "\\vec",
    "\\dot",
    "\\mathring",
    "\\widecheck",
    "\\widehat",
    "\\widetilde",
    "\\overrightarrow",
    "\\overleftarrow",
    "\\Overrightarrow",
    "\\overleftrightarrow",
    "\\overgroup",
    "\\overlinesegment",
    "\\overleftharpoon",
    "\\overrightharpoon",
];

const TEXT_NAMES: &[&str] = &[
    "\\'",
    "\\`",
    "\\^",
    "\\~",
    "\\=",
    "\\u",
    "\\.",
    "\\\"",
    "\\c",
    "\\r",
    "\\H",
    "\\v",
    "\\textcircled",
];

const TEXT_ARG_TYPES: &[ArgType] = &[ArgType::Primitive];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Accent,
        names: MATH_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_math),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Accent,
        names: TEXT_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: TEXT_ARG_TYPES,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_text),
        mathml_builder: None,
        html_builder: None,
    },
];

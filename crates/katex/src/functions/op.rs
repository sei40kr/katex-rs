//! Big operators (`\sum`, `\int`, `\bigcup`, …) plus `\mathop` and the
//! function-name operators (`\sin`, `\cos`, …). Mirrors upstream
//! `functions/op.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, OpBody, ParseNode};

fn single_char_big_op(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{220F}' => "\\prod",
        '\u{2210}' => "\\coprod",
        '\u{2211}' => "\\sum",
        '\u{22c0}' => "\\bigwedge",
        '\u{22c1}' => "\\bigvee",
        '\u{22c2}' => "\\bigcap",
        '\u{22c3}' => "\\bigcup",
        '\u{2a00}' => "\\bigodot",
        '\u{2a01}' => "\\bigoplus",
        '\u{2a02}' => "\\bigotimes",
        '\u{2a04}' => "\\biguplus",
        '\u{2a06}' => "\\bigsqcup",
        _ => return None,
    })
}

fn single_char_integral(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{222b}' => "\\int",
        '\u{222c}' => "\\iint",
        '\u{222d}' => "\\iiint",
        '\u{222e}' => "\\oint",
        '\u{222f}' => "\\oiint",
        '\u{2230}' => "\\oiiint",
        _ => return None,
    })
}

fn handler_big_ops(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let mut name = ctx.func_name.clone();
    if name.chars().count() == 1
        && let Some(canon) = single_char_big_op(name.chars().next().unwrap())
    {
        name = SmolStr::new(canon);
    }
    Ok(ParseNode::Op {
        mode: ctx.parser.mode,
        loc: None,
        limits: true,
        always_handle_supsub: false,
        suppress_base_shift: false,
        parent_is_supsub: false,
        body: OpBody::Symbol(name),
    })
}

fn handler_mathop(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = ord_argument(args[0].clone());
    Ok(ParseNode::Op {
        mode: ctx.parser.mode,
        loc: None,
        limits: false,
        always_handle_supsub: false,
        suppress_base_shift: false,
        parent_is_supsub: false,
        body: OpBody::Composite(body),
    })
}

fn handler_named_no_limits(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::Op {
        mode: ctx.parser.mode,
        loc: None,
        limits: false,
        always_handle_supsub: false,
        suppress_base_shift: false,
        parent_is_supsub: false,
        body: OpBody::Symbol(ctx.func_name.clone()),
    })
}

fn handler_named_limits(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::Op {
        mode: ctx.parser.mode,
        loc: None,
        limits: true,
        always_handle_supsub: false,
        suppress_base_shift: false,
        parent_is_supsub: false,
        body: OpBody::Symbol(ctx.func_name.clone()),
    })
}

fn handler_integrals(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let mut name = ctx.func_name.clone();
    if name.chars().count() == 1
        && let Some(canon) = single_char_integral(name.chars().next().unwrap())
    {
        name = SmolStr::new(canon);
    }
    Ok(ParseNode::Op {
        mode: ctx.parser.mode,
        loc: None,
        limits: false,
        always_handle_supsub: false,
        suppress_base_shift: false,
        parent_is_supsub: false,
        body: OpBody::Symbol(name),
    })
}

const BIG_OP_NAMES: &[&str] = &[
    "\\coprod",
    "\\bigvee",
    "\\bigwedge",
    "\\biguplus",
    "\\bigcap",
    "\\bigcup",
    "\\intop",
    "\\prod",
    "\\sum",
    "\\bigotimes",
    "\\bigoplus",
    "\\bigodot",
    "\\bigsqcup",
    "\\smallint",
    "\u{220F}",
    "\u{2210}",
    "\u{2211}",
    "\u{22c0}",
    "\u{22c1}",
    "\u{22c2}",
    "\u{22c3}",
    "\u{2a00}",
    "\u{2a01}",
    "\u{2a02}",
    "\u{2a04}",
    "\u{2a06}",
];
const MATHOP_NAMES: &[&str] = &["\\mathop"];
const NO_LIMITS_NAMES: &[&str] = &[
    "\\arcsin", "\\arccos", "\\arctan", "\\arctg", "\\arcctg", "\\arg", "\\ch", "\\cos", "\\cosec",
    "\\cosh", "\\cot", "\\cotg", "\\coth", "\\csc", "\\ctg", "\\cth", "\\deg", "\\dim", "\\exp",
    "\\hom", "\\ker", "\\lg", "\\ln", "\\log", "\\sec", "\\sin", "\\sinh", "\\sh", "\\tan",
    "\\tanh", "\\tg", "\\th",
];
const LIMITS_NAMES: &[&str] = &[
    "\\det", "\\gcd", "\\inf", "\\lim", "\\max", "\\min", "\\Pr", "\\sup",
];
const INTEGRAL_NAMES: &[&str] = &[
    "\\int", "\\iint", "\\iiint", "\\oint", "\\oiint", "\\oiiint", "\u{222b}", "\u{222c}",
    "\u{222d}", "\u{222e}", "\u{222f}", "\u{2230}",
];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Op,
        names: BIG_OP_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_big_ops),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Op,
        names: MATHOP_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: true,
        handler: Some(handler_mathop),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Op,
        names: NO_LIMITS_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_named_no_limits),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Op,
        names: LIMITS_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_named_limits),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Op,
        names: INTEGRAL_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: true,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_integrals),
        mathml_builder: None,
        html_builder: None,
    },
];

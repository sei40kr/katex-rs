//! `\mathord` / `\mathbin` / …, `\@binrel`, and `\stackrel` / `\overset`
//! / `\underset`. Mirrors upstream `functions/mclass.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::functions::utils::{binrel_class, is_character_box};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, OpBody, ParseNode};

fn handler_class(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = args[0].clone();
    // funcName slice(5) = "ord", "bin", … → prefix with "m".
    let mclass = format!("m{}", &ctx.func_name.as_str()[5..]);
    let is_char_box = is_character_box(&body);
    Ok(ParseNode::MClass {
        mode: ctx.parser.mode,
        loc: None,
        mclass: SmolStr::new(mclass),
        body: ord_argument(body),
        is_character_box: is_char_box,
    })
}

fn handler_binrel(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let mclass = SmolStr::new(binrel_class(&args[0]));
    let body_arg = args[1].clone();
    let is_char_box = is_character_box(&body_arg);
    Ok(ParseNode::MClass {
        mode: ctx.parser.mode,
        loc: None,
        mclass,
        body: ord_argument(body_arg),
        is_character_box: is_char_box,
    })
}

fn handler_stack(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let base_arg = args[1].clone();
    let shifted_arg = args[0].clone();

    let mclass = if ctx.func_name.as_str() != "\\stackrel" {
        binrel_class(&base_arg)
    } else {
        "mrel"
    };

    let base_op = ParseNode::Op {
        mode: base_arg.mode(),
        loc: None,
        limits: true,
        always_handle_supsub: true,
        suppress_base_shift: ctx.func_name.as_str() != "\\stackrel",
        parent_is_supsub: false,
        body: OpBody::Composite(ord_argument(base_arg)),
    };

    let supsub = if ctx.func_name.as_str() == "\\underset" {
        ParseNode::SupSub {
            mode: shifted_arg.mode(),
            loc: None,
            base: Some(Box::new(base_op)),
            sup: None,
            sub: Some(Box::new(shifted_arg.clone())),
        }
    } else {
        ParseNode::SupSub {
            mode: shifted_arg.mode(),
            loc: None,
            base: Some(Box::new(base_op)),
            sup: Some(Box::new(shifted_arg.clone())),
            sub: None,
        }
    };

    let is_char_box = is_character_box(&supsub);
    Ok(ParseNode::MClass {
        mode: ctx.parser.mode,
        loc: None,
        mclass: SmolStr::new(mclass),
        body: vec![supsub],
        is_character_box: is_char_box,
    })
}

const CLASS_NAMES: &[&str] = &[
    "\\mathord",
    "\\mathbin",
    "\\mathrel",
    "\\mathopen",
    "\\mathclose",
    "\\mathpunct",
    "\\mathinner",
];
const BINREL_NAMES: &[&str] = &["\\@binrel"];
const STACK_NAMES: &[&str] = &["\\stackrel", "\\overset", "\\underset"];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::MClass,
        names: CLASS_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: true,
        handler: Some(handler_class),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::MClass,
        names: BINREL_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_binrel),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::MClass,
        names: STACK_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_stack),
        mathml_builder: None,
    },
];

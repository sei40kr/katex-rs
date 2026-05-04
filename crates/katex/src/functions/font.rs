//! `\mathrm`, `\mathbb`, …, `\boldsymbol`, `\bm`, plus the old-style
//! `\rm` / `\sf` / … directives. Mirrors upstream `functions/font.ts`.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec, normalize_argument};
use crate::functions::utils::{binrel_class, is_character_box};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn alias(func_name: &str) -> &str {
    match func_name {
        "\\Bbb" => "\\mathbb",
        "\\bold" => "\\mathbf",
        "\\frak" => "\\mathfrak",
        "\\bm" => "\\boldsymbol",
        other => other,
    }
}

fn handler_font(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = normalize_argument(args[0].clone());
    let func = alias(ctx.func_name.as_str());
    Ok(ParseNode::Font {
        mode: ctx.parser.mode,
        loc: None,
        font: SmolStr::new(&func[1..]),
        body: Box::new(body),
    })
}

fn handler_boldsymbol(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = args[0].clone();
    let mclass = binrel_class(&body);
    let is_char_box = is_character_box(&body);
    let inner_font = ParseNode::Font {
        mode: ctx.parser.mode,
        loc: None,
        font: SmolStr::new_static("boldsymbol"),
        body: Box::new(body),
    };
    Ok(ParseNode::MClass {
        mode: ctx.parser.mode,
        loc: None,
        mclass: SmolStr::new(mclass),
        body: vec![inner_font],
        is_character_box: is_char_box,
    })
}

fn handler_old_font(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let parser = ctx.parser;
    let mode = parser.mode;
    let break_on = ctx.break_on_token_text;
    let body = parser.parse_expression(true, break_on)?;
    // Upstream: `style = "math" + funcName.slice(1)` → e.g. `mathrm`.
    let style = format!("math{}", &ctx.func_name.as_str()[1..]);
    let inner_group = ParseNode::OrdGroup {
        mode: parser.mode,
        loc: None,
        body,
        semisimple: false,
    };
    Ok(ParseNode::Font {
        mode,
        loc: None,
        font: SmolStr::new(style),
        body: Box::new(inner_group),
    })
}

const FONT_NAMES: &[&str] = &[
    "\\mathrm",
    "\\mathit",
    "\\mathbf",
    "\\mathnormal",
    "\\mathsfit",
    "\\mathbb",
    "\\mathcal",
    "\\mathfrak",
    "\\mathscr",
    "\\mathsf",
    "\\mathtt",
    "\\Bbb",
    "\\bold",
    "\\frak",
];
const BOLDSYMBOL_NAMES: &[&str] = &["\\boldsymbol", "\\bm"];
const OLD_FONT_NAMES: &[&str] = &["\\rm", "\\sf", "\\tt", "\\bf", "\\it", "\\cal"];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Font,
        names: FONT_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: true,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_font),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::MClass,
        names: BOLDSYMBOL_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_boldsymbol),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Font,
        names: OLD_FONT_NAMES,
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_old_font),
        mathml_builder: None,
        html_builder: None,
    },
];

//! Combines upstream `functions/html.ts` (`\htmlClass`, `\htmlId`,
//! `\htmlStyle`, `\htmlData`) and `functions/htmlmathml.ts`
//! (`\html@mathml`).

use std::collections::HashMap;

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;

fn handler_html(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let value = match &args[0] {
        ParseNode::Raw { string, .. } => string.clone(),
        _ => return Err(ParseError::new("html command expected raw argument")),
    };
    let body = args[1].clone();

    // TODO(strict): reportNonstrict("htmlExtension", ...).
    let mut attributes: HashMap<SmolStr, String> = HashMap::new();
    let func_name = ctx.func_name.as_str();
    match func_name {
        "\\htmlClass" => {
            attributes.insert(SmolStr::new_static("class"), value);
        }
        "\\htmlId" => {
            attributes.insert(SmolStr::new_static("id"), value);
        }
        "\\htmlStyle" => {
            attributes.insert(SmolStr::new_static("style"), value);
        }
        "\\htmlData" => {
            for item in value.split(',') {
                let eq = item.find('=').ok_or_else(|| {
                    ParseError::new(format!(
                        "\\htmlData key/value '{}' missing equals sign",
                        item
                    ))
                })?;
                let key = item[..eq].trim();
                let val = item[eq + 1..].to_string();
                attributes.insert(SmolStr::new(format!("data-{key}")), val);
            }
        }
        _ => return Err(ParseError::new("Unrecognized html command")),
    }

    if !ctx.parser.settings.trust {
        return Ok(ctx.parser.format_unsupported_cmd(func_name));
    }
    Ok(ParseNode::Html {
        mode: ctx.parser.mode,
        loc: None,
        attributes,
        body: ord_argument(body),
    })
}

fn handler_htmlmathml(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    Ok(ParseNode::HtmlMathml {
        mode: ctx.parser.mode,
        loc: None,
        html: ord_argument(args[0].clone()),
        mathml: ord_argument(args[1].clone()),
    })
}

const HTML_NAMES: &[&str] = &["\\htmlClass", "\\htmlId", "\\htmlStyle", "\\htmlData"];
const HTMLMATHML_NAMES: &[&str] = &["\\html@mathml"];

const HTML_ARGS: &[ArgType] = &[ArgType::Raw, ArgType::Original];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Html,
        names: HTML_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: HTML_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_html),
        mathml_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::HtmlMathml,
        names: HTMLMATHML_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: true,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_htmlmathml),
        mathml_builder: None,
    },
];

//! `\href` and `\url`. Mirrors upstream `functions/href.ts`.
//!
//! Upstream guards both via `parser.settings.isTrusted(...)`. Phase 5
//! follows the data variant of `Settings.trust`: `true` means trust all,
//! `false` means trust none. The function-callback variant lands when the
//! parser exposes a richer settings surface.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec, ord_argument};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::types::Mode;

fn handler_href(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let body = args[1].clone();
    let href = match &args[0] {
        ParseNode::Url { url, .. } => url.clone(),
        _ => return Err(ParseError::new("\\href expected a URL argument")),
    };
    if !ctx.parser.settings.trust {
        return Ok(ctx.parser.format_unsupported_cmd("\\href"));
    }
    Ok(ParseNode::Href {
        mode: ctx.parser.mode,
        loc: None,
        href,
        body: ord_argument(body),
    })
}

fn handler_url(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let href = match &args[0] {
        ParseNode::Url { url, .. } => url.clone(),
        _ => return Err(ParseError::new("\\url expected a URL argument")),
    };
    if !ctx.parser.settings.trust {
        return Ok(ctx.parser.format_unsupported_cmd("\\url"));
    }
    let chars: Vec<ParseNode> = href
        .chars()
        .map(|c| {
            let text = if c == '~' {
                SmolStr::new_static("\\textasciitilde")
            } else {
                SmolStr::new(c.to_string())
            };
            ParseNode::TextOrd {
                mode: Mode::Text,
                loc: None,
                text,
            }
        })
        .collect();
    let body = ParseNode::Text {
        mode: ctx.parser.mode,
        loc: None,
        body: chars,
        font: Some(SmolStr::new_static("\\texttt")),
    };
    Ok(ParseNode::Href {
        mode: ctx.parser.mode,
        loc: None,
        href,
        body: ord_argument(body),
    })
}

const HREF_NAMES: &[&str] = &["\\href"];
const URL_NAMES: &[&str] = &["\\url"];

const HREF_ARGS: &[ArgType] = &[ArgType::Url, ArgType::Original];
const URL_ARGS: &[ArgType] = &[ArgType::Url];

pub const SPECS: &[FunctionSpec] = &[
    FunctionSpec {
        node_type: NodeType::Href,
        names: HREF_NAMES,
        num_args: 2,
        num_optional_args: 0,
        arg_types: HREF_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_href),
        mathml_builder: None,
        html_builder: None,
    },
    FunctionSpec {
        node_type: NodeType::Href,
        names: URL_NAMES,
        num_args: 1,
        num_optional_args: 0,
        arg_types: URL_ARGS,
        allowed_in_argument: false,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(handler_url),
        mathml_builder: None,
        html_builder: None,
    },
];

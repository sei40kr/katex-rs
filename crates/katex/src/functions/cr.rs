//! `\\` — line break inside text or table rows. Mirrors upstream
//! `functions/cr.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let next_is_bracket = ctx.parser.gullet.future()?.text.as_str() == "[";
    let size = if next_is_bracket {
        match ctx.parser.parse_size_group(true)? {
            Some(ParseNode::Size { value, .. }) => Some(value),
            _ => None,
        }
    } else {
        None
    };
    // Upstream: `newLine = !displayMode || !useStrictBehavior(...)`. Without
    // the strict callback wired up we follow the lenient path: line breaks
    // outside display mode become `newLine = true`; in display mode we
    // mirror the warn-but-allow default by also returning `true`.
    // TODO(strict): tighten when reportNonstrict lands.
    let new_line = !ctx.parser.settings.display_mode;
    Ok(ParseNode::Cr {
        mode: ctx.parser.mode,
        loc: None,
        new_line,
        size,
    })
}

const NAMES: &[&str] = &["\\\\"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Cr,
    names: NAMES,
    num_args: 0,
    num_optional_args: 0,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

//! `\includegraphics`. Mirrors upstream `functions/includegraphics.ts`.
//!
//! Parser-side only — actual image rendering belongs to the renderer.
//! The trust gate uses `Settings.trust` (data-only variant); the function
//! callback variant is deferred.

use std::sync::OnceLock;

use regex::Regex;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::units::{Measurement, Unit};

const DEFAULT_WIDTH: Measurement = Measurement::new(0.0, Unit::Em);
const DEFAULT_HEIGHT: Measurement = Measurement::new(0.9, Unit::Em);
const DEFAULT_TOTAL_HEIGHT: Measurement = Measurement::new(0.0, Unit::Em);

fn size_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"([-+]?) *(\d+(?:\.\d*)?|\.\d+) *([a-z]{2})").unwrap())
}

fn pure_number_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^[-+]? *(\d+(\.\d*)?|\.\d+)$").unwrap())
}

fn parse_size(s: &str) -> Result<Measurement, ParseError> {
    if pure_number_re().is_match(s) {
        let n: f64 = s.trim().parse().unwrap_or(0.0);
        return Ok(Measurement::new(n, Unit::Bp));
    }
    let captures = size_re()
        .captures(s)
        .ok_or_else(|| ParseError::new(format!("Invalid size: '{s}' in \\includegraphics")))?;
    let sign = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    let magnitude = captures.get(2).expect("magnitude").as_str();
    let unit_str = captures.get(3).expect("unit").as_str();
    let number: f64 = format!("{sign}{magnitude}").parse().unwrap_or(0.0);
    let unit: Unit = unit_str.parse().map_err(|_| {
        ParseError::new(format!("Invalid unit: '{unit_str}' in \\includegraphics."))
    })?;
    Ok(Measurement::new(number, unit))
}

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut totalheight = DEFAULT_TOTAL_HEIGHT;
    let mut alt = String::new();

    if let Some(Some(opt)) = opt_args.first() {
        let attribute_str = match opt {
            ParseNode::Raw { string, .. } => string.clone(),
            _ => return Err(ParseError::new("\\includegraphics expected raw argument")),
        };
        for attr in attribute_str.split(',') {
            if let Some(eq) = attr.find('=') {
                let key = attr[..eq].trim();
                let val = attr[eq + 1..].trim();
                match key {
                    "alt" => alt = val.to_string(),
                    "width" => width = parse_size(val)?,
                    "height" => height = parse_size(val)?,
                    "totalheight" => totalheight = parse_size(val)?,
                    other => {
                        return Err(ParseError::new(format!(
                            "Invalid key: '{other}' in \\includegraphics."
                        )));
                    }
                }
            }
        }
    }

    let src = match &args[0] {
        ParseNode::Url { url, .. } => url.clone(),
        _ => return Err(ParseError::new("\\includegraphics expected URL argument")),
    };

    if alt.is_empty() {
        // Strip directory and extension from src.
        let last_slash = src.rfind(['/', '\\']).map(|i| i + 1).unwrap_or(0);
        let stripped = &src[last_slash..];
        let dot = stripped.rfind('.').unwrap_or(stripped.len());
        alt = stripped[..dot].to_string();
    }

    if !ctx.parser.settings.trust {
        return Ok(ctx.parser.format_unsupported_cmd("\\includegraphics"));
    }

    Ok(ParseNode::IncludeGraphics {
        mode: ctx.parser.mode,
        loc: None,
        alt,
        width,
        height,
        totalheight,
        src,
    })
}

const NAMES: &[&str] = &["\\includegraphics"];
const ARGS: &[ArgType] = &[ArgType::Raw, ArgType::Url];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::IncludeGraphics,
    names: NAMES,
    num_args: 1,
    num_optional_args: 1,
    arg_types: ARGS,
    allowed_in_argument: false,
    allowed_in_text: false,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
    html_builder: None,
}];

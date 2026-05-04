//! `\begin` / `\end`. Mirrors upstream `functions/environment.ts`.
//!
//! Phase 5 ports the parser surface that *would* dispatch to environment
//! handlers, but the `\begin` path is gated on a populated environment
//! registry. The full registry (matrix, align, cases, …) lands in Phase 8.
//! Until then `\begin` reports "No such environment", which is the same
//! error upstream raises for an unknown name — so input that uses
//! environments fails clearly rather than silently mis-parsing.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::types::Mode;

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let name_group = args[0].clone();
    let body = match &name_group {
        ParseNode::OrdGroup { body, .. } => body.clone(),
        _ => return Err(ParseError::new("Invalid environment name")),
    };
    let mut env_name = String::new();
    for n in &body {
        match n {
            ParseNode::TextOrd { text, .. } => env_name.push_str(text),
            _ => return Err(ParseError::new("Invalid environment name")),
        }
    }
    if ctx.func_name.as_str() == "\\begin" {
        // Phase 8 will look env_name up in the environments registry,
        // dispatch, and chase the matching \end. For now, surface the
        // upstream-shaped error.
        return Err(ParseError::new(format!("No such environment: {env_name}")));
    }
    Ok(ParseNode::Environment {
        mode: ctx.parser.mode,
        loc: None,
        name: SmolStr::new(env_name),
        name_group: Box::new(name_group),
    })
}

const NAMES: &[&str] = &["\\begin", "\\end"];
const ARGS: &[ArgType] = &[ArgType::Mode(Mode::Text)];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Environment,
    names: NAMES,
    num_args: 1,
    num_optional_args: 0,
    arg_types: ARGS,
    allowed_in_argument: false,
    allowed_in_text: false,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
}];

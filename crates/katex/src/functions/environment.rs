//! `\begin` / `\end`. Mirrors upstream `functions/environment.ts`.
//!
//! `\begin{name}` looks `name` up in [`crate::environments::ENVIRONMENTS`];
//! if found, the environment's handler runs (consuming arguments and the
//! environment body) and the parser confirms the trailing `\end{name}`.
//! `\end` produces a sentinel `Environment` node so the matching call in
//! the parent environment's body parser can recognise the terminator.

use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::environments::ENVIRONMENTS;
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::tree::ArgType;
use crate::types::Mode;

fn extract_env_name(group: &ParseNode) -> Result<String, ParseError> {
    let body: Vec<&ParseNode> = match group {
        ParseNode::OrdGroup { body, .. } => body.iter().collect(),
        _ => return Err(ParseError::new("Invalid environment name")),
    };
    let mut name = String::new();
    for n in body {
        match n {
            ParseNode::TextOrd { text, .. } => name.push_str(text),
            _ => return Err(ParseError::new("Invalid environment name")),
        }
    }
    Ok(name)
}

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let name_group = args[0].clone();
    let env_name = extract_env_name(&name_group)?;

    if ctx.func_name.as_str() == "\\end" {
        return Ok(ParseNode::Environment {
            mode: ctx.parser.mode,
            loc: None,
            name: SmolStr::new(env_name),
            name_group: Box::new(name_group),
        });
    }

    let env = ENVIRONMENTS
        .get(env_name.as_str())
        .ok_or_else(|| ParseError::new(format!("No such environment: {env_name}")))?;

    // Parse the environment's mandatory + optional arguments.
    let total = env.num_args + env.num_optional_args;
    let mut args: Vec<ParseNode> = Vec::with_capacity(env.num_args);
    let mut opt_args: Vec<Option<ParseNode>> = Vec::with_capacity(env.num_optional_args);
    for i in 0..total {
        let arg_type = env.arg_types.get(i).copied();
        let is_optional = i < env.num_optional_args;
        let parsed =
            ctx.parser
                .parse_arg_for_environment(arg_type, is_optional, env_name.as_str())?;
        if is_optional {
            opt_args.push(parsed);
        } else if let Some(node) = parsed {
            args.push(node);
        } else {
            return Err(ParseError::new(format!(
                "Missing argument for environment {env_name}"
            )));
        }
    }

    let env_ctx = crate::define_environment::EnvContext {
        mode: ctx.parser.mode,
        env_name: env_name.as_str(),
        parser: ctx.parser,
    };
    let result = (env.handler)(env_ctx, &args, &opt_args)?;

    // Match the trailing \end{name}.
    let end_token = ctx.parser.fetch()?.text.clone();
    if end_token.as_str() != "\\end" {
        return Err(ParseError::new(format!(
            "Expected \\end at end of environment {env_name}"
        )));
    }
    ctx.parser.consume();
    let end_name_group = ctx.parser.parse_environment_name_group()?;
    let end_name = extract_env_name(&end_name_group)?;
    if end_name != env_name {
        return Err(ParseError::new(format!(
            "Mismatch: \\begin{{{env_name}}} matched by \\end{{{end_name}}}"
        )));
    }

    Ok(result)
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

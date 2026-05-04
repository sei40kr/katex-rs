//! `\smash[t|b|tb]{body}`. Mirrors upstream `functions/smash.ts`.

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};

fn handler(
    ctx: FunctionContext<'_, '_>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let mut smash_height = false;
    let mut smash_depth = false;
    let tb_arg = opt_args.first().and_then(|o| o.as_ref());
    if let Some(arg) = tb_arg {
        if let ParseNode::OrdGroup { body, .. } = arg {
            for node in body {
                let letter = match node {
                    ParseNode::TextOrd { text, .. }
                    | ParseNode::MathOrd { text, .. }
                    | ParseNode::Atom { text, .. }
                    | ParseNode::Spacing { text, .. }
                    | ParseNode::AccentToken { text, .. }
                    | ParseNode::OpToken { text, .. } => text.as_str(),
                    _ => {
                        smash_height = false;
                        smash_depth = false;
                        break;
                    }
                };
                match letter {
                    "t" => smash_height = true,
                    "b" => smash_depth = true,
                    _ => {
                        smash_height = false;
                        smash_depth = false;
                        break;
                    }
                }
            }
        }
    } else {
        smash_height = true;
        smash_depth = true;
    }
    Ok(ParseNode::Smash {
        mode: ctx.parser.mode,
        loc: None,
        body: Box::new(args[0].clone()),
        smash_height,
        smash_depth,
    })
}

const NAMES: &[&str] = &["\\smash"];

pub const SPECS: &[FunctionSpec] = &[FunctionSpec {
    node_type: NodeType::Smash,
    names: NAMES,
    num_args: 1,
    num_optional_args: 1,
    arg_types: &[],
    allowed_in_argument: false,
    allowed_in_text: true,
    allowed_in_math: true,
    infix: false,
    primitive: false,
    handler: Some(handler),
    mathml_builder: None,
}];

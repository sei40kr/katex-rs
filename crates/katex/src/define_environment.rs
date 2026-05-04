//! Environment-spec data type and registry shape. Mirrors upstream
//! KaTeX's `defineEnvironment.ts`.
//!
//! Same registration model as [`crate::define_function`]: instead of
//! upstream's mutable `_environments` dict populated by side-effecting
//! calls, environments live in a `Lazy` `HashMap` keyed by name and
//! built from an explicit slice. See the deviation note in
//! [`crate::define_function`] for the rationale.

use crate::define_function::{HtmlBuilder, MathmlBuilder};
use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::parser::Parser;
use crate::tree::ArgType;
use crate::types::Mode;

/// Per-call context handed to an [`EnvHandler`].
pub struct EnvContext<'a, 's> {
    pub mode: Mode,
    pub env_name: &'a str,
    pub parser: &'a mut Parser<'s>,
}

/// Environment handler. Builds and returns the `ParseNode` for one
/// `\begin{name} ... \end{name}` invocation. Mirrors upstream's
/// `EnvHandler`.
pub type EnvHandler = for<'a, 's> fn(
    ctx: EnvContext<'a, 's>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError>;

/// One row in the environment dispatch table. Mirrors upstream's
/// `EnvSpec<NODETYPE>`. Note that upstream's runtime allows an
/// environment with no MathML/HTML builders if the parser-side handler
/// rewrites itself into another node type — both slots are therefore
/// optional here.
///
/// `Copy` for the same reason as [`FunctionSpec`]: every field is a fn
/// pointer, primitive, enum, or `&'static [...]` slice.
#[derive(Copy, Clone)]
pub struct EnvSpec {
    /// Discriminant of the [`ParseNode`] variant the handler emits.
    /// Always set, mirroring upstream — environments that delegate to
    /// another node type still register with that type so renderer
    /// dispatch is uniform.
    pub node_type: NodeType,

    /// All environment names sharing this spec.
    pub names: &'static [&'static str],

    /// Number of mandatory arguments after `\begin{name}`.
    pub num_args: usize,

    /// Per-argument parsing mode. When non-empty, the slice length is
    /// `num_optional_args + num_args`.
    pub arg_types: &'static [ArgType],

    /// Whether the environment may appear inside text mode. Defaults to
    /// `false` upstream.
    pub allowed_in_text: bool,

    /// Number of optional `[...]` arguments. Defaults to `0` upstream.
    pub num_optional_args: usize,

    /// Required handler — environments always need parser-side logic.
    pub handler: EnvHandler,

    /// MathML builder. Optional only because some environments share a
    /// node type with another spec that owns the builder.
    pub mathml_builder: Option<MathmlBuilder>,

    /// HTML+CSS builder. Optional always.
    pub html_builder: Option<HtmlBuilder>,
}

impl EnvSpec {
    pub const fn new(
        node_type: NodeType,
        names: &'static [&'static str],
        num_args: usize,
        handler: EnvHandler,
    ) -> Self {
        Self {
            node_type,
            names,
            num_args,
            arg_types: &[],
            allowed_in_text: false,
            num_optional_args: 0,
            handler,
            mathml_builder: None,
            html_builder: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_handler(
        _ctx: EnvContext<'_, '_>,
        _args: &[ParseNode],
        _opt_args: &[Option<ParseNode>],
    ) -> Result<ParseNode, ParseError> {
        Ok(ParseNode::Internal {
            mode: Mode::Math,
            loc: None,
        })
    }

    #[test]
    fn defaults_match_upstream_spec() {
        let s = EnvSpec::new(NodeType::Internal, &["matrix"], 0, dummy_handler);
        assert_eq!(s.num_args, 0);
        assert_eq!(s.num_optional_args, 0);
        assert!(!s.allowed_in_text);
        assert!(s.arg_types.is_empty());
        assert!(s.mathml_builder.is_none());
    }
}

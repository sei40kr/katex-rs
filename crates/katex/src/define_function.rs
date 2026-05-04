//! Function-spec data type and shared handler signature for the
//! function-dispatch registry. Mirrors upstream KaTeX's
//! `defineFunction.ts`.
//!
//! # Deviations from upstream
//!
//! - Upstream's `_functions` / `_htmlGroupBuilders` / `_mathmlGroupBuilders`
//!   are mutable module-level dictionaries populated by side-effecting
//!   `defineFunction(...)` calls in dozens of files. We reject that
//!   pattern (registration order is fragile and `wasm32` has no portable
//!   `inventory`/`linkme` story) and instead build a single `Lazy`
//!   `HashMap` from an explicit slice of [`FunctionSpec`] values in
//!   [`crate::functions`]. Registration order is now an obvious local
//!   data-flow choice rather than a load-order accident.
//! - The mathml/html builder slots live on the spec itself rather than in
//!   parallel side-tables. There is exactly one place to look for
//!   "everything about a function".
//! - Upstream's `greediness` field is gone in modern KaTeX (the parser
//!   no longer needs it). We follow current upstream and omit it; if a
//!   future upstream port reintroduces it, add the field here and
//!   thread it through the parser.
//! - The handler takes `&mut dyn ParserApi` instead of borrowing a
//!   concrete `Parser`. Phase 4 introduces the real `Parser` type and
//!   adds methods to [`ParserApi`]; Phase 3 keeps the trait empty so
//!   the registry can be populated and round-tripped without depending
//!   on a parser yet. The MathML/HTML builders are similarly shaped
//!   around opaque `dyn` traits — the real signatures land alongside
//!   the renderers in Phases 6 and 10.

use smol_str::SmolStr;

use crate::parse_error::ParseError;
use crate::parse_node::{NodeType, ParseNode};
use crate::token::Token;
use crate::tree::{ArgType, BreakToken};

/// Stand-in for the parser surface used by function handlers. Phase 4
/// fills in the methods (`fetch`, `consume`, `parse_group`, …); Phase 3
/// only needs the trait to exist so handlers can name `&mut dyn ParserApi`
/// in their signature without depending on a concrete parser type.
pub trait ParserApi {}

/// Stand-in for the rendering options threaded through builders. Phase 6
/// (MathML) and Phase 10 (HTML+CSS) introduce the real fields; the
/// trait lives here so the builder fn-pointer types in [`FunctionSpec`]
/// have a stable signature to be filled in later.
pub trait BuilderOptions {}

/// Phase-6 placeholder: the abstract MathML node returned by a MathML
/// builder. The concrete type (`mathml_tree::MathNode`) arrives with the
/// renderer.
pub trait MathDomNode {}

/// Phase-10 placeholder: the abstract HTML node returned by an HTML
/// builder. The concrete type (`dom_tree::HtmlDomNode`) arrives with
/// the HTML renderer.
pub trait HtmlDomNode {}

/// Per-call context handed to a [`FunctionHandler`].
pub struct FunctionContext<'a> {
    pub func_name: SmolStr,
    pub parser: &'a mut dyn ParserApi,
    pub token: Option<Token>,
    pub break_on_token_text: Option<BreakToken>,
}

/// Parser-side handler. Builds and returns the `ParseNode` for one
/// invocation of the function. Mirrors upstream's `FunctionHandler`.
///
/// Optional arguments come in as `Option<ParseNode>` (upstream:
/// `(AnyParseNode | null | undefined)[]`). Mandatory arguments are
/// guaranteed present by the parser — the slice length always matches
/// `FunctionSpec::num_args`.
pub type FunctionHandler = fn(
    ctx: FunctionContext<'_>,
    args: &[ParseNode],
    opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError>;

/// Phase-6+ MathML builder. The handler receives the parse node and the
/// rendering options; it returns a MathML node to be inserted into the
/// output tree.
pub type MathmlBuilder =
    fn(group: &ParseNode, options: &dyn BuilderOptions) -> Box<dyn MathDomNode>;

/// Phase-10 HTML builder. Optional on most specs because MathML is the
/// first milestone.
pub type HtmlBuilder = fn(group: &ParseNode, options: &dyn BuilderOptions) -> Box<dyn HtmlDomNode>;

/// One row in the function dispatch table. Mirrors upstream's
/// `FunctionSpec<NODETYPE>` (the per-NodeType generic is collapsed:
/// every spec returns the same enum, and the `node_type` field captures
/// what variant the handler emits).
///
/// `Copy` because every field is a fn pointer, primitive, enum, or
/// `&'static [...]` slice — handy for building static spec slices and
/// for the registry that holds `&'static FunctionSpec` references.
#[derive(Copy, Clone)]
pub struct FunctionSpec {
    /// Discriminant of the [`ParseNode`] variant this handler produces.
    /// Always set: upstream's `defineFunctionBuilders` shape (which
    /// registers a builder without a parser-side handler) still
    /// requires `type` so renderer dispatch works.
    pub node_type: NodeType,

    /// All function names that share this spec. Mirrors upstream's
    /// `defineFunction({ names: [...], ... })`.
    pub names: &'static [&'static str],

    /// Number of mandatory arguments.
    pub num_args: usize,

    /// Number of optional `[...]` arguments. They are passed before the
    /// mandatory args in `arg_types` (matching upstream).
    pub num_optional_args: usize,

    /// Per-argument parsing mode. When non-empty, the slice length is
    /// `num_optional_args + num_args`.
    pub arg_types: &'static [ArgType],

    /// Whether the function may appear as the body of a primitive
    /// argument (e.g. `\sqrt{...}` or sup/subscripts). Defaults to
    /// `false` upstream.
    pub allowed_in_argument: bool,

    /// Whether the function is permitted in text mode (default `false`).
    pub allowed_in_text: bool,

    /// Whether the function is permitted in math mode (default `true`).
    pub allowed_in_math: bool,

    /// Whether the function is an infix operator (`\over`, `\atop`, …).
    pub infix: bool,

    /// Whether the function is a TeX primitive.
    pub primitive: bool,

    /// Parser-side handler. `None` mirrors upstream's
    /// `defineFunctionBuilders` shape (renderer-only registration).
    pub handler: Option<FunctionHandler>,

    /// MathML builder. Optional now; required for any node that
    /// reaches the MathML output (Phase 6).
    pub mathml_builder: Option<MathmlBuilder>,

    /// HTML+CSS builder. Optional always — only filled in for nodes
    /// that participate in the HTML pipeline (Phase 10).
    pub html_builder: Option<HtmlBuilder>,
}

impl FunctionSpec {
    /// Builder-style constructor with sensible defaults matching
    /// upstream's `defineFunction` zero-value behaviour.
    pub const fn new(node_type: NodeType, names: &'static [&'static str], num_args: usize) -> Self {
        Self {
            node_type,
            names,
            num_args,
            num_optional_args: 0,
            arg_types: &[],
            allowed_in_argument: false,
            allowed_in_text: false,
            allowed_in_math: true,
            infix: false,
            primitive: false,
            handler: None,
            mathml_builder: None,
            html_builder: None,
        }
    }
}

/// Normalize an argument: a single-child `ordgroup` collapses to its
/// only child. Mirrors upstream's `normalizeArgument` helper. Useful in
/// handlers that want to look through an unnecessary `{...}` wrapper.
pub fn normalize_argument(arg: ParseNode) -> ParseNode {
    match arg {
        ParseNode::OrdGroup { mut body, .. } if body.len() == 1 => body.remove(0),
        other => other,
    }
}

/// Coerce an argument to the list-of-children form expected by the
/// MathML/HTML builders. Mirrors upstream's `ordargument` helper:
/// flattens an `ordgroup` body, otherwise wraps the single node in a
/// one-element vec.
pub fn ord_argument(arg: ParseNode) -> Vec<ParseNode> {
    match arg {
        ParseNode::OrdGroup { body, .. } => body,
        other => vec![other],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Mode;

    #[test]
    fn defaults_match_upstream_spec() {
        let s = FunctionSpec::new(crate::parse_node::NodeType::Internal, &["\\foo"], 1);
        assert_eq!(s.num_args, 1);
        assert_eq!(s.num_optional_args, 0);
        assert!(!s.allowed_in_text);
        assert!(s.allowed_in_math);
        assert!(!s.allowed_in_argument);
        assert!(!s.infix);
        assert!(!s.primitive);
        assert!(s.handler.is_none());
        assert!(s.mathml_builder.is_none());
        assert!(s.html_builder.is_none());
    }

    #[test]
    fn ord_argument_unwraps_ordgroup() {
        let body = vec![
            ParseNode::MathOrd {
                mode: Mode::Math,
                loc: None,
                text: SmolStr::new("a"),
            },
            ParseNode::MathOrd {
                mode: Mode::Math,
                loc: None,
                text: SmolStr::new("b"),
            },
        ];
        let group = ParseNode::OrdGroup {
            mode: Mode::Math,
            loc: None,
            body: body.clone(),
            semisimple: false,
        };
        assert_eq!(ord_argument(group), body);
    }

    #[test]
    fn ord_argument_wraps_singleton() {
        let n = ParseNode::MathOrd {
            mode: Mode::Math,
            loc: None,
            text: SmolStr::new("x"),
        };
        let v = ord_argument(n.clone());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], n);
    }

    #[test]
    fn normalize_argument_unwraps_singleton_ordgroup() {
        let inner = ParseNode::MathOrd {
            mode: Mode::Math,
            loc: None,
            text: SmolStr::new("z"),
        };
        let group = ParseNode::OrdGroup {
            mode: Mode::Math,
            loc: None,
            body: vec![inner.clone()],
            semisimple: false,
        };
        assert_eq!(normalize_argument(group), inner);
    }

    #[test]
    fn normalize_argument_leaves_multichild_ordgroup() {
        let group = ParseNode::OrdGroup {
            mode: Mode::Math,
            loc: None,
            body: vec![
                ParseNode::MathOrd {
                    mode: Mode::Math,
                    loc: None,
                    text: SmolStr::new("a"),
                },
                ParseNode::MathOrd {
                    mode: Mode::Math,
                    loc: None,
                    text: SmolStr::new("b"),
                },
            ],
            semisimple: false,
        };
        let result = normalize_argument(group.clone());
        assert_eq!(result, group);
    }
}

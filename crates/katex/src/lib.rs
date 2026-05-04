//! Rust port of KaTeX.
//!
//! Phase 0–3 surface: env-independent leaf primitives, the
//! lexer / token / namespace / macro-expander layer, the codegen-emitted
//! static data tables (`symbols`, `macros`, `spacing_data`, the unicode
//! helpers, and `font_metrics_data`), the [`parse_node::ParseNode`] AST
//! type, and the function/environment/macro dispatch shapes plus the
//! lazy [`functions::FUNCTIONS`] registry.
//! Parser, builder, and renderer land in subsequent phases.
//!
//! See `CLAUDE.md` at the repo root for vision and architectural rules.

#![forbid(unsafe_code)]

pub mod define_environment;
pub mod define_function;
pub mod define_macro;
pub mod font_metrics_data;
pub mod functions;
pub mod lexer;
pub mod macro_expander;
pub mod macros;
pub mod namespace;
pub mod parse_error;
pub mod parse_node;
pub mod settings;
pub mod source_location;
pub mod spacing_data;
pub mod style;
pub mod symbols;
pub mod token;
pub mod tree;
pub mod types;
pub mod unicode_accents;
pub mod unicode_scripts;
pub mod unicode_sup_or_sub;
pub mod unicode_symbols;
pub mod units;

pub use define_environment::{EnvContext, EnvHandler, EnvSpec};
pub use define_function::{
    BuilderOptions, FunctionContext, FunctionHandler, FunctionSpec, HtmlBuilder, HtmlDomNode,
    MathDomNode, MathmlBuilder, ParserApi, normalize_argument, ord_argument,
};
pub use define_macro::MacroSpec;
pub use font_metrics_data::{CharacterMetrics, FONT_METRICS_DATA};
pub use functions::{FUNCTIONS, FunctionRegistry};
pub use lexer::Lexer;
pub use macro_expander::{
    BuiltinFn, BuiltinResult, MacroArg, MacroDefinition, MacroExpander, MacroExpansion,
};
pub use macros::MACROS;
pub use namespace::Namespace;
pub use parse_error::ParseError;
pub use parse_node::{ArrayTag, HLineSpec, NodeType, OpBody, ParseNode};
pub use settings::{OutputFormat, Settings, SettingsBuilder, StrictMode};
pub use source_location::SourceLocation;
pub use spacing_data::{SPACINGS, SpacingTable, TIGHT_SPACINGS};
pub use style::Style;
pub use symbols::{Font, Group, SYMBOLS, SymbolInfo, SymbolTable};
pub use token::Token;
pub use tree::{AlignSpec, ArgType, Atom, BreakToken, ColSeparationType, StyleStr};
pub use types::{AtomClass, Mode};
pub use unicode_accents::{UNICODE_ACCENTS, UnicodeAccent};
pub use unicode_scripts::{SCRIPT_DATA, Script, script_from_codepoint, supported_codepoint};
pub use unicode_sup_or_sub::{U_SUBS_AND_SUPS, UNICODE_SUB_REGEX};
pub use unicode_symbols::UNICODE_SYMBOLS;
pub use units::{Measurement, Unit, make_em};

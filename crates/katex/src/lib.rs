//! Rust port of KaTeX.
//!
//! Phase 0 + Phase 1 surface: env-independent leaf primitives plus the
//! lexer / token / namespace / macro-expander layer. Parser, builder, and
//! renderer land in subsequent phases.
//!
//! See `CLAUDE.md` at the repo root for vision and architectural rules.

#![forbid(unsafe_code)]

pub mod lexer;
pub mod macro_expander;
pub mod namespace;
pub mod parse_error;
pub mod settings;
pub mod source_location;
pub mod style;
pub mod token;
pub mod types;
pub mod units;

pub use lexer::Lexer;
pub use macro_expander::{
    BuiltinFn, BuiltinResult, MacroArg, MacroDefinition, MacroExpander, MacroExpansion,
};
pub use namespace::Namespace;
pub use parse_error::ParseError;
pub use settings::{OutputFormat, Settings, SettingsBuilder, StrictMode};
pub use source_location::SourceLocation;
pub use style::Style;
pub use token::Token;
pub use types::Mode;
pub use units::{Measurement, Unit, make_em};

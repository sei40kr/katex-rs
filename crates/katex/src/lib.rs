//! Rust port of KaTeX.
//!
//! Currently exposes the env-independent leaf primitives shared across the
//! parser, builder, and renderer (`SourceLocation`, `ParseError`, `Settings`,
//! `Style`, `Unit`/`Measurement`). Lexing, parsing, building, and rendering
//! land in subsequent modules.
//!
//! See `CLAUDE.md` at the repo root for vision and architectural rules.

#![forbid(unsafe_code)]

pub mod parse_error;
pub mod settings;
pub mod source_location;
pub mod style;
pub mod units;

pub use parse_error::ParseError;
pub use settings::{OutputFormat, Settings, SettingsBuilder, StrictMode};
pub use source_location::SourceLocation;
pub use style::Style;
pub use units::{Measurement, Unit, make_em};

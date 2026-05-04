//! Shared scalar types referenced across the parser, builder, and renderer.
//!
//! Mirrors upstream KaTeX's `types.ts`. Currently only `Mode` is defined;
//! other shared types (e.g. `StyleStr`, `BreakToken`) will arrive as the
//! port reaches the relevant phases.

/// Whether tokens are being parsed as math or text. `MacroExpander` and
/// the parser switch between these modes mid-stream.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Math,
    Text,
}

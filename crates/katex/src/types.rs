//! Shared scalar types referenced across the parser, builder, and renderer.
//!
//! Mirrors upstream KaTeX's `types.ts`. Other shared types
//! (e.g. `StyleStr`, `BreakToken`) will arrive as the port reaches the
//! relevant phases.

/// Whether tokens are being parsed as math or text. `MacroExpander` and
/// the parser switch between these modes mid-stream.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Math,
    Text,
}

/// TeX atom classes — the eight categories that drive inter-atom
/// spacing (see TeXbook ch. 17). Mirrors upstream's atom-class strings
/// (`"mord"`, `"mop"`, …); the discriminant order here is **load-bearing**
/// — [`crate::spacing_data::SpacingTable`] indexes by `as usize` and the
/// build script emits its rows in this exact order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AtomClass {
    MOrd = 0,
    MOp = 1,
    MBin = 2,
    MRel = 3,
    MOpen = 4,
    MClose = 5,
    MPunct = 6,
    MInner = 7,
}

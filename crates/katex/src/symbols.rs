//! Symbol table — `(mode, name) → SymbolInfo`. Mirrors upstream's
//! `symbols.ts`.
//!
//! **Deviation from the issue's API sketch.** Issue #3 calls for
//! `phf::Map<(Mode, &str), SymbolInfo>`. We instead hold one
//! `phf::Map<&'static str, SymbolInfo>` per mode and dispatch in
//! [`SymbolTable::get`]. Tuple keys would force either a hand-rolled
//! `phf_shared::PhfHash` impl that build.rs and the runtime crate
//! agree on bit-for-bit, or the slow `phf_macros` proc-macro at
//! compile time. The two-map split is simpler and the caller-side
//! API is identical.

use crate::types::Mode;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Font {
    Main,
    Ams,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Group {
    // ATOMS
    Bin,
    Close,
    Inner,
    Open,
    Punct,
    Rel,
    // NON_ATOMS
    AccentToken,
    MathOrd,
    OpToken,
    Spacing,
    TextOrd,
}

impl Group {
    pub const fn is_atom(self) -> bool {
        matches!(
            self,
            Group::Bin | Group::Close | Group::Inner | Group::Open | Group::Punct | Group::Rel
        )
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SymbolInfo {
    pub font: Font,
    pub group: Group,
    pub replace: Option<&'static str>,
}

pub struct SymbolTable {
    pub math: phf::Map<&'static str, SymbolInfo>,
    pub text: phf::Map<&'static str, SymbolInfo>,
}

impl SymbolTable {
    pub fn get(&self, key: (Mode, &str)) -> Option<&SymbolInfo> {
        let (mode, name) = key;
        match mode {
            Mode::Math => self.math.get(name),
            Mode::Text => self.text.get(name),
        }
    }
}

pub static SYMBOLS: SymbolTable = include!(concat!(env!("OUT_DIR"), "/symbols.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_is_a_math_mathord() {
        let info = SYMBOLS.get((Mode::Math, "\\alpha")).expect("\\alpha");
        assert_eq!(info.font, Font::Main);
        assert_eq!(info.group, Group::MathOrd);
        assert_eq!(info.replace, Some("\u{03b1}"));
    }

    #[test]
    fn alpha_text_mode_is_absent() {
        assert!(SYMBOLS.get((Mode::Text, "\\alpha")).is_none());
    }

    #[test]
    fn equiv_lookup() {
        let info = SYMBOLS.get((Mode::Math, "\\equiv")).unwrap();
        assert_eq!(info.group, Group::Rel);
        assert_eq!(info.replace, Some("\u{2261}"));
    }

    #[test]
    fn group_is_atom_classification() {
        assert!(Group::Rel.is_atom());
        assert!(Group::Bin.is_atom());
        assert!(!Group::MathOrd.is_atom());
        assert!(!Group::Spacing.is_atom());
    }
}

//! Small shared types referenced across `parse_node`, `define_function`,
//! `define_environment`, and (eventually) the array environment.
//!
//! **Naming note.** Upstream's `tree.ts` is the abstract DOM-fragment type
//! (`DocumentFragment`, `VirtualNode`). That belongs to the rendering
//! layer and lands in Phase 6 as `mathml_tree.rs` / `dom_tree.rs`. This
//! file is the home for the small parser-side helpers the issue asked
//! for; it intentionally does **not** mirror the upstream `tree.ts`
//! module.

use smol_str::SmolStr;

use crate::types::Mode;

/// The atom-typed subset of [`crate::symbols::Group`]. Mirrors upstream's
/// `Atom = "bin" | "close" | "inner" | "open" | "punct" | "rel"` (in
/// `symbols.ts`). The `atom` ParseNode variant carries one of these as
/// its `family`.
///
/// Kept as its own enum (rather than reusing `Group`) so the type system
/// rules out non-atom groups in the parse-node field, matching upstream's
/// `Atom` type. Use [`Atom::as_atom_class`] to obtain the corresponding
/// [`crate::types::AtomClass`] for spacing-table lookup.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Atom {
    Bin,
    Close,
    Inner,
    Open,
    Punct,
    Rel,
}

impl Atom {
    pub const fn as_str(self) -> &'static str {
        match self {
            Atom::Bin => "bin",
            Atom::Close => "close",
            Atom::Inner => "inner",
            Atom::Open => "open",
            Atom::Punct => "punct",
            Atom::Rel => "rel",
        }
    }

    pub const fn as_atom_class(self) -> crate::types::AtomClass {
        match self {
            Atom::Bin => crate::types::AtomClass::MBin,
            Atom::Close => crate::types::AtomClass::MClose,
            Atom::Inner => crate::types::AtomClass::MInner,
            Atom::Open => crate::types::AtomClass::MOpen,
            Atom::Punct => crate::types::AtomClass::MPunct,
            Atom::Rel => crate::types::AtomClass::MRel,
        }
    }
}

/// Argument-parsing modes for [`crate::define_function::FunctionSpec`].
/// Mirrors upstream's `ArgType` union in `types.ts`.
///
/// "original" means the argument is parsed in the same mode as the
/// surrounding context — e.g. the body of `\textcolor` adopts whichever
/// mode the function call sits in. The `Mode` variant pins the argument
/// to math or text regardless of context.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArgType {
    Color,
    Size,
    Url,
    Raw,
    Original,
    Hbox,
    Primitive,
    Mode(Mode),
}

/// Display style passed to `\displaystyle` / `\textstyle` / `\scriptstyle`
/// / `\scriptscriptstyle`. Mirrors upstream's `StyleStr` union.
///
/// Kept distinct from [`crate::style::Style`] (which has eight cramped/
/// uncramped variants for layout) because the parser only ever surfaces
/// these four "user-facing" styles in `styling` parse nodes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StyleStr {
    Text,
    Display,
    Script,
    ScriptScript,
}

/// One column / separator description in an `array` environment row.
/// Mirrors upstream's `AlignSpec` union (`environments/array.ts`).
#[derive(Clone, Debug, PartialEq)]
pub enum AlignSpec {
    Separator {
        separator: SmolStr,
    },
    Align {
        align: SmolStr,
        pregap: Option<f64>,
        postgap: Option<f64>,
    },
}

/// MathML column-separation modes for `array`-derived environments.
/// Mirrors upstream's `ColSeparationType` union.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColSeparationType {
    Align,
    Alignat,
    Gather,
    Small,
    Cd,
}

/// Tokens the parser recognises as group-terminators. Mirrors upstream's
/// `BreakToken` union in `types.ts`. Wired through into
/// [`crate::define_function::FunctionContext`] so handlers know what
/// terminated their argument list.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BreakToken {
    /// `]`
    CloseBracket,
    /// `}`
    CloseBrace,
    /// `\endgroup`
    EndGroup,
    /// `$`
    Dollar,
    /// `\)`
    CloseParen,
    /// `\\`
    DoubleBackslash,
    /// `\end`
    End,
    /// `EOF`
    Eof,
}

impl BreakToken {
    pub const fn as_str(self) -> &'static str {
        match self {
            BreakToken::CloseBracket => "]",
            BreakToken::CloseBrace => "}",
            BreakToken::EndGroup => "\\endgroup",
            BreakToken::Dollar => "$",
            BreakToken::CloseParen => "\\)",
            BreakToken::DoubleBackslash => "\\\\",
            BreakToken::End => "\\end",
            BreakToken::Eof => "EOF",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AtomClass;

    #[test]
    fn atom_to_atom_class_round_trip() {
        let pairs = [
            (Atom::Bin, AtomClass::MBin),
            (Atom::Close, AtomClass::MClose),
            (Atom::Inner, AtomClass::MInner),
            (Atom::Open, AtomClass::MOpen),
            (Atom::Punct, AtomClass::MPunct),
            (Atom::Rel, AtomClass::MRel),
        ];
        for (atom, class) in pairs {
            assert_eq!(atom.as_atom_class(), class);
        }
    }

    #[test]
    fn break_token_text_matches_upstream() {
        assert_eq!(BreakToken::CloseBracket.as_str(), "]");
        assert_eq!(BreakToken::EndGroup.as_str(), "\\endgroup");
        assert_eq!(BreakToken::DoubleBackslash.as_str(), "\\\\");
        assert_eq!(BreakToken::End.as_str(), "\\end");
        assert_eq!(BreakToken::Eof.as_str(), "EOF");
    }
}

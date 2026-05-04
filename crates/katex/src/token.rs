//! Lexed tokens flowing through the gullet.
//!
//! Mirrors upstream KaTeX's `Token` class (`Token.ts`). The text is stored as
//! `SmolStr` so common short tokens (single characters, control sequences up
//! to 22 bytes) live inline without heap allocation.
//!
//! Deviations from upstream, recorded per the project's "deviate
//! deliberately" rule:
//! - Upstream's `Token.loc` is `SourceLocation | null | undefined`; we use
//!   `Option<SourceLocation>` (one absent state instead of two).
//! - `noexpand` and `treat_as_relax` upstream are nullable booleans set
//!   externally on a Token instance after construction. We model them as
//!   plain `bool` fields defaulting to `false`. `Token` is owned by value
//!   through the expander stack so interior mutability is unnecessary.
//! - `text` is `SmolStr` rather than `String`; macro tables and parser code
//!   compare token text by string slice, so the public API is unchanged.

use smol_str::SmolStr;

use crate::source_location::SourceLocation;

/// A lexed token. `text` is one of:
/// * a control word like `"\\frac"` (control words include the backslash
///   but never trailing whitespace, even though the lexer consumes it),
/// * a control symbol like `"\\$"` (backslash + a single non-letter),
/// * a control space `"\\ "` (backslash followed by whitespace in the
///   source — always normalized to this two-char form),
/// * the literal single space `" "` for any run of whitespace,
/// * the synthetic end-of-input marker `"EOF"`,
/// * or a single source character (possibly a multi-byte codepoint plus
///   trailing combining marks).
#[derive(Clone, Debug)]
pub struct Token {
    pub text: SmolStr,
    pub loc: Option<SourceLocation>,
    /// Set by the `\noexpand` macro to suppress the next expansion step.
    pub noexpand: bool,
    /// Set by `\noexpand` alongside `noexpand`; once expansion has stopped,
    /// the token's `text` is rewritten to `"\\relax"` before reaching the
    /// parser (see `MacroExpander::expand_next_token`).
    pub treat_as_relax: bool,
}

impl Token {
    pub fn new(text: impl Into<SmolStr>, loc: Option<SourceLocation>) -> Self {
        Self {
            text: text.into(),
            loc,
            noexpand: false,
            treat_as_relax: false,
        }
    }

    /// Build a new token whose location spans from `self.loc.start` to
    /// `end.loc.end`. Mirrors upstream `Token.range`.
    pub fn range(&self, end: &Token, text: impl Into<SmolStr>) -> Token {
        Token::new(
            text,
            SourceLocation::range(self.loc.as_ref(), end.loc.as_ref()),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn defaults_have_flags_unset() {
        let t = Token::new("\\frac", None);
        assert_eq!(t.text, "\\frac");
        assert!(!t.noexpand);
        assert!(!t.treat_as_relax);
    }

    #[test]
    fn range_merges_locations() {
        let input: Arc<str> = Arc::from("ab");
        let a = Token::new("a", Some(SourceLocation::new(input.clone(), 0, 1)));
        let b = Token::new("b", Some(SourceLocation::new(input.clone(), 1, 2)));
        let merged = a.range(&b, "ab");
        let loc = merged.loc.unwrap();
        assert_eq!(loc.start, 0);
        assert_eq!(loc.end, 2);
    }
}

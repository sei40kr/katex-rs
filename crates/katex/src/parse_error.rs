//! Errors thrown by the parser/builder/renderer.
//!
//! Mirrors upstream KaTeX's `ParseError` class. The `Display` format is kept
//! byte-for-byte compatible with upstream's message: the literal prefix
//! `"KaTeX parse error: "`, an optional `" at position N: "` (or
//! `" at end of input: "`) suffix, and an excerpt of the source with a
//! combining underscore (`\u{0332}`) appended to each character of the offending
//! span. This matters for snapshot parity tests against upstream KaTeX.
//!
//! Deviations from upstream:
//! - Upstream takes a `Token | AnyParseNode` and reads `.loc`. We accept a
//!   `SourceLocation` directly because tokens/parse-nodes do not yet exist
//!   in Phase 0; callers that have a token will pass `tok.loc.clone()`.
//! - Upstream's `position` is reported in JS string indices (UTF-16 code
//!   units). We store byte indices to match Rust's `&str` slicing. For
//!   ASCII LaTeX (the overwhelmingly common case) the two are identical;
//!   "position N" in error messages is therefore byte-based here.

use std::fmt;

use thiserror::Error;

use crate::source_location::SourceLocation;

const CONTEXT_CHARS: usize = 15;

#[derive(Debug, Clone, Error)]
#[error("{}", self.format_message())]
pub struct ParseError {
    /// The underlying error message without any context added.
    pub raw_message: String,
    /// Optional location of the offending source span.
    pub location: Option<SourceLocation>,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            raw_message: message.into(),
            location: None,
        }
    }

    pub fn with_location(message: impl Into<String>, location: SourceLocation) -> Self {
        Self {
            raw_message: message.into(),
            location: Some(location),
        }
    }

    /// Byte offset of the start of the offending span, if known.
    pub fn position(&self) -> Option<usize> {
        self.location.as_ref().map(|l| l.start)
    }

    /// Byte length of the offending span, if known.
    pub fn length(&self) -> Option<usize> {
        self.location.as_ref().map(|l| l.end - l.start)
    }

    fn format_message(&self) -> String {
        let mut out = format!("KaTeX parse error: {}", self.raw_message);
        let Some(loc) = self.location.as_ref() else {
            return out;
        };
        if loc.start > loc.end {
            return out;
        }
        let input: &str = &loc.input;
        if loc.start == input.len() {
            out.push_str(" at end of input: ");
        } else {
            use fmt::Write as _;
            let _ = write!(out, " at position {}: ", loc.start + 1);
        }

        write_left_context(&mut out, input, loc.start);
        write_underlined(&mut out, &input[loc.start..loc.end]);
        write_right_context(&mut out, input, loc.end);
        out
    }
}

fn write_underlined(out: &mut String, s: &str) {
    out.reserve(s.len() * 2);
    for c in s.chars() {
        out.push(c);
        out.push('\u{0332}');
    }
}

fn write_left_context(out: &mut String, input: &str, start: usize) {
    let prefix = &input[..start];
    // Walk back from `start`, taking the byte offset of the (CONTEXT_CHARS+1)-th
    // char from the right; if we run out, no truncation needed.
    if let Some((cutoff, _)) = prefix.char_indices().rev().nth(CONTEXT_CHARS) {
        // `cutoff` is the byte offset of the char one *past* the slice we want;
        // skip it to drop one more char and append the ellipsis.
        let kept_start = cutoff + prefix[cutoff..].chars().next().unwrap().len_utf8();
        out.push('\u{2026}');
        out.push_str(&prefix[kept_start..]);
    } else {
        out.push_str(prefix);
    }
}

fn write_right_context(out: &mut String, input: &str, end: usize) {
    let suffix = &input[end..];
    if let Some((cutoff, _)) = suffix.char_indices().nth(CONTEXT_CHARS) {
        out.push_str(&suffix[..cutoff]);
        out.push('\u{2026}');
    } else {
        out.push_str(suffix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn loc(input: &str, start: usize, end: usize) -> SourceLocation {
        SourceLocation::new(Arc::from(input), start, end)
    }

    #[test]
    fn message_without_location() {
        let e = ParseError::new("oops");
        assert_eq!(e.to_string(), "KaTeX parse error: oops");
    }

    #[test]
    fn message_at_position() {
        // input "abc", offending at byte 1 ("b") of length 1; 1-based
        // position is 2. Underlined "b" with combining underscore.
        let e = ParseError::with_location("bad", loc("abc", 1, 2));
        assert_eq!(
            e.to_string(),
            "KaTeX parse error: bad at position 2: ab\u{0332}c"
        );
    }

    #[test]
    fn message_at_end_of_input() {
        let e = ParseError::with_location("eof", loc("abc", 3, 3));
        assert_eq!(e.to_string(), "KaTeX parse error: eof at end of input: abc");
    }

    #[test]
    fn message_truncates_long_left_context() {
        // 20 chars before start, single char span, none after
        let input = "0123456789abcdefghij!"; // 20 + '!'
        // start at index 20 ('!'), end 21
        let e = ParseError::with_location("x", loc(input, 20, 21));
        // left context is "…" + last 15 chars before start
        // = "…" + "56789abcdefghij"
        // underlined = "!" + combining underscore
        // right context = "" (end == input.len())
        let expected = "KaTeX parse error: x at position 21: \u{2026}56789abcdefghij!\u{0332}";
        assert_eq!(e.to_string(), expected);
    }

    #[test]
    fn message_truncates_long_right_context() {
        // 1 char before, 1 underlined, 20 chars after
        let input = "x!0123456789abcdefghij";
        // start 1 ('!'), end 2
        let e = ParseError::with_location("y", loc(input, 1, 2));
        // upstream: end + 15 < input.length i.e. 2 + 15 = 17 < 22 -> truncate
        // right = input.slice(end, end+15) + "…" = "0123456789abcde…"
        let expected = "KaTeX parse error: y at position 2: x!\u{0332}0123456789abcde\u{2026}";
        assert_eq!(e.to_string(), expected);
    }

    #[test]
    fn position_and_length_helpers() {
        let e = ParseError::with_location("z", loc("abcd", 1, 3));
        assert_eq!(e.position(), Some(1));
        assert_eq!(e.length(), Some(2));

        let e = ParseError::new("nada");
        assert!(e.position().is_none());
        assert!(e.length().is_none());
    }
}

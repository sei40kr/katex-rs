//! Lexing/parsing positional information for error reporting.
//!
//! Mirrors upstream KaTeX's `SourceLocation`. The upstream class stores a
//! reference to a `Lexer` so its `input` string can be retrieved later;
//! we instead carry an `Arc<str>` directly. This keeps `SourceLocation`
//! self-contained, lets it outlive the lexer, and avoids a circular type
//! dependency between lexer/token/source-location in Rust.

use std::sync::Arc;

/// A range `[start, end)` (zero-based, end-exclusive) into an immutable
/// input string. The input is held via `Arc<str>` so cloning a
/// `SourceLocation` is cheap and locations can be freely passed around.
#[derive(Clone, Debug)]
pub struct SourceLocation {
    pub start: usize,
    pub end: usize,
    pub input: Arc<str>,
}

impl SourceLocation {
    pub fn new(input: Arc<str>, start: usize, end: usize) -> Self {
        Self { start, end, input }
    }

    /// Merge two locations into a single range covering both.
    ///
    /// Returns `None` if either location is missing or if they refer to
    /// different inputs (compared by `Arc` pointer equality, mirroring
    /// upstream's `lexer !==` reference check).
    pub fn range(first: Option<&SourceLocation>, second: Option<&SourceLocation>) -> Option<Self> {
        match (first, second) {
            (Some(a), Some(b)) if Arc::ptr_eq(&a.input, &b.input) => {
                Some(Self::new(a.input.clone(), a.start, b.end))
            }
            (Some(a), None) => Some(a.clone()),
            _ => None,
        }
    }
}

impl PartialEq for SourceLocation {
    // `Arc::ptr_eq` (not `==` on `Arc<str>`) mirrors upstream's
    // `lexer === lexer` reference-identity check: two locations into
    // *equal but distinct* input buffers are not equal.
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end == other.end && Arc::ptr_eq(&self.input, &other.input)
    }
}

impl Eq for SourceLocation {}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Arc<str> {
        Arc::from("hello world")
    }

    #[test]
    fn range_merges_two_locations_on_same_input() {
        let s = input();
        let a = SourceLocation::new(s.clone(), 0, 3);
        let b = SourceLocation::new(s.clone(), 6, 11);
        let merged = SourceLocation::range(Some(&a), Some(&b)).unwrap();
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 11);
        assert!(Arc::ptr_eq(&merged.input, &s));
    }

    #[test]
    fn range_returns_first_when_second_missing() {
        let s = input();
        let a = SourceLocation::new(s.clone(), 0, 3);
        let merged = SourceLocation::range(Some(&a), None).unwrap();
        assert_eq!(merged, a);
    }

    #[test]
    fn range_returns_none_when_first_missing() {
        let s = input();
        let b = SourceLocation::new(s, 6, 11);
        assert!(SourceLocation::range(None, Some(&b)).is_none());
    }

    #[test]
    fn range_returns_none_for_different_inputs() {
        let a = SourceLocation::new(Arc::from("abc"), 0, 1);
        let b = SourceLocation::new(Arc::from("abc"), 1, 2);
        assert!(SourceLocation::range(Some(&a), Some(&b)).is_none());
    }
}

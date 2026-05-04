//! Regex-driven tokenizer.
//!
//! Mirrors upstream KaTeX's `Lexer.ts`. The tokenization grammar — including
//! how control words swallow trailing whitespace, how runs of whitespace
//! collapse to a single space token, and how `\verb`/`\verb*` capture their
//! body up to a delimiter character — is reproduced here so snapshot tests
//! against upstream tokenize identically.
//!
//! Deviations from upstream, per the project's "deviate deliberately" rule:
//!
//! - Upstream uses a single `RegExp` with the global (`g`) flag and tracks
//!   the cursor through the regex's mutable `lastIndex`. We hold an explicit
//!   `pos: usize` byte cursor and call `Regex::captures_at(input, pos)`
//!   with a manual `start == pos` check — equivalent semantics, but the
//!   lexer has no hidden mutable state in the regex.
//! - Upstream's tokenRegex contains backreferences (`\verb*X.*?X`,
//!   `\verbX.*?X`), which the `regex` crate intentionally doesn't support.
//!   We hand-scan `\verb` and `\verb*` ahead of the main regex match; the
//!   resulting token text and span are identical to upstream's.
//! - Upstream walks UTF-16 code units, so the surrogate-pair branch
//!   `[\uD800-\uDBFF][\uDC00-\uDFFF]` matches astral characters as a pair
//!   of code units. We operate on Rust `&str` (UTF-8 / scalar values), so
//!   the equivalent class is `[\u{10000}-\u{10FFFF}]`. For input that is
//!   valid Unicode (the only kind `&str` admits) the matched span is the
//!   same.
//! - Upstream's `Settings.reportNonstrict` is invoked when a `%` comment
//!   runs to end-of-input. Strict callbacks aren't wired through to the
//!   lexer in Phase 1; the comment is consumed silently. A TODO is left in
//!   place for Phase 4 once the parser-side strict infrastructure lands.
//! - `^^` (TeX's caret-caret hex escape) is **not** implemented in upstream
//!   KaTeX and is not implemented here either. `^^A` tokenizes as three
//!   separate tokens `^`, `^`, `A`. The Phase 1 acceptance test for "^^
//!   escapes" therefore asserts pass-through behavior.
//! - Upstream does not strip a leading byte-order mark (U+FEFF). It falls
//!   into the `\u{F900}-\u{FFFF}` range and tokenizes as a regular single
//!   character. We match this behavior.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use regex::Regex;
use smol_str::SmolStr;

use crate::parse_error::ParseError;
use crate::source_location::SourceLocation;
use crate::token::Token;

// Mirrors upstream's `tokenRegexString` (Lexer.ts) with two structural
// changes documented in the module-level comment:
//   1. The astral surrogate-pair branch is replaced with a single scalar
//      class `[\u{10000}-\u{10FFFF}]`.
//   2. The `\verb` and `\verb*` branches are removed and handled in code,
//      since the `regex` crate does not support backreferences.
//
// Capture group layout (1-indexed):
//   1: whitespace run
//   2: control-space whitespace body (the chars after the leading `\`)
//   3: control-word name (the leading `\` plus the letters)
//   4: any single matched element (used as the fallback token text)
const TOKEN_REGEX_STR: &str = concat!(
    r"([ \r\n\t]+)",
    r"|\\(\n|[ \r\t]+\n?)[ \r\t]*",
    r"|(\\[a-zA-Z@]+)[ \r\n\t]*",
    "|(",
    r"[!-\[\]-\u{2027}\u{202A}-\u{D7FF}\u{F900}-\u{FFFF}][\u{0300}-\u{036F}]*",
    r"|[\u{10000}-\u{10FFFF}][\u{0300}-\u{036F}]*",
    r"|\\[\u{0}-\u{FFFF}]",
    ")",
);

fn token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(TOKEN_REGEX_STR).expect("token regex compiles"))
}

/// Stateful tokenizer over an immutable input buffer.
pub struct Lexer {
    input: Arc<str>,
    pos: usize,
    /// Category codes. The lexer itself only consults catcode 14 (comment);
    /// `MacroExpander` additionally checks catcode 13 (active) when
    /// resolving single-character expansions. Defaults match upstream:
    /// `%` is a comment, `~` is active.
    pub catcodes: HashMap<SmolStr, u8>,
}

impl Lexer {
    pub fn new(input: impl Into<Arc<str>>) -> Self {
        let mut catcodes = HashMap::new();
        catcodes.insert(SmolStr::new_static("%"), 14);
        catcodes.insert(SmolStr::new_static("~"), 13);
        Self {
            input: input.into(),
            pos: 0,
            catcodes,
        }
    }

    pub fn input(&self) -> &Arc<str> {
        &self.input
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_catcode(&mut self, ch: impl Into<SmolStr>, code: u8) {
        self.catcodes.insert(ch.into(), code);
    }

    /// Lex one token, advancing the cursor past it.
    pub fn lex(&mut self) -> Result<Token, ParseError> {
        loop {
            let pos = self.pos;
            if pos == self.input.len() {
                return Ok(self.make_token(SmolStr::new_static("EOF"), pos, pos));
            }

            // Hand-scan \verb/\verb* before the main regex. If neither
            // shape matches, fall through to the regex which will pick up
            // `\verb` as a plain control word.
            if let Some(tok) = self.try_lex_verb()? {
                return Ok(tok);
            }

            let captures = match token_regex().captures_at(&self.input, pos) {
                Some(c) if c.get(0).expect("group 0").start() == pos => c,
                _ => return Err(self.unexpected_char_error(pos)),
            };
            let m = captures.get(0).expect("group 0");
            let end = m.end();
            self.pos = end;

            // Group 3 = control word (without trailing whitespace);
            // Group 4 = any single element (single char, surrogate, control symbol);
            // Group 2 set = control-space; Group 1 set = whitespace run.
            let text: SmolStr = if let Some(cw) = captures.get(3) {
                SmolStr::new(cw.as_str())
            } else if let Some(elem) = captures.get(4) {
                SmolStr::new(elem.as_str())
            } else if captures.get(2).is_some() {
                SmolStr::new_static("\\ ")
            } else {
                SmolStr::new_static(" ")
            };

            // Comment character: skip to next '\n' (or EOF) and re-lex.
            if self.catcodes.get(text.as_str()).copied() == Some(14) {
                if let Some(rel) = self.input[self.pos..].find('\n') {
                    self.pos = self.pos + rel + 1;
                } else {
                    // TODO: settings.report_nonstrict("commentAtEnd", ...)
                    // once strict-mode plumbing reaches the lexer.
                    self.pos = self.input.len();
                }
                continue;
            }

            return Ok(self.make_token(text, pos, end));
        }
    }

    fn make_token(&self, text: SmolStr, start: usize, end: usize) -> Token {
        Token::new(
            text,
            Some(SourceLocation::new(self.input.clone(), start, end)),
        )
    }

    /// Try to match `\verb*X...X` or `\verbX...X` at the cursor. Returns
    /// `Ok(None)` if the cursor isn't at a `\verb`/`\verb*` form, *or* if
    /// the form is structurally invalid (no delimiter, unstarred delimiter
    /// is `*` or a letter, body crosses a newline). On `None`, the caller
    /// falls back to the main regex which will tokenize `\verb` as a plain
    /// control word — matching upstream's regex-backtracking semantics.
    fn try_lex_verb(&mut self) -> Result<Option<Token>, ParseError> {
        let pos = self.pos;
        let s = &self.input[pos..];
        let starred = s.starts_with("\\verb*");
        let unstarred = !starred && s.starts_with("\\verb");
        if !starred && !unstarred {
            return Ok(None);
        }
        // After `\verb`, the next char (control-word boundary) must not be
        // a letter — otherwise `\verbose` etc. wouldn't be a control word.
        // We already ate `\verb*` for the starred branch.
        let prefix_len = if starred { 6 } else { 5 };
        let after = &s[prefix_len..];
        let Some(delim) = after.chars().next() else {
            return Ok(None);
        };
        // Unstarred `\verb` cannot be followed by `*` (that's the starred
        // form) nor by a letter (that's `\verbX...`, a longer control
        // word). Mirrors upstream's `[^*a-zA-Z]` delimiter class.
        if !starred && (delim == '*' || delim.is_ascii_alphabetic()) {
            return Ok(None);
        }
        let body_start = delim.len_utf8();
        // Scan for a matching delimiter. Upstream's `.*?` doesn't cross
        // newlines, so a `\n` before the closing delim aborts the match.
        let mut close_end_in_after: Option<usize> = None;
        for (i, c) in after[body_start..].char_indices() {
            if c == '\n' {
                return Ok(None);
            }
            if c == delim {
                close_end_in_after = Some(body_start + i + c.len_utf8());
                break;
            }
        }
        let Some(close_end) = close_end_in_after else {
            return Ok(None);
        };
        let end = pos + prefix_len + close_end;
        let text = SmolStr::new(&self.input[pos..end]);
        self.pos = end;
        Ok(Some(self.make_token(text, pos, end)))
    }

    fn unexpected_char_error(&self, pos: usize) -> ParseError {
        let ch = self.input[pos..]
            .chars()
            .next()
            .expect("not at end of input");
        ParseError::with_location(
            format!("Unexpected character: '{}'", ch),
            SourceLocation::new(self.input.clone(), pos, pos + ch.len_utf8()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_all(input: &str) -> Vec<String> {
        let mut lex = Lexer::new(input);
        let mut out = Vec::new();
        loop {
            let t = lex.lex().expect("lex error");
            if t.text == "EOF" {
                break;
            }
            out.push(t.text.to_string());
        }
        out
    }

    #[test]
    fn frac_one_two() {
        // `\frac{1}{2}` — control word, then literal {, 1, }, {, 2, }.
        assert_eq!(
            lex_all(r"\frac{1}{2}"),
            vec![r"\frac", "{", "1", "}", "{", "2", "}"]
        );
    }

    #[test]
    fn alpha_sub_sup() {
        // `\alpha_i^2`
        assert_eq!(lex_all(r"\alpha_i^2"), vec![r"\alpha", "_", "i", "^", "2"]);
    }

    #[test]
    fn control_word_swallows_trailing_whitespace() {
        // `\frac  {1}` — the spaces between control word and `{` are
        // consumed by the control-word match, not emitted as a separate
        // space token.
        assert_eq!(lex_all(r"\frac  {1}"), vec![r"\frac", "{", "1", "}"]);
    }

    #[test]
    fn control_word_vs_control_symbol() {
        // Letters → control word; non-letter → control symbol.
        assert_eq!(lex_all(r"\foo\$\@x"), vec![r"\foo", r"\$", r"\@x"]);
    }

    #[test]
    fn run_of_whitespace_collapses_to_single_space_token() {
        // Outside a control-word context, any run of whitespace is one
        // " " token but the cursor advances past the entire run.
        assert_eq!(lex_all("a   b"), vec!["a", " ", "b"]);
        assert_eq!(lex_all("a\n\tb"), vec!["a", " ", "b"]);
    }

    #[test]
    fn control_space_canonicalizes_to_backslash_space() {
        // `\ ` (or `\` + any whitespace) is the canonical control-space
        // token "\\ " regardless of which whitespace follows.
        assert_eq!(lex_all("\\ "), vec!["\\ "]);
        assert_eq!(lex_all("\\\n"), vec!["\\ "]);
        assert_eq!(lex_all("\\  \t"), vec!["\\ "]);
    }

    #[test]
    fn comments_skip_to_newline() {
        // `%` plus everything to the next newline is dropped.
        assert_eq!(lex_all("a%comment\nb"), vec!["a", "b"]);
        // Comment with no terminating newline → consumes to EOF.
        assert_eq!(lex_all("a%trailing"), vec!["a"]);
    }

    #[test]
    fn caret_caret_is_two_separate_caret_tokens() {
        // KaTeX upstream does not implement TeX's `^^` hex escapes;
        // `^^A` tokenizes as three separate tokens.
        assert_eq!(lex_all("^^A"), vec!["^", "^", "A"]);
    }

    #[test]
    fn bom_passes_through_as_a_character_token() {
        // U+FEFF falls into the lexer's BMP range and becomes a regular
        // single-character token (matching upstream).
        let toks = lex_all("\u{FEFF}x");
        assert_eq!(toks, vec!["\u{FEFF}", "x"]);
    }

    #[test]
    fn locations_track_byte_offsets() {
        let mut lex = Lexer::new(r"\frac{1}");
        let t = lex.lex().unwrap();
        let loc = t.loc.unwrap();
        assert_eq!(t.text, r"\frac");
        assert_eq!(loc.start, 0);
        assert_eq!(loc.end, 5);
        // Next token: `{` at byte 5..6.
        let t = lex.lex().unwrap();
        let loc = t.loc.unwrap();
        assert_eq!(t.text, "{");
        assert_eq!(loc.start, 5);
        assert_eq!(loc.end, 6);
    }

    #[test]
    fn eof_token_returned_at_end() {
        let mut lex = Lexer::new("a");
        assert_eq!(lex.lex().unwrap().text, "a");
        let eof = lex.lex().unwrap();
        assert_eq!(eof.text, "EOF");
        let loc = eof.loc.unwrap();
        assert_eq!(loc.start, 1);
        assert_eq!(loc.end, 1);
    }

    #[test]
    fn unexpected_character_error() {
        // U+E000 is in the BMP private use area, which the regex
        // explicitly excludes; the lexer should report it as unexpected.
        let mut lex = Lexer::new("\u{E000}");
        let err = lex.lex().unwrap_err();
        assert!(err.raw_message.starts_with("Unexpected character"));
    }

    #[test]
    fn verb_starred_captures_body() {
        // `\verb*|x|` → single token `\verb*|x|`.
        assert_eq!(lex_all(r"\verb*|x|"), vec![r"\verb*|x|"]);
    }

    #[test]
    fn verb_unstarred_with_non_letter_delimiter() {
        assert_eq!(lex_all(r"\verb|abc|"), vec![r"\verb|abc|"]);
    }

    #[test]
    fn verb_unstarred_falls_back_when_followed_by_letter() {
        // `\verbose` is just a control word — the unstarred `\verb`
        // delimiter class excludes letters.
        assert_eq!(lex_all(r"\verbose"), vec![r"\verbose"]);
    }

    #[test]
    fn verb_without_closing_delim_falls_back_to_control_word() {
        // No closing `|` — the `\verb` branch fails and we tokenize
        // `\verb` as a plain control word, leaving `|abc` to follow.
        assert_eq!(lex_all(r"\verb|abc"), vec![r"\verb", "|", "a", "b", "c"]);
    }

    #[test]
    fn combining_marks_attach_to_preceding_codepoint() {
        // U+0301 is a combining acute. `e\u{0301}` is a single token.
        let toks = lex_all("e\u{0301}");
        assert_eq!(toks, vec!["e\u{0301}"]);
    }

    #[test]
    fn astral_codepoint_is_a_single_token() {
        // U+1F600 (😀) — one Rust char, one token.
        let toks = lex_all("\u{1F600}");
        assert_eq!(toks, vec!["\u{1F600}"]);
    }
}

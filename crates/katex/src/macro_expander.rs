//! Macro expansion engine — the "gullet" sitting between lexer and parser.
//!
//! Mirrors upstream KaTeX's `MacroExpander.ts`. Token flow matches upstream
//! exactly: the push-back stack is held in reverse order (top of stack =
//! last element = next token emitted), and macro bodies / argument tokens
//! are likewise stored reversed so they can be `pushTokens`-ed straight
//! onto the stack.
//!
//! Deviations from upstream, per the project's "deviate deliberately" rule:
//!
//! - `MacroDefinition` is a Rust `enum` rather than a TypeScript union of
//!   `string | MacroExpansion | ((ctx) => ...)`. The `Builtin` variant
//!   wraps a plain `fn` pointer; the codegen-driven built-in table arrives
//!   in Phase 2 (the namespace's `builtins` map is empty for Phase 1).
//! - Built-in functions take `&mut MacroExpander` directly rather than a
//!   `MacroContextInterface` trait. Upstream's interface exists to break a
//!   TypeScript circular import; Rust doesn't need it.
//! - `expandOnce` returns `Result<ExpandStep, ParseError>` where
//!   `ExpandStep` is `NotExpanded | Pushed(usize)`. Upstream encodes this
//!   as `number | boolean`, which Rust wouldn't model cleanly.
//! - The expander borrows `&'s Settings` rather than holding it by Arc.
//!   Macros from `settings.macros` (a `HashMap<String, String>`) are
//!   converted to `MacroDefinition::Source` once at construction.
//! - Strict-mode reporting (`reportNonstrict`) is not yet wired through;
//!   Phase 4's parser will plumb it through. The expander itself never
//!   calls it directly.

use std::collections::HashMap;

use smol_str::SmolStr;

use crate::lexer::Lexer;
use crate::parse_error::ParseError;
use crate::settings::Settings;
use crate::source_location::SourceLocation;
use crate::token::Token;
use crate::types::Mode;

/// Pre-tokenized macro body with optional argument metadata. `tokens`
/// is in **reverse** order (top of expansion = last element).
#[derive(Clone, Debug)]
pub struct MacroExpansion {
    pub tokens: Vec<Token>,
    pub num_args: usize,
    /// Delimiter sequences for delimited parameters. If present, the
    /// outer `Vec` has length `num_args + 1`: index 0 is the prefix
    /// matched before the first arg, indices `1..=num_args` are the
    /// trailing delimiters of each arg.
    pub delimiters: Option<Vec<Vec<SmolStr>>>,
    /// `\let`-style aliases set this so `expandTokens` (with
    /// `expandableOnly`) leaves them as opaque tokens.
    pub unexpandable: bool,
}

impl MacroExpansion {
    pub fn new(tokens: Vec<Token>, num_args: usize) -> Self {
        Self {
            tokens,
            num_args,
            delimiters: None,
            unexpandable: false,
        }
    }
}

/// Result returned by built-in (function) macros. Either a raw source
/// body (will be tokenized lazily) or a fully-prepared expansion.
#[derive(Clone, Debug)]
pub enum BuiltinResult {
    Source(SmolStr),
    Expansion(MacroExpansion),
}

/// A built-in macro callback. Receives mutable access to the expander so
/// it can pop tokens, peek the future, etc.
pub type BuiltinFn = fn(&mut MacroExpander<'_>) -> Result<BuiltinResult, ParseError>;

#[derive(Clone, Debug)]
pub enum MacroDefinition {
    /// Raw macro body text (the common case — `\def` macros, user macros
    /// from `Settings.macros`). Tokenized lazily on first expansion.
    Source(SmolStr),
    /// Pre-tokenized expansion. Used by `\let` aliases and Phase 2's
    /// codegen output.
    Expansion(MacroExpansion),
    /// Built-in dynamic macro. Phase 1 keeps the variant for type-shape
    /// parity with upstream; the macros table itself is empty.
    Builtin(BuiltinFn),
}

/// One step of expansion.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ExpandStep {
    /// The top token was not expandable (or was suppressed by
    /// `\noexpand`). It has been pushed back onto the stack.
    NotExpanded,
    /// The top token was expanded; `n` tokens are now on the stack
    /// in its place (where `n` may be zero).
    Pushed(usize),
}

pub struct MacroExpander<'s> {
    pub mode: Mode,
    pub macros: crate::namespace::Namespace<MacroDefinition>,
    settings: &'s Settings,
    expansion_count: u32,
    lexer: Lexer,
    /// Push-back buffer in **reverse** order: `last()` is the next token
    /// to emit; `push` adds at the tail.
    stack: Vec<Token>,
}

impl<'s> MacroExpander<'s> {
    pub fn new(input: impl Into<std::sync::Arc<str>>, settings: &'s Settings, mode: Mode) -> Self {
        let lexer = Lexer::new(input);
        // Phase 2 codegen will provide the upstream built-in macro table;
        // for Phase 1 the builtins map is empty.
        let builtins: HashMap<String, MacroDefinition> = HashMap::new();
        let globals: HashMap<String, MacroDefinition> = settings
            .macros
            .iter()
            .map(|(k, v)| (k.clone(), MacroDefinition::Source(SmolStr::new(v))))
            .collect();
        Self {
            mode,
            macros: crate::namespace::Namespace::new(builtins, globals),
            settings,
            expansion_count: 0,
            lexer,
            stack: Vec::new(),
        }
    }

    pub fn feed(&mut self, input: impl Into<std::sync::Arc<str>>) {
        self.lexer = Lexer::new(input);
    }

    pub fn switch_mode(&mut self, new_mode: Mode) {
        self.mode = new_mode;
    }

    pub fn begin_group(&mut self) {
        self.macros.begin_group();
    }

    pub fn end_group(&mut self) -> Result<(), ParseError> {
        self.macros.end_group()
    }

    pub fn end_groups(&mut self) -> Result<(), ParseError> {
        self.macros.end_groups()
    }

    /// Peek the next unexpanded token without removing it.
    pub fn future(&mut self) -> Result<&Token, ParseError> {
        if self.stack.is_empty() {
            let t = self.lexer.lex()?;
            self.stack.push(t);
        }
        Ok(self.stack.last().expect("just pushed"))
    }

    /// Remove and return the next unexpanded token.
    pub fn pop_token(&mut self) -> Result<Token, ParseError> {
        self.future()?;
        Ok(self.stack.pop().expect("future ensured non-empty"))
    }

    pub fn push_token(&mut self, t: Token) {
        self.stack.push(t);
    }

    /// Append tokens to the stack. The caller is responsible for
    /// reversing the list if it represents a forward token stream.
    pub fn push_tokens(&mut self, tokens: impl IntoIterator<Item = Token>) {
        self.stack.extend(tokens);
    }

    /// Pop unexpanded space tokens (single-char `" "` only — control-space
    /// `"\\ "` is not consumed, matching upstream).
    pub fn consume_spaces(&mut self) -> Result<(), ParseError> {
        loop {
            let is_space = self.future()?.text == " ";
            if !is_space {
                return Ok(());
            }
            self.stack.pop();
        }
    }

    fn count_expansion(&mut self, amount: u32) -> Result<(), ParseError> {
        self.expansion_count = self.expansion_count.saturating_add(amount);
        if let Some(max) = self.settings.max_expand
            && self.expansion_count > max
        {
            return Err(ParseError::new(
                "Too many expansions: infinite loop or need to increase maxExpand setting",
            ));
        }
        Ok(())
    }

    /// Consume a macro argument: either a balanced `{...}` group, the
    /// next single token, or — when `delims` is provided — a sequence of
    /// tokens up to (and not including) those delimiters. Returns the
    /// argument tokens **reversed**, plus the start/end tokens whose
    /// locations bracket the argument span.
    pub fn consume_arg(&mut self, delims: Option<&[SmolStr]>) -> Result<MacroArg, ParseError> {
        let is_delimited = delims.is_some_and(|d| !d.is_empty());
        if !is_delimited {
            self.consume_spaces()?;
        }
        let start = self.future()?.clone();
        let mut tokens: Vec<Token> = Vec::new();
        let mut depth: i32 = 0;
        let mut match_idx: usize = 0;
        let mut last_tok: Token;
        loop {
            let tok = self.pop_token()?;
            tokens.push(tok.clone());
            last_tok = tok.clone();
            match tok.text.as_str() {
                "{" => depth += 1,
                "}" => {
                    depth -= 1;
                    if depth == -1 {
                        return Err(ParseError::with_location(
                            "Extra }",
                            tok.loc.clone().unwrap_or_else(|| {
                                SourceLocation::new(self.lexer.input().clone(), 0, 0)
                            }),
                        ));
                    }
                }
                "EOF" => {
                    let expected: String = if let Some(ds) = delims {
                        if is_delimited {
                            ds.get(match_idx).map(|s| s.to_string()).unwrap_or_default()
                        } else {
                            "}".to_string()
                        }
                    } else {
                        "}".to_string()
                    };
                    let msg = format!(
                        "Unexpected end of input in a macro argument, expected '{}'",
                        expected
                    );
                    return Err(match tok.loc.clone() {
                        Some(loc) => ParseError::with_location(msg, loc),
                        None => ParseError::new(msg),
                    });
                }
                _ => {}
            }
            if let Some(ds) = delims
                && is_delimited
            {
                let at_match_depth = depth == 0
                    || (depth == 1 && ds.get(match_idx).map(|d| d.as_str()) == Some("{"));
                if at_match_depth
                    && Some(tok.text.as_str()) == ds.get(match_idx).map(SmolStr::as_str)
                {
                    match_idx += 1;
                    if match_idx == ds.len() {
                        // Drop the matched delimiter tokens from the tail.
                        let new_len = tokens.len() - match_idx;
                        tokens.truncate(new_len);
                        break;
                    }
                } else {
                    match_idx = 0;
                }
            }
            if !(depth != 0 || is_delimited) {
                break;
            }
        }
        // Strip outer braces if argument has the form `{...}`.
        if start.text == "{" && tokens.last().map(|t| t.text.as_str()) == Some("}") {
            tokens.pop();
            tokens.remove(0);
        }
        tokens.reverse();
        Ok(MacroArg {
            tokens,
            start,
            end: last_tok,
        })
    }

    /// Consume `num_args` macro arguments, optionally with delimiters.
    pub fn consume_args(
        &mut self,
        num_args: usize,
        delimiters: Option<&[Vec<SmolStr>]>,
    ) -> Result<Vec<Vec<Token>>, ParseError> {
        if let Some(delims) = delimiters {
            if delims.len() != num_args + 1 {
                return Err(ParseError::new(
                    "The length of delimiters doesn't match the number of args!",
                ));
            }
            let prefix = &delims[0];
            for d in prefix {
                let tok = self.pop_token()?;
                if &tok.text != d {
                    return Err(match tok.loc.clone() {
                        Some(loc) => ParseError::with_location(
                            "Use of the macro doesn't match its definition",
                            loc,
                        ),
                        None => ParseError::new("Use of the macro doesn't match its definition"),
                    });
                }
            }
        }
        let mut args = Vec::with_capacity(num_args);
        for i in 0..num_args {
            let arg_delims = delimiters.map(|d| d[i + 1].as_slice());
            args.push(self.consume_arg(arg_delims)?.tokens);
        }
        Ok(args)
    }

    /// Resolve a macro name to a ready-to-push `MacroExpansion`. Mirrors
    /// upstream `_getExpansion` (string bodies are tokenized with a
    /// throwaway lexer; tokens are reversed for stack ordering). Returns
    /// `None` if the name isn't bound, or is bound to a single character
    /// with a non-active catcode (catcode != 13).
    fn get_expansion(&mut self, name: &str) -> Result<Option<MacroExpansion>, ParseError> {
        let definition = match self.macros.get(name) {
            Some(d) => d.clone(),
            None => return Ok(None),
        };
        // Single-char active-catcode check.
        if name.chars().count() == 1
            && let Some(&code) = self.macros_lexer_catcode(name)
            && code != 13
        {
            return Ok(None);
        }
        let result = match definition {
            MacroDefinition::Source(s) => BuiltinResult::Source(s),
            MacroDefinition::Expansion(e) => return Ok(Some(e)),
            MacroDefinition::Builtin(f) => f(self)?,
        };
        let exp = match result {
            BuiltinResult::Source(body) => Self::tokenize_source_body(&body)?,
            BuiltinResult::Expansion(e) => e,
        };
        Ok(Some(exp))
    }

    fn macros_lexer_catcode(&self, name: &str) -> Option<&u8> {
        self.lexer.catcodes.get(name)
    }

    /// Build a `MacroExpansion` from a raw source body. Counts `#1..#9`
    /// references (treating `##` as a literal `#`) to derive `num_args`,
    /// then tokenizes the body with a throwaway lexer and reverses the
    /// token list.
    fn tokenize_source_body(body: &str) -> Result<MacroExpansion, ParseError> {
        let mut num_args: usize = 0;
        if body.contains('#') {
            let stripped = body.replace("##", "");
            while stripped.contains(&format!("#{}", num_args + 1)) {
                num_args += 1;
            }
        }
        let mut body_lexer = Lexer::new(body.to_string());
        let mut tokens = Vec::new();
        loop {
            let t = body_lexer.lex()?;
            if t.text == "EOF" {
                break;
            }
            tokens.push(t);
        }
        tokens.reverse();
        Ok(MacroExpansion {
            tokens,
            num_args,
            delimiters: None,
            unexpandable: false,
        })
    }

    /// Expand the next token at most once. See `ExpandStep`.
    fn expand_once_inner(&mut self, expandable_only: bool) -> Result<ExpandStep, ParseError> {
        let top_token = self.pop_token()?;
        let name = top_token.text.clone();
        let expansion = if !top_token.noexpand {
            self.get_expansion(&name)?
        } else {
            None
        };
        let exp = match expansion {
            Some(e) if !(expandable_only && e.unexpandable) => e,
            other => {
                if expandable_only
                    && other.is_none()
                    && name.starts_with('\\')
                    && !self.is_defined(&name)
                {
                    return Err(ParseError::new(format!(
                        "Undefined control sequence: {}",
                        name
                    )));
                }
                self.push_token(top_token);
                return Ok(ExpandStep::NotExpanded);
            }
        };
        self.count_expansion(1)?;
        let delim_owned = exp.delimiters.clone();
        let args = self.consume_args(exp.num_args, delim_owned.as_deref())?;
        let mut tokens = exp.tokens;
        if exp.num_args > 0 {
            // Right-to-left walk, looking for `#` in the (reversed) body.
            let mut i = tokens.len();
            while i > 0 {
                i -= 1;
                if tokens[i].text == "#" {
                    if i == 0 {
                        return Err(match tokens[0].loc.clone() {
                            Some(loc) => ParseError::with_location(
                                "Incomplete placeholder at end of macro body",
                                loc,
                            ),
                            None => ParseError::new("Incomplete placeholder at end of macro body"),
                        });
                    }
                    // `prev` is the next token in source order (since the
                    // body is reversed).
                    let prev_idx = i - 1;
                    let prev_text = tokens[prev_idx].text.clone();
                    if prev_text == "#" {
                        // `##` → drop the first `#`, keeping one literal.
                        tokens.remove(i);
                        i = prev_idx; // continue scanning from prev
                    } else if let Some(n) = parse_arg_index(&prev_text) {
                        // Replace the `#N` pair (positions prev_idx..=i)
                        // with args[n - 1] (already reversed for stack).
                        tokens.splice(prev_idx..=i, args[n - 1].iter().cloned());
                        // After splice, the next index to inspect is the
                        // one just before the inserted block.
                        i = prev_idx;
                    } else {
                        return Err(match tokens[prev_idx].loc.clone() {
                            Some(loc) => {
                                ParseError::with_location("Not a valid argument number", loc)
                            }
                            None => ParseError::new("Not a valid argument number"),
                        });
                    }
                }
            }
        }
        let pushed = tokens.len();
        self.push_tokens(tokens);
        Ok(ExpandStep::Pushed(pushed))
    }

    /// Public expand-once for callers that want the upstream-style
    /// "false or count" result. Errors propagate normally.
    pub fn expand_once(&mut self) -> Result<Option<usize>, ParseError> {
        match self.expand_once_inner(false)? {
            ExpandStep::NotExpanded => Ok(None),
            ExpandStep::Pushed(n) => Ok(Some(n)),
        }
    }

    pub fn expand_after_future(&mut self) -> Result<&Token, ParseError> {
        self.expand_once_inner(false)?;
        self.future()
    }

    /// Recursively expand the next token; return the first
    /// non-expandable result. If the token was passed through
    /// `\noexpand`, its `text` is rewritten to `"\relax"` here (matching
    /// upstream's `treatAsRelax` rule for the parser-facing path).
    pub fn expand_next_token(&mut self) -> Result<Token, ParseError> {
        loop {
            if matches!(self.expand_once_inner(false)?, ExpandStep::NotExpanded) {
                let mut token = self
                    .stack
                    .pop()
                    .expect("expand_once leaves stack non-empty on NotExpanded");
                if token.treat_as_relax {
                    token.text = SmolStr::new_static("\\relax");
                }
                return Ok(token);
            }
        }
    }

    /// Fully expand `tokens` (which must be in **reverse** order — the
    /// caller's responsibility) and return the result in **forward**
    /// order. Used by `\edef`-style operations and by `expand_macro`.
    pub fn expand_tokens(&mut self, tokens: Vec<Token>) -> Result<Vec<Token>, ParseError> {
        let mut output: Vec<Token> = Vec::new();
        let old_len = self.stack.len();
        self.push_tokens(tokens);
        while self.stack.len() > old_len {
            if matches!(self.expand_once_inner(true)?, ExpandStep::NotExpanded) {
                let mut token = self.stack.pop().expect("non-empty");
                if token.treat_as_relax {
                    token.noexpand = false;
                    token.treat_as_relax = false;
                }
                output.push(token);
            }
        }
        let n = output.len() as u32;
        self.count_expansion(n)?;
        Ok(output)
    }

    pub fn expand_macro(&mut self, name: &str) -> Result<Option<Vec<Token>>, ParseError> {
        if !self.macros.has(name) {
            return Ok(None);
        }
        let seed = vec![Token::new(name, None)];
        Ok(Some(self.expand_tokens(seed)?))
    }

    pub fn expand_macro_as_text(&mut self, name: &str) -> Result<Option<String>, ParseError> {
        let toks = self.expand_macro(name)?;
        Ok(toks.map(|ts| ts.iter().map(|t| t.text.as_str()).collect()))
    }

    /// Whether `name` resolves to *anything* meaningful — a macro,
    /// a function/symbol (Phase 5+), or one of the implicit commands
    /// (`^`, `_`, `\limits`, `\nolimits`).
    pub fn is_defined(&self, name: &str) -> bool {
        // Phase 1: only macros + implicit commands. Function/symbol
        // tables come on-line in Phase 3+.
        self.macros.has(name) || is_implicit_command(name)
    }

    /// Whether `name` is *expandable* (i.e. would be substituted by
    /// `expand_once_inner(true)`).
    pub fn is_expandable(&self, name: &str) -> bool {
        match self.macros.get(name) {
            Some(MacroDefinition::Source(_)) | Some(MacroDefinition::Builtin(_)) => true,
            Some(MacroDefinition::Expansion(e)) => !e.unexpandable,
            None => false, // functions table not yet wired (Phase 3+)
        }
    }
}

fn parse_arg_index(text: &str) -> Option<usize> {
    if text.len() != 1 {
        return None;
    }
    let b = text.as_bytes()[0];
    if (b'1'..=b'9').contains(&b) {
        Some((b - b'0') as usize)
    } else {
        None
    }
}

fn is_implicit_command(name: &str) -> bool {
    matches!(name, "^" | "_" | "\\limits" | "\\nolimits")
}

#[derive(Clone, Debug)]
pub struct MacroArg {
    /// Argument tokens in **reverse** order (ready for the stack).
    pub tokens: Vec<Token>,
    pub start: Token,
    pub end: Token,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn mx<'s>(input: &str, settings: &'s Settings) -> MacroExpander<'s> {
        MacroExpander::new(input.to_string(), settings, Mode::Math)
    }

    fn drain(mx: &mut MacroExpander<'_>) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            let t = mx.expand_next_token().expect("expand");
            if t.text == "EOF" {
                break;
            }
            out.push(t.text.to_string());
        }
        out
    }

    #[test]
    fn passthrough_when_no_macros() {
        let s = Settings::default();
        let mut m = mx(r"a+b", &s);
        assert_eq!(drain(&mut m), vec!["a", "+", "b"]);
    }

    #[test]
    fn expands_simple_string_macro() {
        let s = Settings::builder().add_macro("\\RR", "\\mathbb{R}").build();
        let mut m = mx(r"\RR", &s);
        assert_eq!(drain(&mut m), vec!["\\mathbb", "{", "R", "}"]);
    }

    #[test]
    fn expands_macro_with_one_arg() {
        let s = Settings::builder().add_macro("\\twice", "{#1#1}").build();
        let mut m = mx(r"\twice{x}", &s);
        assert_eq!(drain(&mut m), vec!["{", "x", "x", "}"]);
    }

    #[test]
    fn expands_macro_with_two_args_in_correct_order() {
        let s = Settings::builder().add_macro("\\swap", "[#2,#1]").build();
        let mut m = mx(r"\swap{a}{b}", &s);
        assert_eq!(drain(&mut m), vec!["[", "b", ",", "a", "]"]);
    }

    #[test]
    fn double_hash_becomes_literal_hash() {
        // Upstream collapses `##` to `#` only inside the arg-substitution
        // pass — i.e., when the macro has at least one numbered argument.
        // `(##)#1` therefore expands to `(#)<arg1>`.
        let s = Settings::builder().add_macro("\\h", "(##)#1").build();
        let mut m = mx(r"\h{x}", &s);
        assert_eq!(drain(&mut m), vec!["(", "#", ")", "x"]);
    }

    #[test]
    fn macro_expansion_inside_group_is_local() {
        let s = Settings::builder().add_macro("\\x", "y").build();
        let mut m = mx(r"\x", &s);
        m.begin_group();
        // Inject a local override.
        m.macros.set(
            "\\x",
            Some(MacroDefinition::Source(SmolStr::new("z"))),
            false,
        );
        let t = m.expand_next_token().unwrap();
        assert_eq!(t.text, "z");
        m.end_group().unwrap();
        // Reset input and re-test the global macro.
        m.feed(r"\x".to_string());
        let t = m.expand_next_token().unwrap();
        assert_eq!(t.text, "y");
    }

    #[test]
    fn future_does_not_consume() {
        let s = Settings::default();
        let mut m = mx("ab", &s);
        let f1 = m.future().unwrap().text.clone();
        let f2 = m.future().unwrap().text.clone();
        assert_eq!(f1, "a");
        assert_eq!(f2, "a");
        let t = m.pop_token().unwrap();
        assert_eq!(t.text, "a");
        let t = m.pop_token().unwrap();
        assert_eq!(t.text, "b");
    }

    #[test]
    fn consume_spaces_skips_only_real_space_tokens() {
        let s = Settings::default();
        let mut m = mx("   a", &s);
        m.consume_spaces().unwrap();
        let t = m.pop_token().unwrap();
        assert_eq!(t.text, "a");
    }

    #[test]
    fn consume_arg_strips_outer_braces() {
        let s = Settings::default();
        let mut m = mx("{abc}", &s);
        let arg = m.consume_arg(None).unwrap();
        // Tokens come back reversed.
        let texts: Vec<_> = arg
            .tokens
            .iter()
            .rev()
            .map(|t| t.text.to_string())
            .collect();
        assert_eq!(texts, vec!["a", "b", "c"]);
    }

    #[test]
    fn consume_arg_undelimited_takes_one_token() {
        let s = Settings::default();
        let mut m = mx("xy", &s);
        let arg = m.consume_arg(None).unwrap();
        let texts: Vec<_> = arg.tokens.iter().map(|t| t.text.to_string()).collect();
        // Reversed: a single 'x'.
        assert_eq!(texts, vec!["x"]);
        let next = m.pop_token().unwrap();
        assert_eq!(next.text, "y");
    }

    #[test]
    fn consume_arg_extra_close_brace_errors() {
        let s = Settings::default();
        let mut m = mx("}", &s);
        let err = m.consume_arg(None).unwrap_err();
        assert!(err.raw_message.contains("Extra }"));
    }

    #[test]
    fn max_expand_caps_runaway_recursion() {
        // Direct self-reference would normally loop forever; max_expand
        // saves us. The default cap is 1000.
        let s = Settings::builder().add_macro("\\loop", "\\loop").build();
        let mut m = mx(r"\loop", &s);
        let err = m.expand_next_token().unwrap_err();
        assert!(err.raw_message.contains("Too many expansions"));
    }

    #[test]
    fn is_defined_includes_implicit_commands() {
        let s = Settings::default();
        let m = mx("", &s);
        assert!(m.is_defined("^"));
        assert!(m.is_defined("_"));
        assert!(m.is_defined("\\limits"));
        assert!(m.is_defined("\\nolimits"));
        assert!(!m.is_defined("\\undefined"));
    }

    #[test]
    fn expand_tokens_clears_treat_as_relax() {
        // Simulate a `\noexpand`-flagged token flowing through
        // expandTokens: the flags should be cleared but text preserved.
        // The macro must be defined — `\noexpand` only sets these flags
        // for expandable names, so the token is one that *is* defined,
        // we just don't want to expand it this round.
        let s = Settings::builder().add_macro("\\foo", "x").build();
        let mut m = mx("", &s);
        let mut t = Token::new("\\foo", None);
        t.noexpand = true;
        t.treat_as_relax = true;
        let out = m.expand_tokens(vec![t]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "\\foo");
        assert!(!out[0].noexpand);
        assert!(!out[0].treat_as_relax);
    }

    #[test]
    fn expand_next_token_rewrites_treat_as_relax_to_relax() {
        let s = Settings::default();
        let mut m = mx("", &s);
        let mut t = Token::new("\\foo", None);
        t.noexpand = true;
        t.treat_as_relax = true;
        m.push_token(t);
        let out = m.expand_next_token().unwrap();
        assert_eq!(out.text, "\\relax");
    }
}

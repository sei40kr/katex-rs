//! Parser — converts the macro-expanded token stream into a `Vec<ParseNode>`.
//!
//! Mirrors upstream KaTeX's `Parser.ts`. Method names and overall control
//! flow follow upstream so future fixes port mechanically.
//!
//! # Deviations from upstream
//!
//! - `next_token` is `Option<Token>` rather than upstream's `Token | null`.
//!   Upstream relies on a single null sentinel; Rust's `Option` carries
//!   the same information without the `null` ambiguity.
//! - The parser borrows `&'s Settings` rather than holding it by Arc — same
//!   model as [`crate::macro_expander::MacroExpander`].
//! - Strict-mode reporting (`reportNonstrict`) is intentionally elided here:
//!   the dispatch surface for it (functions/strict callbacks) lands in
//!   later phases. Cases where upstream calls `reportNonstrict` are
//!   tagged with a `// TODO(strict): ...` comment.
//! - `parseFunction` is fully wired even though Phase 4 ships an empty
//!   [`crate::functions::FUNCTIONS`] registry: the call always falls
//!   through to `parse_symbol`. Phase 5's function handlers light up
//!   immediately when the registry is populated, with no parser changes.
//! - `formLigatures` mutates the body in place; the Rust port likewise
//!   takes a `&mut Vec<ParseNode>`.

use std::sync::OnceLock;

use regex::Regex;
use smol_str::SmolStr;

use crate::define_function::{FunctionContext, FunctionSpec};
use crate::functions::FUNCTIONS;
use crate::lexer::split_trailing_combining_marks;
use crate::macro_expander::{MacroDefinition, MacroExpander};
use crate::parse_error::ParseError;
use crate::parse_node::ParseNode;
use crate::settings::Settings;
use crate::source_location::SourceLocation;
use crate::symbols::{Group, SYMBOLS};
use crate::token::Token;
use crate::tree::{ArgType, Atom, BreakToken, StyleStr};
use crate::types::Mode;
use crate::unicode_accents::UNICODE_ACCENTS;
use crate::unicode_scripts::supported_codepoint;
use crate::unicode_sup_or_sub::{U_SUBS_AND_SUPS, UNICODE_SUB_REGEX};
use crate::unicode_symbols::UNICODE_SYMBOLS;
use crate::units::{Measurement, Unit};

/// Tokens that always terminate `parse_expression`. Matches upstream's
/// static `Parser.endOfExpression`.
const END_OF_EXPRESSION: &[&str] = &["}", "\\endgroup", "\\end", "\\right", "&"];

/// Latin-1 / Unicode characters that are accepted as math-mode atoms but
/// trigger a `unicodeTextInMathMode` warning under strict mode. Mirrors
/// upstream's `extraLatin` set in `symbols.ts`.
const EXTRA_LATIN: &[&str] = &["Å", "Ç", "Ð", "Þ", "å", "ç", "ð", "þ"];

fn size_validate_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^[-+]? *(?:$|\d+|\d+\.\d*|\.\d*) *[a-z]{0,2} *$").expect("size validate")
    })
}

fn size_capture_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"([-+]?) *(\d+(?:\.\d*)?|\.\d+) *([a-z]{2})").expect("size capture")
    })
}

fn color_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)^(#[a-f0-9]{3,4}|#[a-f0-9]{6}|#[a-f0-9]{8}|[a-f0-9]{6}|[a-z]+)$")
            .expect("color")
    })
}

fn six_hex_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(?i)[0-9a-f]{6}$").expect("six hex"))
}

fn url_unescape_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\\([#$%&~_^{}])").expect("url unescape"))
}

fn verb_prefix_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\\verb[^a-zA-Z]").expect("verb prefix"))
}

fn unicode_sub_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(UNICODE_SUB_REGEX).expect("unicode sub"))
}

pub struct Parser<'s> {
    pub mode: Mode,
    pub gullet: MacroExpander<'s>,
    pub settings: &'s Settings,
    pub leftright_depth: u32,
    next_token: Option<Token>,
}

impl<'s> Parser<'s> {
    pub fn new(input: impl Into<std::sync::Arc<str>>, settings: &'s Settings) -> Self {
        let gullet = MacroExpander::new(input, settings, Mode::Math);
        Self {
            mode: Mode::Math,
            gullet,
            settings,
            leftright_depth: 0,
            next_token: None,
        }
    }

    /// Top-level entry. Parses the entire input.
    pub fn parse(&mut self) -> Result<Vec<ParseNode>, ParseError> {
        if !self.settings.global_group {
            self.gullet.begin_group();
        }
        if self.settings.color_is_text_color {
            self.gullet.macros.set(
                "\\color",
                Some(MacroDefinition::Source(SmolStr::new_static("\\textcolor"))),
                false,
            );
        }
        // Upstream wraps the body in try/finally that always calls
        // endGroups. We mirror that by ensuring endGroups runs whether the
        // body succeeded or not.
        let result = (|| -> Result<Vec<ParseNode>, ParseError> {
            let parsed = self.parse_expression(false, None)?;
            self.expect("EOF", true)?;
            if !self.settings.global_group {
                self.gullet.end_group()?;
            }
            Ok(parsed)
        })();
        // `end_groups` cleans up any group still on the stack regardless
        // of how parsing exited. Discard its error if `result` already
        // carries one.
        let _ = self.gullet.end_groups();
        result
    }

    /// Sub-parse a list of tokens (already in **reverse** order, as in a
    /// `MacroDefinition`). Mirrors upstream `subparse`.
    pub fn subparse(&mut self, tokens: Vec<Token>) -> Result<Vec<ParseNode>, ParseError> {
        let saved_next = self.next_token.take();
        // Push a `}` terminator first; the tokens follow on top of it so
        // they're popped in source order.
        self.gullet
            .push_token(Token::new(SmolStr::new_static("}"), None));
        self.gullet.push_tokens(tokens);
        let parsed = self.parse_expression(false, None)?;
        self.expect("}", true)?;
        self.next_token = saved_next;
        Ok(parsed)
    }

    /// Confirm that the lookahead is `text`, optionally consuming it.
    pub fn expect(&mut self, text: &str, consume: bool) -> Result<(), ParseError> {
        let tok = self.fetch()?.clone();
        if tok.text != text {
            let msg = format!("Expected '{}', got '{}'", text, tok.text);
            return Err(match tok.loc {
                Some(loc) => ParseError::with_location(msg, loc),
                None => ParseError::new(msg),
            });
        }
        if consume {
            self.consume();
        }
        Ok(())
    }

    /// Drop the cached lookahead.
    pub fn consume(&mut self) {
        self.next_token = None;
    }

    /// Return (or fetch) the cached lookahead token.
    pub fn fetch(&mut self) -> Result<&Token, ParseError> {
        if self.next_token.is_none() {
            let t = self.gullet.expand_next_token()?;
            self.next_token = Some(t);
        }
        Ok(self.next_token.as_ref().expect("just populated next_token"))
    }

    pub fn switch_mode(&mut self, new_mode: Mode) {
        self.mode = new_mode;
        self.gullet.switch_mode(new_mode);
    }

    /// Discard space tokens from the lookahead.
    pub fn consume_spaces(&mut self) -> Result<(), ParseError> {
        while self.fetch()?.text == " " {
            self.consume();
        }
        Ok(())
    }

    /// Parse an "expression" (a list of atoms) until a stop condition.
    pub fn parse_expression(
        &mut self,
        break_on_infix: bool,
        break_on_token_text: Option<BreakToken>,
    ) -> Result<Vec<ParseNode>, ParseError> {
        let mut body: Vec<ParseNode> = Vec::new();
        loop {
            if self.mode == Mode::Math {
                self.consume_spaces()?;
            }
            let lex_text = self.fetch()?.text.clone();
            if END_OF_EXPRESSION.iter().any(|t| *t == lex_text.as_str()) {
                break;
            }
            if let Some(brk) = break_on_token_text
                && lex_text.as_str() == brk.as_str()
            {
                break;
            }
            if break_on_infix
                && let Some(spec) = FUNCTIONS.get(lex_text.as_str())
                && spec.infix
            {
                break;
            }
            let atom = self.parse_atom(break_on_token_text)?;
            match atom {
                None => break,
                Some(ParseNode::Internal { .. }) => continue,
                Some(node) => body.push(node),
            }
        }
        if self.mode == Mode::Text {
            Self::form_ligatures(&mut body);
        }
        self.handle_infix_nodes(body)
    }

    /// Rewrite `infix` placeholder nodes (`\over`, …) into their `\frac`-
    /// shaped equivalent. Mirrors upstream `handleInfixNodes`.
    fn handle_infix_nodes(&mut self, body: Vec<ParseNode>) -> Result<Vec<ParseNode>, ParseError> {
        let mut over_index: Option<usize> = None;
        let mut func_name: Option<SmolStr> = None;
        for (i, node) in body.iter().enumerate() {
            if let ParseNode::Infix {
                replace_with,
                token,
                ..
            } = node
            {
                if over_index.is_some() {
                    let msg = "only one infix operator per group";
                    return Err(match token.as_ref().and_then(|t| t.loc.clone()) {
                        Some(loc) => ParseError::with_location(msg, loc),
                        None => ParseError::new(msg),
                    });
                }
                over_index = Some(i);
                func_name = Some(replace_with.clone());
            }
        }
        let (over_index, func_name) = match (over_index, func_name) {
            (Some(i), Some(n)) => (i, n),
            _ => return Ok(body),
        };

        let mut body = body;
        let denom_body: Vec<ParseNode> = body.split_off(over_index + 1);
        let infix_node = body.pop().expect("infix at over_index");
        let numer_body = body;

        let numer_node = unwrap_singleton_ordgroup(numer_body, self.mode);
        let denom_node = unwrap_singleton_ordgroup(denom_body, self.mode);

        let node = if func_name.as_str() == "\\\\abovefrac" {
            self.call_function(
                &func_name,
                vec![numer_node, infix_node, denom_node],
                vec![],
                None,
                None,
            )?
        } else {
            self.call_function(&func_name, vec![numer_node, denom_node], vec![], None, None)?
        };
        Ok(vec![node])
    }

    /// `^` and `_`: parse the argument that follows.
    fn handle_sup_subscript(&mut self, name: &str) -> Result<ParseNode, ParseError> {
        let symbol_token = self.fetch()?.clone();
        let symbol = symbol_token.text.clone();
        self.consume();
        self.consume_spaces()?;
        loop {
            let group = self.parse_group(name, None)?;
            match group {
                Some(ParseNode::Internal { .. }) => continue, // skip `\relax` etc.
                Some(other) => return Ok(other),
                None => {
                    let msg = format!("Expected group after '{}'", symbol);
                    return Err(match symbol_token.loc {
                        Some(loc) => ParseError::with_location(msg, loc),
                        None => ParseError::new(msg),
                    });
                }
            }
        }
    }

    /// Build the "unsupported command" placeholder that the parser emits
    /// when an undefined control sequence is encountered with
    /// `throw_on_error == false`. Mirrors upstream `formatUnsupportedCmd`.
    pub fn format_unsupported_cmd(&self, text: &str) -> ParseNode {
        let textord_array: Vec<ParseNode> = text
            .chars()
            .map(|c| ParseNode::TextOrd {
                mode: Mode::Text,
                loc: None,
                text: SmolStr::new(c.to_string()),
            })
            .collect();
        let text_node = ParseNode::Text {
            mode: self.mode,
            loc: None,
            body: textord_array,
            font: None,
        };
        ParseNode::Color {
            mode: self.mode,
            loc: None,
            color: SmolStr::new(self.settings.error_color.as_str()),
            body: vec![text_node],
        }
    }

    /// Parse one atom: a base group plus any super/subscripts.
    pub fn parse_atom(
        &mut self,
        break_on_token_text: Option<BreakToken>,
    ) -> Result<Option<ParseNode>, ParseError> {
        let mut base = self.parse_group("atom", break_on_token_text)?;

        if matches!(base, Some(ParseNode::Internal { .. })) {
            return Ok(base);
        }

        if self.mode == Mode::Text {
            return Ok(base);
        }

        let mut superscript: Option<ParseNode> = None;
        let mut subscript: Option<ParseNode> = None;
        loop {
            self.consume_spaces()?;
            let lex = self.fetch()?.clone();
            let text = lex.text.as_str();
            match text {
                "\\limits" | "\\nolimits" => {
                    let limits_val = text == "\\limits";
                    let recognised = match base.as_mut() {
                        Some(ParseNode::Op {
                            limits,
                            always_handle_supsub,
                            ..
                        }) => {
                            *limits = limits_val;
                            *always_handle_supsub = true;
                            true
                        }
                        Some(ParseNode::OperatorName {
                            always_handle_supsub,
                            limits,
                            ..
                        }) => {
                            if *always_handle_supsub {
                                *limits = limits_val;
                            }
                            true
                        }
                        _ => false,
                    };
                    if !recognised {
                        let msg = "Limit controls must follow a math operator";
                        return Err(match lex.loc {
                            Some(loc) => ParseError::with_location(msg, loc),
                            None => ParseError::new(msg),
                        });
                    }
                    self.consume();
                }
                "^" => {
                    if superscript.is_some() {
                        return Err(double_script_err("Double superscript", &lex));
                    }
                    superscript = Some(self.handle_sup_subscript("superscript")?);
                }
                "_" => {
                    if subscript.is_some() {
                        return Err(double_script_err("Double subscript", &lex));
                    }
                    subscript = Some(self.handle_sup_subscript("subscript")?);
                }
                "'" => {
                    if superscript.is_some() {
                        return Err(double_script_err("Double superscript", &lex));
                    }
                    let prime = ParseNode::TextOrd {
                        mode: self.mode,
                        loc: None,
                        text: SmolStr::new_static("\\prime"),
                    };
                    let mut primes: Vec<ParseNode> = vec![prime.clone()];
                    self.consume();
                    while self.fetch()?.text == "'" {
                        primes.push(prime.clone());
                        self.consume();
                    }
                    if self.fetch()?.text == "^" {
                        primes.push(self.handle_sup_subscript("superscript")?);
                    }
                    superscript = Some(ParseNode::OrdGroup {
                        mode: self.mode,
                        loc: None,
                        body: primes,
                        semisimple: false,
                    });
                }
                _ if U_SUBS_AND_SUPS
                    .get(&text.chars().next().unwrap_or('\0'))
                    .is_some()
                    && text.chars().count() == 1 =>
                {
                    let ch = text.chars().next().unwrap();
                    let is_sub = unicode_sub_re().is_match(text);
                    let mut subsup_tokens: Vec<Token> =
                        vec![Token::new(*U_SUBS_AND_SUPS.get(&ch).unwrap(), None)];
                    self.consume();
                    loop {
                        let next_text = self.fetch()?.text.clone();
                        let next_char = match next_text.chars().next() {
                            Some(c) if next_text.chars().count() == 1 => c,
                            _ => break,
                        };
                        let Some(mapped) = U_SUBS_AND_SUPS.get(&next_char) else {
                            break;
                        };
                        if unicode_sub_re().is_match(next_text.as_str()) != is_sub {
                            break;
                        }
                        // Build the token list in source order; we reverse
                        // before handing to subparse.
                        subsup_tokens.insert(0, Token::new(*mapped, None));
                        self.consume();
                    }
                    let body = self.subparse(subsup_tokens)?;
                    let ord = ParseNode::OrdGroup {
                        mode: Mode::Math,
                        loc: None,
                        body,
                        semisimple: false,
                    };
                    if is_sub {
                        subscript = Some(ord);
                    } else {
                        superscript = Some(ord);
                    }
                }
                _ => break,
            }
        }

        if superscript.is_some() || subscript.is_some() {
            Ok(Some(ParseNode::SupSub {
                mode: self.mode,
                loc: None,
                base: base.map(Box::new),
                sup: superscript.map(Box::new),
                sub: subscript.map(Box::new),
            }))
        } else {
            Ok(base)
        }
    }

    /// Parse a function: command token + its arguments. Returns `None` if
    /// the lookahead isn't a known function.
    pub fn parse_function(
        &mut self,
        break_on_token_text: Option<BreakToken>,
        name: Option<&str>,
    ) -> Result<Option<ParseNode>, ParseError> {
        let token = self.fetch()?.clone();
        let func = token.text.clone();
        let func_data = match FUNCTIONS.get(func.as_str()) {
            Some(spec) => spec,
            None => return Ok(None),
        };
        self.consume();

        if let Some(n) = name
            && n != "atom"
            && !func_data.allowed_in_argument
        {
            let msg = format!("Got function '{}' with no arguments as {}", func, n);
            return Err(match token.loc {
                Some(loc) => ParseError::with_location(msg, loc),
                None => ParseError::new(msg),
            });
        } else if self.mode == Mode::Text && !func_data.allowed_in_text {
            let msg = format!("Can't use function '{}' in text mode", func);
            return Err(match token.loc {
                Some(loc) => ParseError::with_location(msg, loc),
                None => ParseError::new(msg),
            });
        } else if self.mode == Mode::Math && !func_data.allowed_in_math {
            let msg = format!("Can't use function '{}' in math mode", func);
            return Err(match token.loc {
                Some(loc) => ParseError::with_location(msg, loc),
                None => ParseError::new(msg),
            });
        }

        let (args, opt_args) = self.parse_arguments(&func, func_data)?;
        Ok(Some(self.call_function(
            &func,
            args,
            opt_args,
            Some(token),
            break_on_token_text,
        )?))
    }

    /// Invoke the registered handler for `name`. Errors if the function
    /// has no parser-side handler (i.e. is renderer-only).
    pub fn call_function(
        &mut self,
        name: &str,
        args: Vec<ParseNode>,
        opt_args: Vec<Option<ParseNode>>,
        token: Option<Token>,
        break_on_token_text: Option<BreakToken>,
    ) -> Result<ParseNode, ParseError> {
        let spec = FUNCTIONS
            .get(name)
            .ok_or_else(|| ParseError::new(format!("No function handler for {}", name)))?;
        let handler = spec
            .handler
            .ok_or_else(|| ParseError::new(format!("No function handler for {}", name)))?;
        let func_name = SmolStr::new(name);
        let ctx = FunctionContext {
            func_name,
            parser: self,
            token,
            break_on_token_text,
        };
        handler(ctx, &args, &opt_args)
    }

    /// Pull mandatory + optional arguments off the gullet according to a
    /// function/environment spec. Mirrors upstream `parseArguments`.
    fn parse_arguments(
        &mut self,
        func: &str,
        func_data: &FunctionSpec,
    ) -> Result<(Vec<ParseNode>, Vec<Option<ParseNode>>), ParseError> {
        let total = func_data.num_args + func_data.num_optional_args;
        if total == 0 {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut args: Vec<ParseNode> = Vec::with_capacity(func_data.num_args);
        let mut opt_args: Vec<Option<ParseNode>> = Vec::with_capacity(func_data.num_optional_args);
        for i in 0..total {
            let mut arg_type = func_data.arg_types.get(i).copied();
            let is_optional = i < func_data.num_optional_args;
            if (func_data.primitive && arg_type.is_none())
                || (func_data.node_type == crate::parse_node::NodeType::Sqrt
                    && i == 1
                    && matches!(opt_args.first(), Some(None)))
            {
                arg_type = Some(ArgType::Primitive);
            }
            let name = format!("argument to '{}'", func);
            let arg = self.parse_group_of_type(&name, arg_type, is_optional)?;
            if is_optional {
                opt_args.push(arg);
            } else if let Some(node) = arg {
                args.push(node);
            } else {
                return Err(ParseError::new(
                    "Null argument, please report this as a bug",
                ));
            }
        }
        Ok((args, opt_args))
    }

    /// Parse an environment-spec argument. Convenience wrapper around
    /// [`Parser::parse_group_of_type`] used by the `\begin` dispatcher
    /// in [`crate::functions::environment`]. Mirrors upstream's
    /// per-argument call inside `parseFunction` for environments.
    pub fn parse_arg_for_environment(
        &mut self,
        arg_type: Option<ArgType>,
        optional: bool,
        env_name: &str,
    ) -> Result<Option<ParseNode>, ParseError> {
        let label = format!("argument to environment '{env_name}'");
        self.parse_group_of_type(&label, arg_type, optional)
    }

    /// Parse the `{name}` group that follows `\end`. Mirrors the
    /// `\end` argument-parse done implicitly by `parseFunction`
    /// upstream.
    pub fn parse_environment_name_group(&mut self) -> Result<ParseNode, ParseError> {
        match self.parse_group_of_type(
            "environment name",
            Some(ArgType::Mode(Mode::Text)),
            false,
        )? {
            Some(g) => Ok(g),
            None => Err(ParseError::new("Expected environment name after \\end")),
        }
    }

    /// Parse a group with the typed-argument flow. Mirrors upstream
    /// `parseGroupOfType`.
    fn parse_group_of_type(
        &mut self,
        name: &str,
        arg_type: Option<ArgType>,
        optional: bool,
    ) -> Result<Option<ParseNode>, ParseError> {
        match arg_type {
            Some(ArgType::Color) => self.parse_color_group(optional),
            Some(ArgType::Size) => self.parse_size_group(optional),
            Some(ArgType::Url) => self.parse_url_group(optional),
            Some(ArgType::Mode(m)) => self.parse_argument_group(optional, Some(m)),
            Some(ArgType::Hbox) => {
                let group = self.parse_argument_group(optional, Some(Mode::Text))?;
                Ok(group.map(|g| {
                    let mode = g.mode();
                    ParseNode::Styling {
                        mode,
                        loc: None,
                        style: StyleStr::Text,
                        body: vec![g],
                    }
                }))
            }
            Some(ArgType::Raw) => {
                let token = self.parse_string_group("raw", optional)?;
                Ok(token.map(|t| ParseNode::Raw {
                    mode: Mode::Text,
                    loc: None,
                    string: t.text.to_string(),
                }))
            }
            Some(ArgType::Primitive) => {
                if optional {
                    return Err(ParseError::new("A primitive argument cannot be optional"));
                }
                match self.parse_group(name, None)? {
                    Some(g) => Ok(Some(g)),
                    None => {
                        let tok = self.fetch()?.clone();
                        Err(match tok.loc {
                            Some(loc) => ParseError::with_location(
                                format!("Expected group as {}", name),
                                loc,
                            ),
                            None => ParseError::new(format!("Expected group as {}", name)),
                        })
                    }
                }
            }
            Some(ArgType::Original) | None => self.parse_argument_group(optional, None),
        }
    }

    /// Parse a delimited string group (`{...}` or `[...]`), returning a
    /// synthetic Token whose text is the concatenation of all inner tokens.
    /// Mirrors upstream `parseStringGroup`.
    fn parse_string_group(
        &mut self,
        _mode_name: &str,
        optional: bool,
    ) -> Result<Option<Token>, ParseError> {
        let mut arg_token = match self.gullet.scan_argument(optional)? {
            Some(t) => t,
            None => return Ok(None),
        };
        let mut buf = String::new();
        loop {
            let tok = self.fetch()?.clone();
            if tok.text == "EOF" {
                break;
            }
            buf.push_str(tok.text.as_str());
            self.consume();
        }
        // Consume the synthetic EOF terminator scan_argument pushed.
        self.consume();
        arg_token.text = SmolStr::new(buf);
        Ok(Some(arg_token))
    }

    /// Parse the largest token sequence whose concatenated text matches
    /// `re`. Mirrors upstream `parseRegexGroup`.
    fn parse_regex_group(&mut self, re: &Regex, mode_name: &str) -> Result<Token, ParseError> {
        let first = self.fetch()?.clone();
        let mut last = first.clone();
        let mut buf = String::new();
        loop {
            let tok = self.fetch()?.clone();
            if tok.text == "EOF" {
                break;
            }
            let mut probe = buf.clone();
            probe.push_str(tok.text.as_str());
            if !re.is_match(&probe) {
                break;
            }
            last = tok.clone();
            buf = probe;
            self.consume();
        }
        if buf.is_empty() {
            let msg = format!("Invalid {}: '{}'", mode_name, first.text);
            return Err(match first.loc {
                Some(loc) => ParseError::with_location(msg, loc),
                None => ParseError::new(msg),
            });
        }
        Ok(first.range(&last, buf))
    }

    fn parse_color_group(&mut self, optional: bool) -> Result<Option<ParseNode>, ParseError> {
        let res = match self.parse_string_group("color", optional)? {
            Some(t) => t,
            None => return Ok(None),
        };
        let captures = match color_re().captures(res.text.as_str()) {
            Some(c) => c,
            None => {
                let msg = format!("Invalid color: '{}'", res.text);
                return Err(match res.loc {
                    Some(loc) => ParseError::with_location(msg, loc),
                    None => ParseError::new(msg),
                });
            }
        };
        let mut color = captures.get(0).expect("group 0").as_str().to_string();
        if six_hex_re().is_match(&color) {
            color.insert(0, '#');
        }
        Ok(Some(ParseNode::ColorToken {
            mode: self.mode,
            loc: None,
            color: SmolStr::new(color),
        }))
    }

    pub fn parse_size_group(&mut self, optional: bool) -> Result<Option<ParseNode>, ParseError> {
        // Don't expand before parseStringGroup.
        self.gullet.consume_spaces()?;
        let mut is_blank = false;
        let res: Option<Token> = if !optional && self.gullet.future()?.text != "{" {
            Some(self.parse_regex_group(size_validate_re(), "size")?)
        } else {
            self.parse_string_group("size", optional)?
        };
        let mut res = match res {
            Some(t) => t,
            None => return Ok(None),
        };
        if !optional && res.text.is_empty() {
            res.text = SmolStr::new_static("0pt");
            is_blank = true;
        }
        let captures = match size_capture_re().captures(res.text.as_str()) {
            Some(c) => c,
            None => {
                let msg = format!("Invalid size: '{}'", res.text);
                return Err(match res.loc {
                    Some(loc) => ParseError::with_location(msg, loc),
                    None => ParseError::new(msg),
                });
            }
        };
        let sign = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let magnitude = captures.get(2).expect("magnitude").as_str();
        let unit_str = captures.get(3).expect("unit").as_str();
        let number_text = format!("{}{}", sign, magnitude);
        let number: f64 = number_text.parse().unwrap_or(0.0);
        let unit = match Unit::from_str_or_err(unit_str) {
            Ok(u) => u,
            Err(_) => {
                let msg = format!("Invalid unit: '{}'", unit_str);
                return Err(match res.loc {
                    Some(loc) => ParseError::with_location(msg, loc),
                    None => ParseError::new(msg),
                });
            }
        };
        Ok(Some(ParseNode::Size {
            mode: self.mode,
            loc: None,
            value: Measurement { number, unit },
            is_blank,
        }))
    }

    fn parse_url_group(&mut self, optional: bool) -> Result<Option<ParseNode>, ParseError> {
        // Hyperref-style: `%` becomes active so it isn't stripped as a comment.
        self.gullet.lexer_mut().set_catcode("%", 13);
        self.gullet.lexer_mut().set_catcode("~", 12);
        let res = self.parse_string_group("url", optional)?;
        self.gullet.lexer_mut().set_catcode("%", 14);
        self.gullet.lexer_mut().set_catcode("~", 13);
        let res = match res {
            Some(t) => t,
            None => return Ok(None),
        };
        let url = url_unescape_re()
            .replace_all(res.text.as_str(), "$1")
            .into_owned();
        Ok(Some(ParseNode::Url {
            mode: self.mode,
            loc: None,
            url,
        }))
    }

    fn parse_argument_group(
        &mut self,
        optional: bool,
        mode: Option<Mode>,
    ) -> Result<Option<ParseNode>, ParseError> {
        let arg_token = match self.gullet.scan_argument(optional)? {
            Some(t) => t,
            None => return Ok(None),
        };
        let outer_mode = self.mode;
        if let Some(m) = mode {
            self.switch_mode(m);
        }
        self.gullet.begin_group();
        let expression = self.parse_expression(false, Some(BreakToken::Eof))?;
        self.expect("EOF", true)?;
        self.gullet.end_group()?;
        let node = ParseNode::OrdGroup {
            mode: self.mode,
            loc: arg_token.loc,
            body: expression,
            semisimple: false,
        };
        if mode.is_some() {
            self.switch_mode(outer_mode);
        }
        Ok(Some(node))
    }

    /// Parse a `{...}` group, a `\begingroup ... \endgroup` group, or a
    /// nucleus / function call.
    pub fn parse_group(
        &mut self,
        name: &str,
        break_on_token_text: Option<BreakToken>,
    ) -> Result<Option<ParseNode>, ParseError> {
        let first_token = self.fetch()?.clone();
        let text = first_token.text.clone();
        if text == "{" || text == "\\begingroup" {
            self.consume();
            let group_end = if text == "{" { "}" } else { "\\endgroup" };
            self.gullet.begin_group();
            let break_on = if text == "{" {
                Some(BreakToken::CloseBrace)
            } else {
                Some(BreakToken::EndGroup)
            };
            let expression = self.parse_expression(false, break_on)?;
            let last_token = self.fetch()?.clone();
            self.expect(group_end, true)?;
            self.gullet.end_group()?;
            let loc = SourceLocation::range(first_token.loc.as_ref(), last_token.loc.as_ref());
            return Ok(Some(ParseNode::OrdGroup {
                mode: self.mode,
                loc,
                body: expression,
                semisimple: text == "\\begingroup",
            }));
        }
        // Otherwise: a function call or nucleus.
        if let Some(node) = self.parse_function(break_on_token_text, Some(name))? {
            return Ok(Some(node));
        }
        let symbol = self.parse_symbol()?;
        if symbol.is_none() && text.starts_with('\\') && !is_implicit_command(text.as_str()) {
            if self.settings.throw_on_error {
                let msg = format!("Undefined control sequence: {}", text);
                return Err(match first_token.loc {
                    Some(loc) => ParseError::with_location(msg, loc),
                    None => ParseError::new(msg),
                });
            }
            let unsupported = self.format_unsupported_cmd(text.as_str());
            self.consume();
            return Ok(Some(unsupported));
        }
        Ok(symbol)
    }

    /// In-place text-mode ligature folding: `--` → en-dash text, `---` →
    /// em-dash text, and `''` / `\`\`` → smart-quote pairs.
    fn form_ligatures(group: &mut Vec<ParseNode>) {
        let mut i = 0;
        while i + 1 < group.len() {
            let a_text = match &group[i] {
                ParseNode::TextOrd { text, .. } => Some(text.clone()),
                _ => None,
            };
            let Some(a_text) = a_text else {
                i += 1;
                continue;
            };
            let next_text = match &group[i + 1] {
                ParseNode::TextOrd { text, .. } => Some(text.clone()),
                _ => None,
            };
            let Some(next_text) = next_text else {
                i += 1;
                continue;
            };
            if a_text == "-" && next_text == "-" {
                let after_text = group.get(i + 2).and_then(|n| match n {
                    ParseNode::TextOrd { text, .. } => Some(text.clone()),
                    _ => None,
                });
                if let Some(at) = after_text
                    && at == "-"
                {
                    let loc_a = node_loc(&group[i]).cloned();
                    let loc_c = node_loc(&group[i + 2]).cloned();
                    let merged_loc = SourceLocation::range(loc_a.as_ref(), loc_c.as_ref());
                    let replacement = ParseNode::TextOrd {
                        mode: Mode::Text,
                        loc: merged_loc,
                        text: SmolStr::new_static("---"),
                    };
                    group.splice(i..=i + 2, std::iter::once(replacement));
                } else {
                    let loc_a = node_loc(&group[i]).cloned();
                    let loc_b = node_loc(&group[i + 1]).cloned();
                    let merged_loc = SourceLocation::range(loc_a.as_ref(), loc_b.as_ref());
                    let replacement = ParseNode::TextOrd {
                        mode: Mode::Text,
                        loc: merged_loc,
                        text: SmolStr::new_static("--"),
                    };
                    group.splice(i..=i + 1, std::iter::once(replacement));
                }
            } else if (a_text == "'" || a_text == "`") && next_text == a_text {
                let loc_a = node_loc(&group[i]).cloned();
                let loc_b = node_loc(&group[i + 1]).cloned();
                let merged_loc = SourceLocation::range(loc_a.as_ref(), loc_b.as_ref());
                let mut joined = String::with_capacity(a_text.len() * 2);
                joined.push_str(&a_text);
                joined.push_str(&next_text);
                let replacement = ParseNode::TextOrd {
                    mode: Mode::Text,
                    loc: merged_loc,
                    text: SmolStr::new(joined),
                };
                group.splice(i..=i + 1, std::iter::once(replacement));
            }
            i += 1;
        }
    }

    /// Parse one symbol — a single character (possibly with combining
    /// accents) or a `\verb`-form literal.
    pub fn parse_symbol(&mut self) -> Result<Option<ParseNode>, ParseError> {
        let nucleus = self.fetch()?.clone();
        let mut text: SmolStr = nucleus.text.clone();

        if verb_prefix_re().is_match(text.as_str()) {
            self.consume();
            let mut arg = &text.as_str()[5..];
            let star = arg.starts_with('*');
            if star {
                arg = &arg[1..];
            }
            // Lexer guarantees matching delimiters.
            if arg.len() < 2 {
                return Err(ParseError::new(
                    "\\verb assertion failed --\n                    please report what input caused this bug",
                ));
            }
            let first = arg.chars().next().unwrap();
            let last = arg.chars().next_back().unwrap();
            if first != last {
                return Err(ParseError::new(
                    "\\verb assertion failed --\n                    please report what input caused this bug",
                ));
            }
            // Strip first and last character.
            let body = &arg[first.len_utf8()..arg.len() - last.len_utf8()];
            return Ok(Some(ParseNode::Verb {
                mode: Mode::Text,
                loc: None,
                body: body.to_string(),
                star,
            }));
        }

        // First-codepoint lookup against unicodeSymbols (precomposed
        // accented letters); rewrite text so the rest of the function sees
        // the LaTeX-source equivalent.
        let first_char = text.chars().next().unwrap_or('\0');
        if first_char != '\0'
            && let Some(expansion) = UNICODE_SYMBOLS.get(&first_char)
        {
            let single = first_char.to_string();
            if SYMBOLS.get((self.mode, single.as_str())).is_none() {
                // TODO(strict): reportNonstrict("unicodeTextInMathMode") in math mode.
                let mut new_text = String::with_capacity(text.len() + expansion.len());
                new_text.push_str(expansion);
                new_text.push_str(&text[first_char.len_utf8()..]);
                text = SmolStr::new(new_text);
            }
        }

        // Strip trailing combining marks before symbol lookup; we'll apply
        // them as `accent` wrappers afterwards.
        let combining = split_trailing_combining_marks(text.as_str())
            .map(|(idx, marks)| (idx, marks.to_string()));
        if let Some((idx, _)) = &combining {
            let base = &text[..*idx];
            text = if base == "i" {
                SmolStr::new_static("\u{0131}")
            } else if base == "j" {
                SmolStr::new_static("\u{0237}")
            } else {
                SmolStr::new(base)
            };
        }

        // Symbol-table lookup.
        let mut symbol: ParseNode;
        if let Some(info) = SYMBOLS.get((self.mode, text.as_str())) {
            // TODO(strict): reportNonstrict("unicodeTextInMathMode") for
            // EXTRA_LATIN in math mode.
            let _ = EXTRA_LATIN; // referenced once strict reporting lands.
            let loc = nucleus.loc.clone();
            symbol = match info.group {
                Group::Bin => ParseNode::Atom {
                    family: Atom::Bin,
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::Close => ParseNode::Atom {
                    family: Atom::Close,
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::Inner => ParseNode::Atom {
                    family: Atom::Inner,
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::Open => ParseNode::Atom {
                    family: Atom::Open,
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::Punct => ParseNode::Atom {
                    family: Atom::Punct,
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::Rel => ParseNode::Atom {
                    family: Atom::Rel,
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::AccentToken => ParseNode::AccentToken {
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::MathOrd => ParseNode::MathOrd {
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::OpToken => ParseNode::OpToken {
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::Spacing => ParseNode::Spacing {
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
                Group::TextOrd => ParseNode::TextOrd {
                    mode: self.mode,
                    loc,
                    text: text.clone(),
                },
            };
        } else if first_char as u32 >= 0x80 {
            if self.settings.strict != crate::settings::StrictMode::Disabled {
                if !supported_codepoint(first_char) {
                    // TODO(strict): reportNonstrict("unknownSymbol", ...)
                } else if self.mode == Mode::Math {
                    // TODO(strict): reportNonstrict("unicodeTextInMathMode", ...)
                }
            }
            symbol = ParseNode::TextOrd {
                mode: Mode::Text,
                loc: nucleus.loc.clone(),
                text: text.clone(),
            };
        } else {
            return Ok(None);
        }
        self.consume();

        if let Some((_, marks)) = combining {
            for accent_char in marks.chars() {
                let entry = UNICODE_ACCENTS
                    .get(&accent_char)
                    .ok_or_else(|| ParseError::new(format!("Unknown accent ' {}'", accent_char)))?;
                let command = match self.mode {
                    Mode::Math => entry.math.unwrap_or(entry.text),
                    Mode::Text => entry.text,
                };
                if command.is_empty() {
                    return Err(ParseError::new(format!(
                        "Accent {} unsupported in {:?} mode",
                        accent_char, self.mode
                    )));
                }
                symbol = ParseNode::Accent {
                    mode: self.mode,
                    loc: nucleus.loc.clone(),
                    label: SmolStr::new(command),
                    is_stretchy: false,
                    is_shifty: true,
                    base: Box::new(symbol),
                };
            }
        }

        Ok(Some(symbol))
    }
}

fn unwrap_singleton_ordgroup(mut body: Vec<ParseNode>, mode: Mode) -> ParseNode {
    if body.len() == 1 && matches!(&body[0], ParseNode::OrdGroup { .. }) {
        body.remove(0)
    } else {
        ParseNode::OrdGroup {
            mode,
            loc: None,
            body,
            semisimple: false,
        }
    }
}

fn double_script_err(msg: &str, lex: &Token) -> ParseError {
    match lex.loc.clone() {
        Some(loc) => ParseError::with_location(msg, loc),
        None => ParseError::new(msg),
    }
}

fn node_loc(n: &ParseNode) -> Option<&SourceLocation> {
    n.loc()
}

fn is_implicit_command(name: &str) -> bool {
    matches!(name, "^" | "_" | "\\limits" | "\\nolimits")
}

// Local helper: `Unit::from_str` lives behind `std::str::FromStr`. Wrap it
// so the parser doesn't need to import the trait at every call site.
trait UnitFromStrOrErr: Sized {
    fn from_str_or_err(s: &str) -> Result<Self, ParseError>;
}

impl UnitFromStrOrErr for Unit {
    fn from_str_or_err(s: &str) -> Result<Self, ParseError> {
        <Unit as std::str::FromStr>::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::parse_node::OpBody;

    fn parse(input: &str) -> Result<Vec<ParseNode>, ParseError> {
        let settings = Settings::default();
        let mut p = Parser::new(input.to_string(), &settings);
        // `parse` borrows from the same `settings`, but since we only own
        // it here for the duration of the test, store a separate binding.
        let _ = &settings;
        p.parse()
    }

    fn parse_with(input: &str, settings: &Settings) -> Result<Vec<ParseNode>, ParseError> {
        let mut p = Parser::new(input.to_string(), settings);
        p.parse()
    }

    #[test]
    fn alpha_resolves_to_mathord() {
        let body = parse(r"\alpha").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::MathOrd { text, mode, .. } => {
                assert_eq!(text.as_str(), "\\alpha");
                assert_eq!(*mode, Mode::Math);
            }
            other => panic!("expected mathord, got {:?}", other.node_type()),
        }
    }

    #[test]
    fn one_plus_two_three_atoms() {
        let body = parse("1+2").unwrap();
        assert_eq!(body.len(), 3);
        // "1" is textord (Group::TextOrd), "+" is atom (Bin), "2" is textord.
        match &body[0] {
            ParseNode::TextOrd { text, .. } => assert_eq!(text.as_str(), "1"),
            other => panic!("expected textord, got {:?}", other.node_type()),
        }
        match &body[1] {
            ParseNode::Atom { family, text, .. } => {
                assert_eq!(*family, Atom::Bin);
                assert_eq!(text.as_str(), "+");
            }
            other => panic!("expected atom, got {:?}", other.node_type()),
        }
        match &body[2] {
            ParseNode::TextOrd { text, .. } => assert_eq!(text.as_str(), "2"),
            other => panic!("expected textord, got {:?}", other.node_type()),
        }
    }

    #[test]
    fn supsub_x_pow_2_sub_i() {
        let body = parse("x^2_i").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::SupSub { base, sup, sub, .. } => {
                let base = base.as_deref().expect("base");
                assert!(matches!(base, ParseNode::MathOrd { .. }));
                let sup = sup.as_deref().expect("sup");
                match sup {
                    ParseNode::TextOrd { text, .. } => assert_eq!(text.as_str(), "2"),
                    _ => panic!("expected textord sup"),
                }
                let sub = sub.as_deref().expect("sub");
                match sub {
                    ParseNode::MathOrd { text, .. } => assert_eq!(text.as_str(), "i"),
                    _ => panic!("expected mathord sub"),
                }
            }
            _ => panic!("expected supsub"),
        }
    }

    #[test]
    fn primes_become_ordgroup_with_prime_textords() {
        let body = parse("x''").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::SupSub { base, sup, .. } => {
                assert!(matches!(base.as_deref(), Some(ParseNode::MathOrd { .. })));
                let sup = sup.as_deref().expect("sup");
                match sup {
                    ParseNode::OrdGroup { body, .. } => {
                        assert_eq!(body.len(), 2);
                        for n in body {
                            match n {
                                ParseNode::TextOrd { text, .. } => {
                                    assert_eq!(text.as_str(), "\\prime");
                                }
                                _ => panic!("expected textord prime"),
                            }
                        }
                    }
                    _ => panic!("expected ordgroup of primes"),
                }
            }
            _ => panic!("expected supsub"),
        }
    }

    #[test]
    fn brace_group_yields_ordgroup() {
        let body = parse("{ab}").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::OrdGroup {
                body, semisimple, ..
            } => {
                assert!(!*semisimple);
                assert_eq!(body.len(), 2);
            }
            _ => panic!("expected ordgroup"),
        }
    }

    #[test]
    fn begingroup_endgroup_is_semisimple() {
        let body = parse("\\begingroup x\\endgroup").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::OrdGroup {
                semisimple, body, ..
            } => {
                assert!(*semisimple);
                // x — a single mathord.
                assert_eq!(body.len(), 1);
                assert!(matches!(&body[0], ParseNode::MathOrd { .. }));
            }
            _ => panic!("expected ordgroup"),
        }
    }

    #[test]
    fn double_superscript_errors() {
        let err = parse("x^2^3").unwrap_err();
        assert!(err.raw_message.contains("Double superscript"));
    }

    #[test]
    fn double_subscript_errors() {
        let err = parse("x_1_2").unwrap_err();
        assert!(err.raw_message.contains("Double subscript"));
    }

    #[test]
    fn undefined_control_sequence_errors_in_throw_mode() {
        let err = parse(r"\notacommand").unwrap_err();
        assert!(err.raw_message.contains("Undefined control sequence"));
    }

    #[test]
    fn undefined_control_sequence_becomes_unsupported_cmd_when_not_throwing() {
        let s = Settings::builder().throw_on_error(false).build();
        let body = parse_with(r"\notacommand", &s).unwrap();
        // `\notacommand` becomes a color-wrapped text node.
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::Color { color, body, .. } => {
                assert_eq!(color.as_str(), "#cc0000");
                assert_eq!(body.len(), 1);
                match &body[0] {
                    ParseNode::Text { body, .. } => {
                        // `\notacommand` is 12 characters.
                        assert_eq!(body.len(), 12);
                        for n in body {
                            assert!(matches!(n, ParseNode::TextOrd { .. }));
                        }
                    }
                    _ => panic!("expected text node"),
                }
            }
            _ => panic!("expected color node"),
        }
    }

    #[test]
    fn implicit_caret_at_eof_is_not_undefined_control_sequence() {
        // Bare `^` with no base produces a supsub with empty base when
        // followed by a target. With nothing following, parseGroup at the
        // sup target raises "Expected group after '^'".
        let err = parse("^").unwrap_err();
        assert!(err.raw_message.contains("Expected group"));
    }

    #[test]
    fn macro_expansion_feeds_parser() {
        let s = Settings::builder().add_macro("\\RR", "\\alpha").build();
        let body = parse_with(r"\RR", &s).unwrap();
        assert_eq!(body.len(), 1);
        assert!(matches!(&body[0], ParseNode::MathOrd { text, .. } if text.as_str() == "\\alpha"));
    }

    #[test]
    fn verb_literal_captures_body() {
        // `\verb|abc|` ends up as a verb node with body "abc".
        let body = parse(r"\verb|abc|").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::Verb { body, star, .. } => {
                assert_eq!(body, "abc");
                assert!(!*star);
            }
            _ => panic!("expected verb"),
        }
    }

    #[test]
    fn empty_input_parses_to_empty_body() {
        assert!(parse("").unwrap().is_empty());
    }

    #[test]
    fn space_in_math_mode_is_consumed() {
        // Math mode strips whitespace tokens.
        let body = parse(" x ").unwrap();
        assert_eq!(body.len(), 1);
        assert!(matches!(&body[0], ParseNode::MathOrd { .. }));
    }

    // Phase 5 integration tests — exercise the function dispatch end to end
    // on a representative cross-section of registered handlers.

    #[test]
    fn frac_builds_genfrac_node() {
        let body = parse(r"\frac{1}{2}").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::GenFrac {
                has_bar_line,
                left_delim,
                right_delim,
                ..
            } => {
                assert!(*has_bar_line);
                assert!(left_delim.is_none());
                assert!(right_delim.is_none());
            }
            other => panic!("expected genfrac, got {:?}", other.node_type()),
        }
    }

    #[test]
    fn binom_sets_paren_delims() {
        let body = parse(r"\binom{n}{k}").unwrap();
        match &body[0] {
            ParseNode::GenFrac {
                left_delim,
                right_delim,
                has_bar_line,
                ..
            } => {
                assert_eq!(left_delim.as_deref(), Some("("));
                assert_eq!(right_delim.as_deref(), Some(")"));
                assert!(!*has_bar_line);
            }
            other => panic!("expected genfrac, got {:?}", other.node_type()),
        }
    }

    #[test]
    fn dfrac_wraps_in_styling_display() {
        let body = parse(r"\dfrac{1}{2}").unwrap();
        match &body[0] {
            ParseNode::Styling { style, body, .. } => {
                assert_eq!(*style, StyleStr::Display);
                assert!(matches!(body[0], ParseNode::GenFrac { .. }));
            }
            other => panic!("expected styling wrapper, got {:?}", other.node_type()),
        }
    }

    #[test]
    fn sqrt_with_optional_index() {
        let body = parse(r"\sqrt[3]{x}").unwrap();
        match &body[0] {
            ParseNode::Sqrt { index, .. } => {
                assert!(index.is_some());
            }
            _ => panic!("expected sqrt"),
        }
    }

    #[test]
    fn sqrt_without_index() {
        let body = parse(r"\sqrt{x}").unwrap();
        match &body[0] {
            ParseNode::Sqrt { index, .. } => assert!(index.is_none()),
            _ => panic!("expected sqrt"),
        }
    }

    #[test]
    fn hat_builds_accent_node() {
        let body = parse(r"\hat{x}").unwrap();
        match &body[0] {
            ParseNode::Accent {
                label,
                is_stretchy,
                is_shifty,
                ..
            } => {
                assert_eq!(label.as_str(), "\\hat");
                assert!(!*is_stretchy);
                assert!(*is_shifty);
            }
            _ => panic!("expected accent"),
        }
    }

    #[test]
    fn widetilde_is_stretchy_but_shifty() {
        let body = parse(r"\widetilde{abc}").unwrap();
        match &body[0] {
            ParseNode::Accent {
                is_stretchy,
                is_shifty,
                ..
            } => {
                assert!(*is_stretchy);
                assert!(*is_shifty);
            }
            _ => panic!("expected accent"),
        }
    }

    #[test]
    fn sum_is_op_with_limits() {
        let body = parse(r"\sum").unwrap();
        match &body[0] {
            ParseNode::Op {
                limits,
                body: OpBody::Symbol(name),
                ..
            } => {
                assert!(*limits);
                assert_eq!(name.as_str(), "\\sum");
            }
            _ => panic!("expected op"),
        }
    }

    #[test]
    fn sin_is_op_named() {
        let body = parse(r"\sin").unwrap();
        match &body[0] {
            ParseNode::Op {
                limits,
                body: OpBody::Symbol(name),
                ..
            } => {
                assert!(!*limits);
                assert_eq!(name.as_str(), "\\sin");
            }
            _ => panic!("expected op"),
        }
    }

    #[test]
    fn left_right_pairs_delims() {
        let body = parse(r"\left( x \right)").unwrap();
        match &body[0] {
            ParseNode::LeftRight { left, right, .. } => {
                assert_eq!(left.as_str(), "(");
                assert_eq!(right.as_str(), ")");
            }
            _ => panic!("expected leftright"),
        }
    }

    #[test]
    fn middle_outside_left_errors() {
        let err = parse(r"\middle|").unwrap_err();
        assert!(err.raw_message.contains("\\middle without preceding"));
    }

    #[test]
    fn over_infix_rewrites_to_frac() {
        let body = parse(r"a \over b").unwrap();
        // `\over` should rewrite into a single genfrac wrapping a/b.
        assert_eq!(body.len(), 1);
        assert!(matches!(&body[0], ParseNode::GenFrac { .. }));
    }

    #[test]
    fn text_command_switches_to_text_mode() {
        let body = parse(r"\text{hi}").unwrap();
        match &body[0] {
            ParseNode::Text { body, font, .. } => {
                assert_eq!(font.as_ref().map(|s| s.as_str()), Some("\\text"));
                assert_eq!(body.len(), 2);
            }
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn mathbb_builds_font_node() {
        let body = parse(r"\mathbb{R}").unwrap();
        match &body[0] {
            ParseNode::Font { font, .. } => assert_eq!(font.as_str(), "mathbb"),
            _ => panic!("expected font"),
        }
    }

    #[test]
    fn bbb_alias_resolves_to_mathbb() {
        let body = parse(r"\Bbb{R}").unwrap();
        match &body[0] {
            ParseNode::Font { font, .. } => assert_eq!(font.as_str(), "mathbb"),
            _ => panic!("expected font"),
        }
    }

    #[test]
    fn boldsymbol_is_mclass_wrapper() {
        let body = parse(r"\boldsymbol{x}").unwrap();
        match &body[0] {
            ParseNode::MClass { mclass, body, .. } => {
                assert_eq!(mclass.as_str(), "mord");
                assert!(matches!(&body[0], ParseNode::Font { .. }));
            }
            _ => panic!("expected mclass"),
        }
    }

    #[test]
    fn textcolor_carries_color() {
        let body = parse(r"\textcolor{red}{x}").unwrap();
        match &body[0] {
            ParseNode::Color { color, .. } => assert_eq!(color.as_str(), "red"),
            _ => panic!("expected color"),
        }
    }

    #[test]
    fn xleftarrow_with_optional_below() {
        let body = parse(r"\xleftarrow[a]{b}").unwrap();
        match &body[0] {
            ParseNode::XArrow { label, below, .. } => {
                assert_eq!(label.as_str(), "\\xleftarrow");
                assert!(below.is_some());
            }
            _ => panic!("expected xArrow"),
        }
    }

    #[test]
    fn rule_with_shift() {
        let body = parse(r"\rule[1pt]{2pt}{3pt}").unwrap();
        match &body[0] {
            ParseNode::Rule { shift, .. } => assert!(shift.is_some()),
            _ => panic!("expected rule"),
        }
    }

    #[test]
    fn smash_default_smashes_both() {
        let body = parse(r"\smash{x}").unwrap();
        match &body[0] {
            ParseNode::Smash {
                smash_height,
                smash_depth,
                ..
            } => {
                assert!(*smash_height);
                assert!(*smash_depth);
            }
            _ => panic!("expected smash"),
        }
    }

    #[test]
    fn smash_with_t_only_smashes_height() {
        let body = parse(r"\smash[t]{x}").unwrap();
        match &body[0] {
            ParseNode::Smash {
                smash_height,
                smash_depth,
                ..
            } => {
                assert!(*smash_height);
                assert!(!*smash_depth);
            }
            _ => panic!("expected smash"),
        }
    }

    #[test]
    fn unknown_environment_errors() {
        let err = parse(r"\begin{nonesuch}").unwrap_err();
        assert!(err.raw_message.contains("No such environment"));
    }

    #[test]
    fn pmatrix_parses_to_leftright_array() {
        let body = parse(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}").unwrap();
        assert_eq!(body.len(), 1);
        match &body[0] {
            ParseNode::LeftRight {
                body, left, right, ..
            } => {
                assert_eq!(left.as_str(), "(");
                assert_eq!(right.as_str(), ")");
                assert!(matches!(&body[0], ParseNode::Array { .. }));
                if let ParseNode::Array { body: rows, .. } = &body[0] {
                    assert_eq!(rows.len(), 2);
                    assert_eq!(rows[0].len(), 2);
                    assert_eq!(rows[1].len(), 2);
                }
            }
            other => panic!("expected leftright(array), got {:?}", other.node_type()),
        }
    }

    #[test]
    fn matrix_parses_to_array() {
        let body = parse(r"\begin{matrix} 1 & 2 \end{matrix}").unwrap();
        assert!(matches!(&body[0], ParseNode::Array { .. }));
    }

    #[test]
    fn cases_wraps_array_in_leftright_braces() {
        let body = parse(r"\begin{cases} 1 \\ 2 \end{cases}").unwrap();
        match &body[0] {
            ParseNode::LeftRight { left, right, .. } => {
                assert_eq!(left.as_str(), "\\{");
                assert_eq!(right.as_str(), ".");
            }
            _ => panic!("expected leftright"),
        }
    }

    #[test]
    fn aligned_parses_to_array_node() {
        let body = parse(r"\begin{aligned} a &= b \\ c &= d \end{aligned}").unwrap();
        match &body[0] {
            ParseNode::Array {
                body: rows,
                col_separation_type,
                ..
            } => {
                assert_eq!(rows.len(), 2);
                assert_eq!(
                    *col_separation_type,
                    Some(crate::tree::ColSeparationType::Align)
                );
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn mismatched_end_environment_errors() {
        let err = parse(r"\begin{matrix} 1 \end{cases}").unwrap_err();
        assert!(err.raw_message.contains("Mismatch"));
    }

    #[test]
    fn href_without_trust_yields_color_placeholder() {
        let body = parse(r"\href{http://example.com}{link}").unwrap();
        // Default trust=false → handler returns formatUnsupportedCmd.
        match &body[0] {
            ParseNode::Color { color, .. } => assert_eq!(color.as_str(), "#cc0000"),
            other => panic!("expected color placeholder, got {:?}", other.node_type()),
        }
    }

    #[test]
    fn href_with_trust_passes_through() {
        let s = Settings::builder().trust(true).build();
        let body = parse_with(r"\href{http://example.com}{x}", &s).unwrap();
        match &body[0] {
            ParseNode::Href { href, .. } => assert_eq!(href, "http://example.com"),
            _ => panic!("expected href"),
        }
    }

    #[test]
    fn big_delimiter_yields_delimsizing() {
        let body = parse(r"\bigl(").unwrap();
        match &body[0] {
            ParseNode::DelimSizing {
                size,
                mclass,
                delim,
                ..
            } => {
                assert_eq!(*size, 1);
                assert_eq!(mclass.as_str(), "mopen");
                assert_eq!(delim.as_str(), "(");
            }
            _ => panic!("expected delimsizing"),
        }
    }

    #[test]
    fn displaystyle_consumes_rest_of_group() {
        let body = parse(r"\displaystyle x").unwrap();
        match &body[0] {
            ParseNode::Styling { style, body, .. } => {
                assert_eq!(*style, StyleStr::Display);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected styling"),
        }
    }
}

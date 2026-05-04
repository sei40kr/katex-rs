//! AST node type for the parser, builder, and renderers.
//!
//! Mirrors upstream KaTeX's `parseNode.ts`, where each TeX construct is a
//! distinct entry in a `ParseNodeTypes` map keyed by a `type` string.
//!
//! # ADR — single enum, not `Box<dyn ParseNode>`
//!
//! Upstream KaTeX models its AST as a discriminated union of TypeScript
//! object literals; downstream code branches on the `type` field. The
//! direct Rust translation would be `Box<dyn ParseNode>` plus `Any`
//! downcasts, but **we deliberately reject that** in favour of a single
//! `enum ParseNode` for three reasons:
//!
//! 1. **Exhaustive `match` is the whole point.** Both the MathML and the
//!    eventual HTML builder dispatch on node type, and we want the
//!    compiler to flag a missing arm the moment a new variant lands.
//!    Trait objects would push that check to runtime.
//! 2. **No allocator-per-node.** The enum lives unboxed inside parent
//!    `Vec<ParseNode>` bodies; only recursive child fields take a
//!    `Box<ParseNode>`. The dyn-trait alternative would heap-allocate
//!    every node.
//! 3. **No ad-hoc downcasts.** Upstream uses `assertNodeType` /
//!    `checkSymbolNodeType` runtime asserts; with a Rust enum, the
//!    "did the parser hand me a `mathord`?" question is just a `match`.
//!
//! Enum variants use PascalCase versions of the upstream `type` strings,
//! e.g. `"color-token"` → `ColorToken`, `"leftright-right"` →
//! `LeftRightRight`, `"xArrow"` → `XArrow`. The mapping is preserved by
//! [`NodeType`] / [`NodeType::as_str`] for any code that needs the
//! upstream string (e.g. error messages, snapshot diffs).
//!
//! Every variant carries:
//! - `mode: Mode` — math vs text, mirrors upstream
//! - `loc: Option<SourceLocation>` — upstream's `SourceLocation | null |
//!   undefined` collapsed to a single absent state
//!
//! plus its variant-specific fields. Recursive children use
//! `Box<ParseNode>` (single child) or `Vec<ParseNode>` (sequence).

use std::collections::HashMap;

use smol_str::SmolStr;

use crate::source_location::SourceLocation;
use crate::token::Token;
use crate::tree::{AlignSpec, Atom, ColSeparationType, StyleStr};
use crate::types::Mode;
use crate::units::Measurement;

/// Upstream's `op` parse node is a tagged union: either a single-symbol
/// operator (`name: "\\sum"`, `body: void`) or a composite one
/// (`symbol: false`, `body: AnyParseNode[]`). We represent the same
/// invariant with a sub-enum so the impossible "both name and body" /
/// "neither" states are unrepresentable.
#[derive(Clone, Debug, PartialEq)]
pub enum OpBody {
    /// Single-symbol form (`{symbol: true, name: "\\sum"}` upstream).
    Symbol(SmolStr),
    /// Composite form (`{symbol: false, body: [...]}` upstream).
    Composite(Vec<ParseNode>),
}

/// A single row of `hLinesBeforeRow` in an array. Each `bool` is `true`
/// for `\hdashline`, `false` for `\hline`. Mirrors upstream's
/// `Array<boolean[]>` shape.
pub type HLineSpec = Vec<bool>;

/// Per-row tag in `array`-style environments. Upstream uses
/// `boolean | AnyParseNode[]`: `true` means "auto-numbered", `false`
/// means "explicit, no number", and an array means "explicit tag".
#[derive(Clone, Debug, PartialEq)]
pub enum ArrayTag {
    Auto,
    None,
    Explicit(Vec<ParseNode>),
}

/// All AST node variants. Names mirror upstream `parseNode.ts` (see the
/// module-level docs for the casing scheme).
#[derive(Clone, Debug, PartialEq)]
pub enum ParseNode {
    // ----- Symbol-derived nodes ------------------------------------
    /// `"atom"` — a symbol whose group is one of the six atom families.
    Atom {
        family: Atom,
        mode: Mode,
        loc: Option<SourceLocation>,
        text: SmolStr,
    },
    /// `"mathord"` — an ordinary math character.
    MathOrd {
        mode: Mode,
        loc: Option<SourceLocation>,
        text: SmolStr,
    },
    /// `"textord"` — an ordinary text character.
    TextOrd {
        mode: Mode,
        loc: Option<SourceLocation>,
        text: SmolStr,
    },
    /// `"spacing"` — a space-producing symbol (e.g. `\,`, `\;`).
    Spacing {
        mode: Mode,
        loc: Option<SourceLocation>,
        text: SmolStr,
    },
    /// `"accent-token"` — raw accent symbol; produces no DOM, only
    /// flows into `accent` constructions.
    AccentToken {
        mode: Mode,
        loc: Option<SourceLocation>,
        text: SmolStr,
    },
    /// `"op-token"` — raw operator symbol (e.g. `\sum`) before
    /// promotion to a full `op` node.
    OpToken {
        mode: Mode,
        loc: Option<SourceLocation>,
        text: SmolStr,
    },

    // ----- Container / grouping nodes ------------------------------
    /// `"ordgroup"` — a `{...}` group flattened into a sequence.
    OrdGroup {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
        /// `\begingroup` / `\endgroup` set this so the group does not
        /// affect implicit operator-grouping (see upstream).
        semisimple: bool,
    },
    /// `"supsub"` — a base with optional super- and subscripts.
    SupSub {
        mode: Mode,
        loc: Option<SourceLocation>,
        base: Option<Box<ParseNode>>,
        sup: Option<Box<ParseNode>>,
        sub: Option<Box<ParseNode>>,
    },

    // ----- Generic-framed primitives -------------------------------
    /// `"raw"` — opaque source string, used by URL/raw arg parsing.
    Raw {
        mode: Mode,
        loc: Option<SourceLocation>,
        string: String,
    },
    /// `"size"` — a parsed size argument (`1em`, `5ex`, …).
    Size {
        mode: Mode,
        loc: Option<SourceLocation>,
        value: Measurement,
        is_blank: bool,
    },
    /// `"styling"` — `\displaystyle`, `\textstyle`, `\scriptstyle`,
    /// `\scriptscriptstyle`.
    Styling {
        mode: Mode,
        loc: Option<SourceLocation>,
        style: StyleStr,
        body: Vec<ParseNode>,
    },
    /// `"text"` — a `\text{...}` block.
    Text {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
        font: Option<SmolStr>,
    },
    /// `"verb"` — `\verb|...|` literal.
    Verb {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: String,
        star: bool,
    },
    /// `"url"` — a parsed URL argument.
    Url {
        mode: Mode,
        loc: Option<SourceLocation>,
        url: String,
    },
    /// `"color"` — a coloured group (`\color{red}{...}`,
    /// `\textcolor{red}{...}`).
    Color {
        mode: Mode,
        loc: Option<SourceLocation>,
        color: SmolStr,
        body: Vec<ParseNode>,
    },
    /// `"color-token"` — the parsed color argument before it's attached
    /// to a body.
    ColorToken {
        mode: Mode,
        loc: Option<SourceLocation>,
        color: SmolStr,
    },
    /// `"tag"` — equation tag (set via `\tag{...}` or auto-numbering).
    Tag {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
        tag: Vec<ParseNode>,
    },

    // ----- Operators ------------------------------------------------
    /// `"op"` — large operator (`\sum`, `\int`, `\operatorname`-output, …).
    Op {
        mode: Mode,
        loc: Option<SourceLocation>,
        limits: bool,
        always_handle_supsub: bool,
        suppress_base_shift: bool,
        parent_is_supsub: bool,
        body: OpBody,
    },
    /// `"operatorname"` — text-mode operator name.
    OperatorName {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
        always_handle_supsub: bool,
        limits: bool,
        parent_is_supsub: bool,
    },

    // ----- Accents and over/underline ------------------------------
    /// `"accent"` — a top accent (e.g. `\hat{x}`).
    Accent {
        mode: Mode,
        loc: Option<SourceLocation>,
        label: SmolStr,
        is_stretchy: bool,
        is_shifty: bool,
        base: Box<ParseNode>,
    },
    /// `"accentUnder"` — bottom accent.
    AccentUnder {
        mode: Mode,
        loc: Option<SourceLocation>,
        label: SmolStr,
        is_stretchy: bool,
        is_shifty: bool,
        base: Box<ParseNode>,
    },
    /// `"horizBrace"` — `\overbrace` / `\underbrace`.
    HorizBrace {
        mode: Mode,
        loc: Option<SourceLocation>,
        label: SmolStr,
        is_over: bool,
        base: Box<ParseNode>,
    },
    /// `"overline"` — `\overline{...}`.
    Overline {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Box<ParseNode>,
    },
    /// `"underline"` — `\underline{...}`.
    Underline {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Box<ParseNode>,
    },
    /// `"xArrow"` — `\xleftarrow` / `\xrightarrow` / etc.
    XArrow {
        mode: Mode,
        loc: Option<SourceLocation>,
        label: SmolStr,
        body: Box<ParseNode>,
        below: Option<Box<ParseNode>>,
    },

    // ----- Fractions, radicals, delimiters -------------------------
    /// `"genfrac"` — `\frac` / `\binom` / `\genfrac` / `\overset` … all
    /// route through here.
    GenFrac {
        mode: Mode,
        loc: Option<SourceLocation>,
        continued: bool,
        numer: Box<ParseNode>,
        denom: Box<ParseNode>,
        has_bar_line: bool,
        left_delim: Option<SmolStr>,
        right_delim: Option<SmolStr>,
        bar_size: Option<Measurement>,
    },
    /// `"sqrt"` — `\sqrt[n]{x}`.
    Sqrt {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Box<ParseNode>,
        index: Option<Box<ParseNode>>,
    },
    /// `"delimsizing"` — explicit delimiter sizing (`\big`, `\Bigg`, …).
    DelimSizing {
        mode: Mode,
        loc: Option<SourceLocation>,
        size: u8,
        mclass: SmolStr,
        delim: SmolStr,
    },
    /// `"leftright"` — a `\left ... \right` pair.
    LeftRight {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
        left: SmolStr,
        right: SmolStr,
        right_color: Option<SmolStr>,
    },
    /// `"leftright-right"` — sentinel emitted when `\right` is parsed
    /// before its `\left`. The parser uses this to propagate the right
    /// delimiter up to the matching `\left`.
    LeftRightRight {
        mode: Mode,
        loc: Option<SourceLocation>,
        delim: SmolStr,
        color: Option<SmolStr>,
    },
    /// `"middle"` — `\middle|` inside a `\left ... \right`.
    Middle {
        mode: Mode,
        loc: Option<SourceLocation>,
        delim: SmolStr,
    },

    // ----- Spacing / kerning / boxes -------------------------------
    /// `"infix"` — placeholder emitted by infix operators (`\over`, …);
    /// resolved when the surrounding group closes.
    Infix {
        mode: Mode,
        loc: Option<SourceLocation>,
        replace_with: SmolStr,
        size: Option<Measurement>,
        token: Option<Token>,
    },
    /// `"kern"` — explicit horizontal kern.
    Kern {
        mode: Mode,
        loc: Option<SourceLocation>,
        dimension: Measurement,
    },
    /// `"lap"` — `\llap` / `\rlap` / `\clap`.
    Lap {
        mode: Mode,
        loc: Option<SourceLocation>,
        alignment: SmolStr,
        body: Box<ParseNode>,
    },
    /// `"raisebox"` — `\raisebox{dy}{body}`.
    RaiseBox {
        mode: Mode,
        loc: Option<SourceLocation>,
        dy: Measurement,
        body: Box<ParseNode>,
    },
    /// `"rule"` — `\rule[shift]{width}{height}`.
    Rule {
        mode: Mode,
        loc: Option<SourceLocation>,
        shift: Option<Measurement>,
        width: Measurement,
        height: Measurement,
    },
    /// `"smash"` — `\smash[t|b|tb]{body}`.
    Smash {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Box<ParseNode>,
        smash_height: bool,
        smash_depth: bool,
    },
    /// `"hbox"` — internal horizontal box wrapper.
    HBox {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
    },
    /// `"vcenter"` — `\vcenter{...}`.
    VCenter {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Box<ParseNode>,
    },

    // ----- Phantoms ------------------------------------------------
    /// `"phantom"` — `\phantom{...}`.
    Phantom {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Vec<ParseNode>,
    },
    /// `"vphantom"` — `\vphantom{...}` (height/depth only).
    VPhantom {
        mode: Mode,
        loc: Option<SourceLocation>,
        body: Box<ParseNode>,
    },

    // ----- Sizing / fonts / classes --------------------------------
    /// `"sizing"` — `\Huge`, `\large`, etc.
    Sizing {
        mode: Mode,
        loc: Option<SourceLocation>,
        size: u8,
        body: Vec<ParseNode>,
    },
    /// `"font"` — a font directive applied to a single argument
    /// (`\mathbb{...}`, `\mathrm{...}`, …).
    Font {
        mode: Mode,
        loc: Option<SourceLocation>,
        font: SmolStr,
        body: Box<ParseNode>,
    },
    /// `"mclass"` — a forced math-class wrapper (`\mathbin`, `\mathrel`, …).
    MClass {
        mode: Mode,
        loc: Option<SourceLocation>,
        mclass: SmolStr,
        body: Vec<ParseNode>,
        is_character_box: bool,
    },
    /// `"pmb"` — `\pmb{...}` "poor-man's bold".
    Pmb {
        mode: Mode,
        loc: Option<SourceLocation>,
        mclass: SmolStr,
        body: Vec<ParseNode>,
    },

    // ----- Choice / environment / cd ------------------------------
    /// `"mathchoice"` — `\mathchoice{D}{T}{S}{SS}`.
    MathChoice {
        mode: Mode,
        loc: Option<SourceLocation>,
        display: Vec<ParseNode>,
        text: Vec<ParseNode>,
        script: Vec<ParseNode>,
        scriptscript: Vec<ParseNode>,
    },
    /// `"environment"` — placeholder for an unmatched `\begin{...}`
    /// (the dispatch into `_environments` lives in Phase 4's parser).
    Environment {
        mode: Mode,
        loc: Option<SourceLocation>,
        name: SmolStr,
        name_group: Box<ParseNode>,
    },
    /// `"array"` — `\begin{array}{cc} ... \end{array}` and friends.
    Array {
        mode: Mode,
        loc: Option<SourceLocation>,
        col_separation_type: Option<ColSeparationType>,
        hskip_before_and_after: Option<bool>,
        add_jot: Option<bool>,
        cols: Option<Vec<AlignSpec>>,
        arraystretch: f64,
        body: Vec<Vec<ParseNode>>,
        row_gaps: Vec<Option<Measurement>>,
        h_lines_before_row: Vec<HLineSpec>,
        tags: Option<Vec<ArrayTag>>,
        leqno: Option<bool>,
        is_cd: Option<bool>,
    },
    /// `"cdlabel"` — a label on an arrow in a `CD` environment.
    CdLabel {
        mode: Mode,
        loc: Option<SourceLocation>,
        side: SmolStr,
        label: Box<ParseNode>,
    },
    /// `"cdlabelparent"` — the parent of a CD label.
    CdLabelParent {
        mode: Mode,
        loc: Option<SourceLocation>,
        fragment: Box<ParseNode>,
    },
    /// `"cr"` — a `\\` row break inside an environment.
    Cr {
        mode: Mode,
        loc: Option<SourceLocation>,
        new_line: bool,
        size: Option<Measurement>,
    },

    // ----- Enclosures ----------------------------------------------
    /// `"enclose"` — `\fbox`, `\colorbox`, `\fcolorbox`, `\cancel`, …
    Enclose {
        mode: Mode,
        loc: Option<SourceLocation>,
        label: SmolStr,
        background_color: Option<SmolStr>,
        border_color: Option<SmolStr>,
        body: Box<ParseNode>,
    },

    // ----- Hyperlinks / HTML hooks ---------------------------------
    /// `"href"` — `\href{url}{body}`.
    Href {
        mode: Mode,
        loc: Option<SourceLocation>,
        href: String,
        body: Vec<ParseNode>,
    },
    /// `"html"` — `\htmlClass{...}{body}` and friends.
    Html {
        mode: Mode,
        loc: Option<SourceLocation>,
        attributes: HashMap<SmolStr, String>,
        body: Vec<ParseNode>,
    },
    /// `"htmlmathml"` — separate HTML and MathML bodies for the same
    /// source range (`\htmlmathml`).
    HtmlMathml {
        mode: Mode,
        loc: Option<SourceLocation>,
        html: Vec<ParseNode>,
        mathml: Vec<ParseNode>,
    },
    /// `"includegraphics"` — `\includegraphics[opts]{src}`.
    IncludeGraphics {
        mode: Mode,
        loc: Option<SourceLocation>,
        alt: String,
        width: Measurement,
        height: Measurement,
        totalheight: Measurement,
        src: String,
    },

    // ----- Internal sentinels --------------------------------------
    /// `"internal"` — placeholder produced by macros / `\noexpand`-like
    /// machinery; carries no payload.
    Internal {
        mode: Mode,
        loc: Option<SourceLocation>,
    },
}

/// Lightweight discriminant for [`ParseNode`]. The lowercase
/// [`NodeType::as_str`] mirrors upstream's `type` strings exactly so the
/// dispatch tables in [`crate::define_function`] / Phase 6 can key on
/// the same name as upstream.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NodeType {
    Atom,
    MathOrd,
    TextOrd,
    Spacing,
    AccentToken,
    OpToken,
    OrdGroup,
    SupSub,
    Raw,
    Size,
    Styling,
    Text,
    Verb,
    Url,
    Color,
    ColorToken,
    Tag,
    Op,
    OperatorName,
    Accent,
    AccentUnder,
    HorizBrace,
    Overline,
    Underline,
    XArrow,
    GenFrac,
    Sqrt,
    DelimSizing,
    LeftRight,
    LeftRightRight,
    Middle,
    Infix,
    Kern,
    Lap,
    RaiseBox,
    Rule,
    Smash,
    HBox,
    VCenter,
    Phantom,
    VPhantom,
    Sizing,
    Font,
    MClass,
    Pmb,
    MathChoice,
    Environment,
    Array,
    CdLabel,
    CdLabelParent,
    Cr,
    Enclose,
    Href,
    Html,
    HtmlMathml,
    IncludeGraphics,
    Internal,
}

impl NodeType {
    /// Upstream `type` string. Stable wire format used by
    /// snapshot-test diffs and (Phase 6+) builder dispatch.
    pub const fn as_str(self) -> &'static str {
        match self {
            NodeType::Atom => "atom",
            NodeType::MathOrd => "mathord",
            NodeType::TextOrd => "textord",
            NodeType::Spacing => "spacing",
            NodeType::AccentToken => "accent-token",
            NodeType::OpToken => "op-token",
            NodeType::OrdGroup => "ordgroup",
            NodeType::SupSub => "supsub",
            NodeType::Raw => "raw",
            NodeType::Size => "size",
            NodeType::Styling => "styling",
            NodeType::Text => "text",
            NodeType::Verb => "verb",
            NodeType::Url => "url",
            NodeType::Color => "color",
            NodeType::ColorToken => "color-token",
            NodeType::Tag => "tag",
            NodeType::Op => "op",
            NodeType::OperatorName => "operatorname",
            NodeType::Accent => "accent",
            NodeType::AccentUnder => "accentUnder",
            NodeType::HorizBrace => "horizBrace",
            NodeType::Overline => "overline",
            NodeType::Underline => "underline",
            NodeType::XArrow => "xArrow",
            NodeType::GenFrac => "genfrac",
            NodeType::Sqrt => "sqrt",
            NodeType::DelimSizing => "delimsizing",
            NodeType::LeftRight => "leftright",
            NodeType::LeftRightRight => "leftright-right",
            NodeType::Middle => "middle",
            NodeType::Infix => "infix",
            NodeType::Kern => "kern",
            NodeType::Lap => "lap",
            NodeType::RaiseBox => "raisebox",
            NodeType::Rule => "rule",
            NodeType::Smash => "smash",
            NodeType::HBox => "hbox",
            NodeType::VCenter => "vcenter",
            NodeType::Phantom => "phantom",
            NodeType::VPhantom => "vphantom",
            NodeType::Sizing => "sizing",
            NodeType::Font => "font",
            NodeType::MClass => "mclass",
            NodeType::Pmb => "pmb",
            NodeType::MathChoice => "mathchoice",
            NodeType::Environment => "environment",
            NodeType::Array => "array",
            NodeType::CdLabel => "cdlabel",
            NodeType::CdLabelParent => "cdlabelparent",
            NodeType::Cr => "cr",
            NodeType::Enclose => "enclose",
            NodeType::Href => "href",
            NodeType::Html => "html",
            NodeType::HtmlMathml => "htmlmathml",
            NodeType::IncludeGraphics => "includegraphics",
            NodeType::Internal => "internal",
        }
    }
}

impl ParseNode {
    /// Discriminant. Equivalent to upstream's `node.type`.
    pub fn node_type(&self) -> NodeType {
        match self {
            ParseNode::Atom { .. } => NodeType::Atom,
            ParseNode::MathOrd { .. } => NodeType::MathOrd,
            ParseNode::TextOrd { .. } => NodeType::TextOrd,
            ParseNode::Spacing { .. } => NodeType::Spacing,
            ParseNode::AccentToken { .. } => NodeType::AccentToken,
            ParseNode::OpToken { .. } => NodeType::OpToken,
            ParseNode::OrdGroup { .. } => NodeType::OrdGroup,
            ParseNode::SupSub { .. } => NodeType::SupSub,
            ParseNode::Raw { .. } => NodeType::Raw,
            ParseNode::Size { .. } => NodeType::Size,
            ParseNode::Styling { .. } => NodeType::Styling,
            ParseNode::Text { .. } => NodeType::Text,
            ParseNode::Verb { .. } => NodeType::Verb,
            ParseNode::Url { .. } => NodeType::Url,
            ParseNode::Color { .. } => NodeType::Color,
            ParseNode::ColorToken { .. } => NodeType::ColorToken,
            ParseNode::Tag { .. } => NodeType::Tag,
            ParseNode::Op { .. } => NodeType::Op,
            ParseNode::OperatorName { .. } => NodeType::OperatorName,
            ParseNode::Accent { .. } => NodeType::Accent,
            ParseNode::AccentUnder { .. } => NodeType::AccentUnder,
            ParseNode::HorizBrace { .. } => NodeType::HorizBrace,
            ParseNode::Overline { .. } => NodeType::Overline,
            ParseNode::Underline { .. } => NodeType::Underline,
            ParseNode::XArrow { .. } => NodeType::XArrow,
            ParseNode::GenFrac { .. } => NodeType::GenFrac,
            ParseNode::Sqrt { .. } => NodeType::Sqrt,
            ParseNode::DelimSizing { .. } => NodeType::DelimSizing,
            ParseNode::LeftRight { .. } => NodeType::LeftRight,
            ParseNode::LeftRightRight { .. } => NodeType::LeftRightRight,
            ParseNode::Middle { .. } => NodeType::Middle,
            ParseNode::Infix { .. } => NodeType::Infix,
            ParseNode::Kern { .. } => NodeType::Kern,
            ParseNode::Lap { .. } => NodeType::Lap,
            ParseNode::RaiseBox { .. } => NodeType::RaiseBox,
            ParseNode::Rule { .. } => NodeType::Rule,
            ParseNode::Smash { .. } => NodeType::Smash,
            ParseNode::HBox { .. } => NodeType::HBox,
            ParseNode::VCenter { .. } => NodeType::VCenter,
            ParseNode::Phantom { .. } => NodeType::Phantom,
            ParseNode::VPhantom { .. } => NodeType::VPhantom,
            ParseNode::Sizing { .. } => NodeType::Sizing,
            ParseNode::Font { .. } => NodeType::Font,
            ParseNode::MClass { .. } => NodeType::MClass,
            ParseNode::Pmb { .. } => NodeType::Pmb,
            ParseNode::MathChoice { .. } => NodeType::MathChoice,
            ParseNode::Environment { .. } => NodeType::Environment,
            ParseNode::Array { .. } => NodeType::Array,
            ParseNode::CdLabel { .. } => NodeType::CdLabel,
            ParseNode::CdLabelParent { .. } => NodeType::CdLabelParent,
            ParseNode::Cr { .. } => NodeType::Cr,
            ParseNode::Enclose { .. } => NodeType::Enclose,
            ParseNode::Href { .. } => NodeType::Href,
            ParseNode::Html { .. } => NodeType::Html,
            ParseNode::HtmlMathml { .. } => NodeType::HtmlMathml,
            ParseNode::IncludeGraphics { .. } => NodeType::IncludeGraphics,
            ParseNode::Internal { .. } => NodeType::Internal,
        }
    }

    /// Mode the node was parsed in (math vs text).
    pub fn mode(&self) -> Mode {
        match self {
            ParseNode::Atom { mode, .. }
            | ParseNode::MathOrd { mode, .. }
            | ParseNode::TextOrd { mode, .. }
            | ParseNode::Spacing { mode, .. }
            | ParseNode::AccentToken { mode, .. }
            | ParseNode::OpToken { mode, .. }
            | ParseNode::OrdGroup { mode, .. }
            | ParseNode::SupSub { mode, .. }
            | ParseNode::Raw { mode, .. }
            | ParseNode::Size { mode, .. }
            | ParseNode::Styling { mode, .. }
            | ParseNode::Text { mode, .. }
            | ParseNode::Verb { mode, .. }
            | ParseNode::Url { mode, .. }
            | ParseNode::Color { mode, .. }
            | ParseNode::ColorToken { mode, .. }
            | ParseNode::Tag { mode, .. }
            | ParseNode::Op { mode, .. }
            | ParseNode::OperatorName { mode, .. }
            | ParseNode::Accent { mode, .. }
            | ParseNode::AccentUnder { mode, .. }
            | ParseNode::HorizBrace { mode, .. }
            | ParseNode::Overline { mode, .. }
            | ParseNode::Underline { mode, .. }
            | ParseNode::XArrow { mode, .. }
            | ParseNode::GenFrac { mode, .. }
            | ParseNode::Sqrt { mode, .. }
            | ParseNode::DelimSizing { mode, .. }
            | ParseNode::LeftRight { mode, .. }
            | ParseNode::LeftRightRight { mode, .. }
            | ParseNode::Middle { mode, .. }
            | ParseNode::Infix { mode, .. }
            | ParseNode::Kern { mode, .. }
            | ParseNode::Lap { mode, .. }
            | ParseNode::RaiseBox { mode, .. }
            | ParseNode::Rule { mode, .. }
            | ParseNode::Smash { mode, .. }
            | ParseNode::HBox { mode, .. }
            | ParseNode::VCenter { mode, .. }
            | ParseNode::Phantom { mode, .. }
            | ParseNode::VPhantom { mode, .. }
            | ParseNode::Sizing { mode, .. }
            | ParseNode::Font { mode, .. }
            | ParseNode::MClass { mode, .. }
            | ParseNode::Pmb { mode, .. }
            | ParseNode::MathChoice { mode, .. }
            | ParseNode::Environment { mode, .. }
            | ParseNode::Array { mode, .. }
            | ParseNode::CdLabel { mode, .. }
            | ParseNode::CdLabelParent { mode, .. }
            | ParseNode::Cr { mode, .. }
            | ParseNode::Enclose { mode, .. }
            | ParseNode::Href { mode, .. }
            | ParseNode::Html { mode, .. }
            | ParseNode::HtmlMathml { mode, .. }
            | ParseNode::IncludeGraphics { mode, .. }
            | ParseNode::Internal { mode, .. } => *mode,
        }
    }

    /// Source span, when known. Mirrors upstream's `node.loc`.
    pub fn loc(&self) -> Option<&SourceLocation> {
        match self {
            ParseNode::Atom { loc, .. }
            | ParseNode::MathOrd { loc, .. }
            | ParseNode::TextOrd { loc, .. }
            | ParseNode::Spacing { loc, .. }
            | ParseNode::AccentToken { loc, .. }
            | ParseNode::OpToken { loc, .. }
            | ParseNode::OrdGroup { loc, .. }
            | ParseNode::SupSub { loc, .. }
            | ParseNode::Raw { loc, .. }
            | ParseNode::Size { loc, .. }
            | ParseNode::Styling { loc, .. }
            | ParseNode::Text { loc, .. }
            | ParseNode::Verb { loc, .. }
            | ParseNode::Url { loc, .. }
            | ParseNode::Color { loc, .. }
            | ParseNode::ColorToken { loc, .. }
            | ParseNode::Tag { loc, .. }
            | ParseNode::Op { loc, .. }
            | ParseNode::OperatorName { loc, .. }
            | ParseNode::Accent { loc, .. }
            | ParseNode::AccentUnder { loc, .. }
            | ParseNode::HorizBrace { loc, .. }
            | ParseNode::Overline { loc, .. }
            | ParseNode::Underline { loc, .. }
            | ParseNode::XArrow { loc, .. }
            | ParseNode::GenFrac { loc, .. }
            | ParseNode::Sqrt { loc, .. }
            | ParseNode::DelimSizing { loc, .. }
            | ParseNode::LeftRight { loc, .. }
            | ParseNode::LeftRightRight { loc, .. }
            | ParseNode::Middle { loc, .. }
            | ParseNode::Infix { loc, .. }
            | ParseNode::Kern { loc, .. }
            | ParseNode::Lap { loc, .. }
            | ParseNode::RaiseBox { loc, .. }
            | ParseNode::Rule { loc, .. }
            | ParseNode::Smash { loc, .. }
            | ParseNode::HBox { loc, .. }
            | ParseNode::VCenter { loc, .. }
            | ParseNode::Phantom { loc, .. }
            | ParseNode::VPhantom { loc, .. }
            | ParseNode::Sizing { loc, .. }
            | ParseNode::Font { loc, .. }
            | ParseNode::MClass { loc, .. }
            | ParseNode::Pmb { loc, .. }
            | ParseNode::MathChoice { loc, .. }
            | ParseNode::Environment { loc, .. }
            | ParseNode::Array { loc, .. }
            | ParseNode::CdLabel { loc, .. }
            | ParseNode::CdLabelParent { loc, .. }
            | ParseNode::Cr { loc, .. }
            | ParseNode::Enclose { loc, .. }
            | ParseNode::Href { loc, .. }
            | ParseNode::Html { loc, .. }
            | ParseNode::HtmlMathml { loc, .. }
            | ParseNode::IncludeGraphics { loc, .. }
            | ParseNode::Internal { loc, .. } => loc.as_ref(),
        }
    }

    /// True for the six variants upstream's `assertSymbolNodeType` /
    /// `checkSymbolNodeType` accepts: `atom`, `mathord`, `textord`,
    /// `spacing`, `accent-token`, `op-token`.
    pub fn is_symbol_node(&self) -> bool {
        matches!(
            self,
            ParseNode::Atom { .. }
                | ParseNode::MathOrd { .. }
                | ParseNode::TextOrd { .. }
                | ParseNode::Spacing { .. }
                | ParseNode::AccentToken { .. }
                | ParseNode::OpToken { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn math_atom(text: &str) -> ParseNode {
        ParseNode::Atom {
            family: Atom::Bin,
            mode: Mode::Math,
            loc: None,
            text: SmolStr::new(text),
        }
    }

    #[test]
    fn node_type_strings_match_upstream() {
        assert_eq!(NodeType::Atom.as_str(), "atom");
        assert_eq!(NodeType::AccentToken.as_str(), "accent-token");
        assert_eq!(NodeType::LeftRightRight.as_str(), "leftright-right");
        assert_eq!(NodeType::XArrow.as_str(), "xArrow");
        assert_eq!(NodeType::AccentUnder.as_str(), "accentUnder");
        assert_eq!(NodeType::HorizBrace.as_str(), "horizBrace");
        assert_eq!(NodeType::CdLabel.as_str(), "cdlabel");
        assert_eq!(NodeType::CdLabelParent.as_str(), "cdlabelparent");
        assert_eq!(NodeType::IncludeGraphics.as_str(), "includegraphics");
        assert_eq!(NodeType::HtmlMathml.as_str(), "htmlmathml");
    }

    #[test]
    fn discriminant_round_trips() {
        let n = math_atom("+");
        assert_eq!(n.node_type(), NodeType::Atom);
        assert_eq!(n.node_type().as_str(), "atom");
        assert_eq!(n.mode(), Mode::Math);
        assert!(n.loc().is_none());
        assert!(n.is_symbol_node());
    }

    #[test]
    fn supsub_carries_optional_children() {
        let n = ParseNode::SupSub {
            mode: Mode::Math,
            loc: None,
            base: Some(Box::new(math_atom("x"))),
            sup: Some(Box::new(math_atom("2"))),
            sub: None,
        };
        assert_eq!(n.node_type(), NodeType::SupSub);
        assert!(!n.is_symbol_node());
    }

    #[test]
    fn op_body_distinguishes_symbol_vs_composite() {
        let sym = ParseNode::Op {
            mode: Mode::Math,
            loc: None,
            limits: false,
            always_handle_supsub: false,
            suppress_base_shift: false,
            parent_is_supsub: false,
            body: OpBody::Symbol(SmolStr::new("\\sum")),
        };
        let composite = ParseNode::Op {
            mode: Mode::Math,
            loc: None,
            limits: false,
            always_handle_supsub: false,
            suppress_base_shift: false,
            parent_is_supsub: false,
            body: OpBody::Composite(vec![math_atom("a")]),
        };
        match &sym {
            ParseNode::Op {
                body: OpBody::Symbol(s),
                ..
            } => assert_eq!(s, "\\sum"),
            _ => panic!("symbol form"),
        }
        match &composite {
            ParseNode::Op {
                body: OpBody::Composite(v),
                ..
            } => assert_eq!(v.len(), 1),
            _ => panic!("composite form"),
        }
    }

    #[test]
    fn symbol_predicate_matches_upstream() {
        let symbol_variants = [
            math_atom("+"),
            ParseNode::MathOrd {
                mode: Mode::Math,
                loc: None,
                text: SmolStr::new("x"),
            },
            ParseNode::TextOrd {
                mode: Mode::Text,
                loc: None,
                text: SmolStr::new("a"),
            },
            ParseNode::Spacing {
                mode: Mode::Math,
                loc: None,
                text: SmolStr::new("\\,"),
            },
            ParseNode::AccentToken {
                mode: Mode::Math,
                loc: None,
                text: SmolStr::new("\\hat"),
            },
            ParseNode::OpToken {
                mode: Mode::Math,
                loc: None,
                text: SmolStr::new("\\sum"),
            },
        ];
        for n in &symbol_variants {
            assert!(n.is_symbol_node(), "expected symbol: {:?}", n.node_type());
        }
        let non_symbol = ParseNode::OrdGroup {
            mode: Mode::Math,
            loc: None,
            body: vec![],
            semisimple: false,
        };
        assert!(!non_symbol.is_symbol_node());
    }
}

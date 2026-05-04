//! Build a [`crate::mathml_tree::MathMlNode`] tree from a `Vec<ParseNode>`.
//!
//! Mirrors upstream KaTeX's `buildMathML.ts`. The three top-level
//! entry points — [`build_math_ml`], [`build_expression`], and
//! [`build_group`] — match upstream by name. Per-`ParseNode`-variant
//! builders are dispatched from [`build_group`] via an exhaustive
//! `match`; the compiler flags any new variant the moment it's added.
//!
//! # Deviations from upstream
//!
//! - Upstream registers per-NodeType builders into a side-table via
//!   `defineFunctionBuilders`. We dispatch by `match` on the
//!   [`ParseNode`] enum directly, which collapses the indirection and
//!   makes the variant→builder mapping checkable at compile time. The
//!   `MathmlBuilder` fn-pointer slot in [`crate::define_function::FunctionSpec`]
//!   stays for future use (and for parity with upstream); the Phase-6
//!   pipeline doesn't need it.
//! - Spacing widths for Phase-6 are a small inline lookup against
//!   upstream's `\,`, `\:`, `\;`, `\!`, `\quad`, `\qquad`, … table.
//!   Anything else falls through to a zero-width `<mspace/>`. The full
//!   spacing-data driven width calculation lands in Phase 10 alongside
//!   HTML+CSS rendering.

use std::fmt::Write as _;

use smol_str::SmolStr;

use crate::mathml_tree::{MathMlElement, MathMlNode};
use crate::options::Options;
use crate::parse_node::{OpBody, ParseNode};
use crate::settings::Settings;
use crate::style::Style;
use crate::symbols::SYMBOLS;
use crate::tree::{Atom, StyleStr};
use crate::types::Mode;
use crate::units::Measurement;

/// Build the top-level `<math xmlns=…><semantics><mrow>…</mrow><annotation
/// encoding="application/x-tex">…</annotation></semantics></math>`
/// element. Mirrors upstream `buildMathML`.
pub fn build_math_ml(
    tree: &[ParseNode],
    tex: &str,
    options: &Options,
    settings: &Settings,
    for_mathml_only: bool,
) -> MathMlElement {
    let body = build_expression(tree, options, true);
    // Upstream skips the wrapper mrow only when the single child is an
    // already row-like element (`mrow` or `mtable`). Other single-child
    // shapes (`mi`, `mo`, `mfrac`, `mroot`, …) still get wrapped.
    let wrapper = if body.len() == 1
        && let Some(MathMlNode::Element(el)) = body.first()
        && matches!(el.tag.as_str(), "mrow" | "mtable")
    {
        body.into_iter().next().expect("len == 1")
    } else {
        MathMlElement::with_children("mrow", body).into_node()
    };
    let annotation =
        MathMlElement::with_children("annotation", vec![MathMlNode::Text(tex.to_string())])
            .with_attribute("encoding", "application/x-tex")
            .into_node();
    let semantics =
        MathMlElement::with_children("semantics", vec![wrapper, annotation]).into_node();
    let mut math = MathMlElement::with_children("math", vec![semantics])
        .with_attribute("xmlns", "http://www.w3.org/1998/Math/MathML");
    if settings.display_mode {
        math.set_attribute("display", "block");
    }
    // `for_mathml_only` will gate HTML+MathML pairing in Phase 10. For
    // pure MathML output the flag has no effect today; touching it
    // keeps the public signature stable across phases.
    let _ = for_mathml_only;
    math
}

/// Build a list of MathML nodes from an expression. Mirrors upstream
/// `buildExpression`. The `is_ordgroup` flag is reserved for Phase-7
/// snapshot-parity tweaks (the "binrel-tightening" pass upstream runs
/// only on non-ordgroup expressions); Phase 6 ignores it.
pub fn build_expression(
    expression: &[ParseNode],
    options: &Options,
    is_ordgroup: bool,
) -> Vec<MathMlNode> {
    let _ = is_ordgroup;
    let mut out = Vec::with_capacity(expression.len());
    for node in expression {
        out.push(build_group(node, options));
    }
    out
}

/// Build a single MathML node from a parse node.
pub fn build_group(group: &ParseNode, options: &Options) -> MathMlNode {
    match group {
        ParseNode::MathOrd { mode, text, .. } => mathord_node(*mode, text, options),
        ParseNode::TextOrd { mode, text, .. } => textord_node(*mode, text, options),
        ParseNode::Atom {
            family, mode, text, ..
        } => atom_node(*family, *mode, text, options),
        ParseNode::Spacing { text, .. } => spacing_node(text),
        ParseNode::AccentToken { mode, text, .. } => mo_text_node(*mode, text, options),
        ParseNode::OpToken { mode, text, .. } => mo_text_node(*mode, text, options),
        ParseNode::OrdGroup { body, .. } => ordgroup_node(body, options),
        ParseNode::SupSub { base, sup, sub, .. } => {
            supsub_node(base.as_deref(), sup.as_deref(), sub.as_deref(), options)
        }
        ParseNode::GenFrac {
            numer,
            denom,
            has_bar_line,
            left_delim,
            right_delim,
            bar_size,
            ..
        } => genfrac_node(
            numer,
            denom,
            *has_bar_line,
            left_delim.as_deref(),
            right_delim.as_deref(),
            *bar_size,
            options,
        ),
        ParseNode::Sqrt { body, index, .. } => sqrt_node(body, index.as_deref(), options),
        ParseNode::Color { color, body, .. } => color_node(color, body, options),
        ParseNode::Styling { style, body, .. } => styling_node(*style, body, options),
        ParseNode::Text { body, font, .. } => text_node(body, font.as_deref(), options),
        ParseNode::Verb { body, star, .. } => verb_node(body, *star),
        ParseNode::Op {
            limits,
            always_handle_supsub,
            parent_is_supsub,
            body,
            ..
        } => op_node(
            *limits,
            *always_handle_supsub,
            *parent_is_supsub,
            body,
            options,
        ),
        ParseNode::OperatorName {
            body,
            limits,
            parent_is_supsub,
            always_handle_supsub,
            ..
        } => operatorname_node(
            body,
            *limits,
            *parent_is_supsub,
            *always_handle_supsub,
            options,
        ),
        ParseNode::Accent {
            label,
            base,
            is_stretchy,
            ..
        } => accent_node(label, base, *is_stretchy, true, options),
        ParseNode::AccentUnder {
            label,
            base,
            is_stretchy,
            ..
        } => accent_node(label, base, *is_stretchy, false, options),
        ParseNode::HorizBrace {
            label,
            base,
            is_over,
            ..
        } => accent_node(label, base, true, *is_over, options),
        ParseNode::Overline { body, .. } => {
            mathml_one_arg_op("mover", body, "\u{203e}", true, options)
        }
        ParseNode::Underline { body, .. } => {
            mathml_one_arg_op("munder", body, "\u{203e}", false, options)
        }
        ParseNode::XArrow {
            label, body, below, ..
        } => xarrow_node(label, body, below.as_deref(), options),
        ParseNode::Font { font, body, .. } => font_node(font, body, options),
        ParseNode::MClass { body, .. } => ordgroup_node(body, options),
        ParseNode::Pmb { body, .. } => pmb_node(body, options),
        ParseNode::Sizing { body, .. } => ordgroup_node(body, options),
        ParseNode::DelimSizing { delim, mclass, .. } => delim_sizing_node(delim, mclass, options),
        ParseNode::LeftRight {
            body, left, right, ..
        } => leftright_node(body, left, right, options),
        ParseNode::Middle { delim, .. } => mo_fence(delim, options),
        ParseNode::Phantom { body, .. } => mphantom_node(body, options),
        ParseNode::VPhantom { body, .. } => mphantom_one(body, options),
        ParseNode::Smash { body, .. } => build_group(body, options),
        ParseNode::Lap { body, .. } => mpadded_zero_width(body, options),
        ParseNode::HBox { body, .. } => ordgroup_node(body, options),
        ParseNode::VCenter { body, .. } => build_group(body, options),
        ParseNode::Kern { dimension, .. } => kern_node(*dimension),
        ParseNode::RaiseBox { body, dy, .. } => raisebox_node(body, *dy, options),
        ParseNode::Rule { width, height, .. } => rule_node(*width, *height),
        ParseNode::Tag { body, tag, .. } => tag_node(body, tag, options),
        ParseNode::Href { body, href, .. } => href_node(body, href, options),
        ParseNode::Html { body, .. } => ordgroup_node(body, options),
        ParseNode::HtmlMathml { mathml, .. } => ordgroup_node(mathml, options),
        ParseNode::Enclose { label, body, .. } => enclose_node(label, body, options),
        ParseNode::IncludeGraphics { alt, .. } => mtext(alt),
        ParseNode::MathChoice {
            display,
            text,
            script,
            scriptscript,
            ..
        } => mathchoice_node(display, text, script, scriptscript, options),
        ParseNode::Array { .. } => crate::environments::array::array_mathml_builder(group, options),
        ParseNode::CdLabel { label, .. } => build_group(label, options),
        ParseNode::CdLabelParent { fragment, .. } => build_group(fragment, options),
        ParseNode::Cr { .. } => MathMlElement::new("mspace").into_node(),
        ParseNode::Url { url, .. } => mtext(url),
        ParseNode::ColorToken { color, .. } => mtext(color.as_str()),
        ParseNode::Internal { .. } => MathMlElement::new("mrow").into_node(),
        ParseNode::Infix { .. } => MathMlElement::new("mrow").into_node(),
        ParseNode::Raw { string, .. } => mtext(string),
        ParseNode::Size { .. } => MathMlElement::new("mspace").into_node(),
        ParseNode::LeftRightRight { delim, .. } => mo_fence(delim, options),
        ParseNode::Environment { name, .. } => mtext(name.as_str()),
    }
}

// ----- Symbol-derived nodes -----------------------------------------

fn mathord_node(mode: Mode, text: &str, options: &Options) -> MathMlNode {
    let resolved = resolve_symbol(mode, text);
    let mut el = MathMlElement::with_children("mi", vec![MathMlNode::Text(resolved)]);
    apply_mathvariant(&mut el, options);
    el.into_node()
}

fn textord_node(mode: Mode, text: &str, options: &Options) -> MathMlNode {
    let resolved = resolve_symbol(mode, text);
    let tag = if is_single_digit(&resolved) {
        "mn"
    } else if resolved.chars().any(|c| c.is_ascii_alphabetic()) {
        "mi"
    } else {
        "mo"
    };
    let mut el = MathMlElement::with_children(tag, vec![MathMlNode::Text(resolved)]);
    if tag == "mi" {
        apply_mathvariant(&mut el, options);
    }
    el.into_node()
}

fn atom_node(_family: Atom, mode: Mode, text: &str, options: &Options) -> MathMlNode {
    let _ = options;
    let resolved = resolve_symbol(mode, text);
    MathMlElement::with_children("mo", vec![MathMlNode::Text(resolved)]).into_node()
}

fn mo_text_node(mode: Mode, text: &str, _options: &Options) -> MathMlNode {
    let resolved = resolve_symbol(mode, text);
    MathMlElement::with_children("mo", vec![MathMlNode::Text(resolved)]).into_node()
}

/// Resolve a symbol-table entry to its MathML character. Falls back to
/// the source text when no entry exists.
fn resolve_symbol(mode: Mode, text: &str) -> String {
    if let Some(info) = SYMBOLS.get((mode, text))
        && let Some(replace) = info.replace
    {
        return replace.to_string();
    }
    text.to_string()
}

fn is_single_digit(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(c) = chars.next() else { return false };
    chars.next().is_none() && c.is_ascii_digit()
}

/// Map upstream's font name (`mathbb`, `mathfrak`, …) to the MathML
/// `mathvariant` attribute value. Mirrors upstream's `fontMap`.
fn font_to_mathvariant(font: &str) -> Option<&'static str> {
    match font {
        "mathbb" | "textbb" => Some("double-struck"),
        "mathbf" | "textbf" => Some("bold"),
        "mathit" | "textit" => Some("italic"),
        "mathnormal" => Some("italic"),
        "mathrm" | "textrm" => Some("normal"),
        "mathsf" | "textsf" => Some("sans-serif"),
        "mathtt" | "texttt" => Some("monospace"),
        "mathfrak" => Some("fraktur"),
        "mathcal" => Some("script"),
        "mathscr" => Some("script"),
        "mathbfit" | "boldsymbol" => Some("bold-italic"),
        _ => None,
    }
}

fn apply_mathvariant(el: &mut MathMlElement, options: &Options) {
    if let Some(font) = options.font.as_deref()
        && let Some(variant) = font_to_mathvariant(font)
    {
        el.set_attribute("mathvariant", variant);
    }
}

// ----- Container / grouping nodes -----------------------------------

fn ordgroup_node(body: &[ParseNode], options: &Options) -> MathMlNode {
    let children = build_expression(body, options, true);
    if children.len() == 1 {
        return children.into_iter().next().unwrap();
    }
    MathMlElement::with_children("mrow", children).into_node()
}

fn supsub_node(
    base: Option<&ParseNode>,
    sup: Option<&ParseNode>,
    sub: Option<&ParseNode>,
    options: &Options,
) -> MathMlNode {
    let base_node = match base {
        Some(b) => build_group(b, options),
        None => MathMlElement::new("mrow").into_node(),
    };
    let sup_node = sup.map(|n| build_group(n, &options.having_style(options.style.sup())));
    let sub_node = sub.map(|n| build_group(n, &options.having_style(options.style.sub())));

    // If the base is an op with `limits`, route through munder/mover.
    let use_limits = matches!(base, Some(ParseNode::Op { limits: true, .. }))
        || matches!(base, Some(ParseNode::OperatorName { limits: true, .. }));

    let (tag, children) = match (use_limits, sup_node, sub_node) {
        (true, Some(s), Some(b)) => ("munderover", vec![base_node, b, s]),
        (true, Some(s), None) => ("mover", vec![base_node, s]),
        (true, None, Some(b)) => ("munder", vec![base_node, b]),
        (false, Some(s), Some(b)) => ("msubsup", vec![base_node, b, s]),
        (false, Some(s), None) => ("msup", vec![base_node, s]),
        (false, None, Some(b)) => ("msub", vec![base_node, b]),
        (_, None, None) => return base_node,
    };
    MathMlElement::with_children(tag, children).into_node()
}

// ----- Fractions, radicals, delimiters ------------------------------

fn genfrac_node(
    numer: &ParseNode,
    denom: &ParseNode,
    has_bar_line: bool,
    left_delim: Option<&str>,
    right_delim: Option<&str>,
    bar_size: Option<Measurement>,
    options: &Options,
) -> MathMlNode {
    let numer_opts = options.having_style(options.style.frac_num());
    let denom_opts = options.having_style(options.style.frac_den());
    let numer_node = build_group(numer, &numer_opts);
    let denom_node = build_group(denom, &denom_opts);
    let mut frac = MathMlElement::with_children("mfrac", vec![numer_node, denom_node]);
    if !has_bar_line {
        frac.set_attribute("linethickness", "0px");
    } else if let Some(size) = bar_size {
        let mut buf = String::new();
        let _ = write!(buf, "{}{}", size.number, size.unit.as_str());
        frac.set_attribute("linethickness", buf);
    }
    let mut node = frac.into_node();
    if left_delim.is_some() || right_delim.is_some() {
        let mut row = MathMlElement::new("mrow");
        if let Some(d) = left_delim {
            row.push(mo_fence(d, options));
        }
        row.push(node);
        if let Some(d) = right_delim {
            row.push(mo_fence(d, options));
        }
        node = row.into_node();
    }
    node
}

fn sqrt_node(body: &ParseNode, index: Option<&ParseNode>, options: &Options) -> MathMlNode {
    let body_node = build_group(body, options);
    match index {
        Some(idx) => {
            let idx_node = build_group(idx, &options.having_style(Style::ScriptScript));
            MathMlElement::with_children("mroot", vec![body_node, idx_node]).into_node()
        }
        None => MathMlElement::with_children("msqrt", vec![body_node]).into_node(),
    }
}

fn mo_fence(delim: &str, _options: &Options) -> MathMlNode {
    let resolved = resolve_symbol(Mode::Math, delim);
    let mut el = MathMlElement::with_children("mo", vec![MathMlNode::Text(resolved)]);
    el.set_attribute("fence", "true");
    // Upstream omits `stretchy="true"` here — `<mo fence="true">` already
    // implies the default stretchy behavior in MathML.
    el.into_node()
}

fn delim_sizing_node(delim: &str, mclass: &str, _options: &Options) -> MathMlNode {
    let resolved = resolve_symbol(Mode::Math, delim);
    let mut el = MathMlElement::with_children("mo", vec![MathMlNode::Text(resolved)]);
    el.set_attribute("fence", "true");
    if !mclass.is_empty() {
        let role = match mclass {
            "mopen" => "open",
            "mclose" => "close",
            _ => "",
        };
        if !role.is_empty() {
            el.set_attribute("form", role);
        }
    }
    el.into_node()
}

fn leftright_node(body: &[ParseNode], left: &str, right: &str, options: &Options) -> MathMlNode {
    let mut row = MathMlElement::new("mrow");
    // `.` is upstream's "no delimiter" sentinel — skip the `<mo>` for
    // either side so the row stays bare.
    if left != "." {
        row.push(mo_fence(left, options));
    }
    for node in build_expression(body, options, false) {
        row.push(node);
    }
    if right != "." {
        row.push(mo_fence(right, options));
    }
    row.into_node()
}

// ----- Spacing / kerning / boxes ------------------------------------

fn spacing_node(text: &str) -> MathMlNode {
    // Mirrors upstream's `spacingFunctions` entries used in MathML.
    let width_em = match text {
        "\\!" => Some(-0.16667),
        "\\," => Some(0.16667),
        "\\>" | "\\:" | "\\medspace" => Some(0.22222),
        "\\;" | "\\thickspace" => Some(0.27778),
        "\\enspace" => Some(0.5),
        "\\quad" => Some(1.0),
        "\\qquad" => Some(2.0),
        " " | "\\ " | "~" | "\\space" | "\\nobreakspace" => None,
        _ => None,
    };
    match width_em {
        Some(em) => MathMlNode::Space(em),
        None => {
            // A non-zero "regular" space — emit a non-breaking text node
            // (upstream emits `\u{00a0}` / `\u{00a0}\u{00a0}` based on
            // context; in MathML mode we just emit a literal space).
            let nbsp = if text == "~" || text == "\\nobreakspace" {
                "\u{00a0}".to_string()
            } else {
                " ".to_string()
            };
            MathMlElement::with_children("mtext", vec![MathMlNode::Text(nbsp)]).into_node()
        }
    }
}

fn kern_node(dim: Measurement) -> MathMlNode {
    // Convert to em with a small unit table; unknown units fall to 0.
    let em = match dim.unit {
        crate::units::Unit::Em => dim.number,
        crate::units::Unit::Ex => dim.number * 0.4305555,
        crate::units::Unit::Mu => dim.number / 18.0,
        _ => 0.0,
    };
    MathMlNode::Space(em)
}

fn raisebox_node(body: &ParseNode, dy: Measurement, options: &Options) -> MathMlNode {
    let mut buf = String::new();
    let _ = write!(buf, "{}{}", dy.number, dy.unit.as_str());
    let mut el = MathMlElement::with_children("mpadded", vec![build_group(body, options)]);
    el.set_attribute("voffset", buf);
    el.into_node()
}

fn rule_node(width: Measurement, height: Measurement) -> MathMlNode {
    let mut el = MathMlElement::new("mspace");
    let mut w = String::new();
    let _ = write!(w, "{}{}", width.number, width.unit.as_str());
    let mut h = String::new();
    let _ = write!(h, "{}{}", height.number, height.unit.as_str());
    el.set_attribute("mathbackground", "black");
    el.set_attribute("width", w);
    el.set_attribute("height", h);
    el.into_node()
}

fn mphantom_node(body: &[ParseNode], options: &Options) -> MathMlNode {
    let inner = build_expression(body, options, false);
    MathMlElement::with_children(
        "mphantom",
        vec![MathMlElement::with_children("mrow", inner).into_node()],
    )
    .into_node()
}

fn mphantom_one(body: &ParseNode, options: &Options) -> MathMlNode {
    MathMlElement::with_children("mphantom", vec![build_group(body, options)]).into_node()
}

fn mpadded_zero_width(body: &ParseNode, options: &Options) -> MathMlNode {
    let mut el = MathMlElement::with_children("mpadded", vec![build_group(body, options)]);
    el.set_attribute("width", "0px");
    el.into_node()
}

// ----- Coloring / styling / fonts -----------------------------------

fn color_node(color: &str, body: &[ParseNode], options: &Options) -> MathMlNode {
    let inner_options = options.with_color(Some(SmolStr::new(color)));
    let inner = build_expression(body, &inner_options, false);
    let mut el = MathMlElement::with_children(
        "mstyle",
        vec![MathMlElement::with_children("mrow", inner).into_node()],
    );
    el.set_attribute("mathcolor", color);
    el.into_node()
}

fn styling_node(style: StyleStr, body: &[ParseNode], options: &Options) -> MathMlNode {
    let new_style = match style {
        StyleStr::Display => Style::Display,
        StyleStr::Text => Style::Text,
        StyleStr::Script => Style::Script,
        StyleStr::ScriptScript => Style::ScriptScript,
    };
    let inner_options = options.having_style(new_style);
    let inner = build_expression(body, &inner_options, false);
    // Upstream's `<mstyle>` carries the children directly (no inner
    // `<mrow>`).
    let mut el = MathMlElement::with_children("mstyle", inner);
    let (display, level) = match style {
        StyleStr::Display => ("true", "0"),
        StyleStr::Text => ("false", "0"),
        StyleStr::Script => ("false", "1"),
        StyleStr::ScriptScript => ("false", "2"),
    };
    el.set_attribute("scriptlevel", level);
    el.set_attribute("displaystyle", display);
    el.into_node()
}

fn text_node(body: &[ParseNode], font: Option<&str>, options: &Options) -> MathMlNode {
    let inner_options = match font {
        Some(f) => options.with_font(Some(SmolStr::new(f))),
        None => options.clone(),
    };
    let inner = build_expression(body, &inner_options, false);
    if let Some(text) = collect_simple_text(&inner) {
        return MathMlElement::with_children("mtext", vec![MathMlNode::Text(text)]).into_node();
    }
    MathMlElement::with_children("mrow", inner).into_node()
}

fn verb_node(body: &str, _star: bool) -> MathMlNode {
    let mut el = MathMlElement::with_children("mtext", vec![MathMlNode::Text(body.to_string())]);
    el.set_attribute("mathvariant", "monospace");
    el.into_node()
}

fn font_node(font: &str, body: &ParseNode, options: &Options) -> MathMlNode {
    let inner_options = options.with_font(Some(SmolStr::new(font)));
    let inner = build_group(body, &inner_options);
    if let Some(variant) = font_to_mathvariant(font) {
        let mut el = MathMlElement::with_children("mstyle", vec![inner]);
        el.set_attribute("mathvariant", variant);
        return el.into_node();
    }
    inner
}

fn pmb_node(body: &[ParseNode], options: &Options) -> MathMlNode {
    let inner = build_expression(body, options, false);
    let mut el = MathMlElement::with_children(
        "mstyle",
        vec![MathMlElement::with_children("mrow", inner).into_node()],
    );
    el.set_attribute("style", "text-shadow: 0.02em 0.01em 0.04em");
    el.into_node()
}

// ----- Operators ----------------------------------------------------

fn op_node(
    _limits: bool,
    _always_handle_supsub: bool,
    _parent_is_supsub: bool,
    body: &OpBody,
    options: &Options,
) -> MathMlNode {
    match body {
        OpBody::Symbol(name) => {
            // Upstream: if the symbol has a unicode replacement (e.g.
            // `\sum` -> `∑`), emit that; otherwise the operator is one
            // of the named ops (`\sin`, `\cos`, `\lim`, …) — strip the
            // leading backslash and emit the bare name. Without this
            // strip, the MathML output reads `<mo>\sin</mo>` literally.
            let text = match SYMBOLS.get((Mode::Math, name)).and_then(|i| i.replace) {
                Some(replacement) => replacement.to_string(),
                None => name.strip_prefix('\\').unwrap_or(name).to_string(),
            };
            MathMlElement::with_children("mo", vec![MathMlNode::Text(text)]).into_node()
        }
        OpBody::Composite(children) => {
            let inner = build_expression(children, options, false);
            MathMlElement::with_children("mo", inner).into_node()
        }
    }
}

fn operatorname_node(
    body: &[ParseNode],
    _limits: bool,
    _parent_is_supsub: bool,
    _always_handle_supsub: bool,
    options: &Options,
) -> MathMlNode {
    // operatorname renders in upright form. We map the sub-expression
    // through `mathrm` font and emit as an `<mi>` — upstream emits an
    // `<mo>` inside a `<mrow>` with `lspace=0 rspace=0`; the `<mi>` form
    // is functionally equivalent for static MathML readers.
    let inner_options = options.with_font(Some(SmolStr::new("mathrm")));
    let inner = build_expression(body, &inner_options, false);
    if let Some(text) = collect_simple_text(&inner) {
        let mut el = MathMlElement::with_children("mi", vec![MathMlNode::Text(text)]);
        el.set_attribute("mathvariant", "normal");
        return el.into_node();
    }
    MathMlElement::with_children("mrow", inner).into_node()
}

/// If every node in `nodes` is a [`MathMlNode::Text`] or a one-text-child
/// `<mi>` / `<mn>` / `<mo>` element, returns the concatenated text.
/// Used by [`text_node`] / [`operatorname_node`] to collapse e.g.
/// `\\text{abc}` to `<mtext>abc</mtext>` instead of three `<mi>`s.
fn collect_simple_text(nodes: &[MathMlNode]) -> Option<String> {
    let mut text = String::new();
    for node in nodes {
        match node {
            MathMlNode::Text(s) => text.push_str(s),
            MathMlNode::Element(el)
                if (el.tag == "mi" || el.tag == "mn" || el.tag == "mo")
                    && el.children.len() == 1 =>
            {
                match &el.children[0] {
                    MathMlNode::Text(s) => text.push_str(s),
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    Some(text)
}

// ----- Accents and over/underline -----------------------------------

fn accent_node(
    label: &str,
    base: &ParseNode,
    is_stretchy: bool,
    is_over: bool,
    options: &Options,
) -> MathMlNode {
    let base_node = build_group(base, options);
    let resolved = resolve_symbol(Mode::Math, label);
    let mut accent = MathMlElement::with_children("mo", vec![MathMlNode::Text(resolved)]);
    accent.set_attribute("stretchy", if is_stretchy { "true" } else { "false" });
    let tag = if is_over { "mover" } else { "munder" };
    let mut wrapper = MathMlElement::with_children(tag, vec![base_node, accent.into_node()]);
    wrapper.set_attribute("accent", "true");
    wrapper.into_node()
}

fn mathml_one_arg_op(
    tag: &str,
    body: &ParseNode,
    operator: &str,
    accent_above: bool,
    options: &Options,
) -> MathMlNode {
    let _ = accent_above;
    let body_node = build_group(body, options);
    let op = MathMlElement::with_children("mo", vec![MathMlNode::Text(operator.to_string())]);
    MathMlElement::with_children(tag, vec![body_node, op.into_node()]).into_node()
}

fn xarrow_node(
    label: &str,
    body: &ParseNode,
    below: Option<&ParseNode>,
    options: &Options,
) -> MathMlNode {
    let resolved = resolve_symbol(Mode::Math, label);
    let arrow = MathMlElement::with_children("mo", vec![MathMlNode::Text(resolved)]).into_node();
    let body_node = build_group(body, options);
    match below {
        Some(b) => MathMlElement::with_children(
            "munderover",
            vec![arrow, build_group(b, options), body_node],
        )
        .into_node(),
        None => MathMlElement::with_children("mover", vec![arrow, body_node]).into_node(),
    }
}

// ----- Misc ---------------------------------------------------------

fn href_node(body: &[ParseNode], href: &str, options: &Options) -> MathMlNode {
    let inner = build_expression(body, options, false);
    MathMlElement::with_children("mrow", inner)
        .with_attribute("href", href)
        .into_node()
}

fn enclose_node(label: &str, body: &ParseNode, options: &Options) -> MathMlNode {
    let inner = build_group(body, options);
    let mut el = MathMlElement::with_children("menclose", vec![inner]);
    let notation = match label {
        "\\cancel" => "updiagonalstrike",
        "\\bcancel" => "downdiagonalstrike",
        "\\xcancel" => "updiagonalstrike downdiagonalstrike",
        "\\sout" => "horizontalstrike",
        "\\fbox" | "\\boxed" => "box",
        "\\angl" => "actuarial",
        "\\phase" => "phasorangle",
        _ => "",
    };
    if !notation.is_empty() {
        el.set_attribute("notation", notation);
    }
    el.into_node()
}

fn tag_node(body: &[ParseNode], tag: &[ParseNode], options: &Options) -> MathMlNode {
    let body_inner = build_expression(body, options, false);
    let tag_inner = build_expression(tag, options, false);
    let row = MathMlElement::with_children(
        "mtr",
        vec![
            MathMlElement::with_children("mtd", body_inner).into_node(),
            MathMlElement::with_children("mtd", tag_inner).into_node(),
        ],
    )
    .into_node();
    MathMlElement::with_children("mtable", vec![row]).into_node()
}

fn mathchoice_node(
    display: &[ParseNode],
    text: &[ParseNode],
    script: &[ParseNode],
    scriptscript: &[ParseNode],
    options: &Options,
) -> MathMlNode {
    let chosen: &[ParseNode] = match options.style {
        Style::Display | Style::DisplayCramped => display,
        Style::Text | Style::TextCramped => text,
        Style::Script | Style::ScriptCramped => script,
        Style::ScriptScript | Style::ScriptScriptCramped => scriptscript,
    };
    ordgroup_node(chosen, options)
}

fn mtext(s: &str) -> MathMlNode {
    MathMlElement::with_children("mtext", vec![MathMlNode::Text(s.to_string())]).into_node()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use crate::settings::Settings;

    fn parse(input: &str) -> Vec<ParseNode> {
        let s = Settings::default();
        let mut p = Parser::new(input.to_string(), &s);
        p.parse().expect("parse")
    }

    fn render(input: &str) -> String {
        let s = Settings::default();
        let tree = parse(input);
        let opts = Options::root_for(&s);
        build_math_ml(&tree, input, &opts, &s, false).to_markup()
    }

    #[test]
    fn frac_one_two_renders_mfrac() {
        let mml = render("\\frac{1}{2}");
        assert!(mml.contains("<mfrac>"), "missing mfrac in: {mml}");
        assert!(mml.contains("<mn>1</mn>"));
        assert!(mml.contains("<mn>2</mn>"));
        assert!(
            mml.starts_with("<math xmlns=\"http://www.w3.org/1998/Math/MathML\">"),
            "missing root: {mml}"
        );
        assert!(
            mml.contains("<annotation encoding=\"application/x-tex\">\\frac{1}{2}</annotation>"),
            "missing annotation: {mml}"
        );
    }

    #[test]
    fn x_squared_renders_msup() {
        let mml = render("x^2");
        assert!(mml.contains("<msup>"), "missing msup in: {mml}");
        assert!(mml.contains("<mi>x</mi>"));
        assert!(mml.contains("<mn>2</mn>"));
    }

    #[test]
    fn alpha_renders_as_unicode() {
        let mml = render("\\alpha");
        assert!(mml.contains("<mi>α</mi>"), "got: {mml}");
    }

    #[test]
    fn sqrt_renders_msqrt() {
        let mml = render("\\sqrt{x}");
        assert!(mml.contains("<msqrt>"), "got: {mml}");
    }

    #[test]
    fn sqrt_with_index_renders_mroot() {
        let mml = render("\\sqrt[3]{x}");
        assert!(mml.contains("<mroot>"), "got: {mml}");
    }

    #[test]
    fn display_mode_sets_attribute() {
        let s = Settings::builder().display_mode(true).build();
        let tree = parse("x");
        let opts = Options::root_for(&s);
        let mml = build_math_ml(&tree, "x", &opts, &s, false).to_markup();
        assert!(mml.contains("display=\"block\""));
    }

    #[test]
    fn empty_input_produces_empty_mrow() {
        let mml = render("");
        assert!(mml.contains("<mrow/>") || mml.contains("<mrow></mrow>"));
    }

    #[test]
    fn binom_wraps_in_paren_delimiters() {
        let mml = render("\\binom{n}{k}");
        assert!(mml.contains("<mfrac"));
        assert!(mml.contains("<mo"));
        assert!(mml.contains(">(<"));
        assert!(mml.contains(">)<"));
    }

    #[test]
    fn ordgroup_with_one_child_unwraps() {
        let mml = render("{x}");
        // Upstream wraps a single non-row-like child in an mrow; only
        // single-child `mrow`/`mtable` bypass the wrapper.
        assert!(
            mml.contains("<mrow><mi>x</mi></mrow><annotation"),
            "got: {mml}"
        );
    }

    #[test]
    fn annotation_escapes_special_chars() {
        let mml = render("a < b");
        assert!(mml.contains("a &lt; b"), "got: {mml}");
    }
}

//! `array`-family environments. Mirrors upstream KaTeX's
//! `environments/array.ts` (which itself bundles `array`, `matrix`,
//! `cases`, `aligned`, `gathered`, `alignat`, `equation`, and the CD
//! shell into one file). The MathML builder for [`ParseNode::Array`]
//! also lives here — same locality as upstream.

use smol_str::SmolStr;

use crate::define_environment::{EnvContext, EnvSpec};
use crate::define_function::MathmlBuilder;
use crate::macro_expander::MacroDefinition;
use crate::mathml_tree::{MathMlElement, MathMlNode};
use crate::options::Options;
use crate::parse_error::ParseError;
use crate::parse_node::{ArrayTag, HLineSpec, NodeType, ParseNode};
use crate::parser::Parser;
use crate::tree::{AlignSpec, BreakToken, ColSeparationType, StyleStr};
use crate::units::{Measurement, make_em};

use super::cd::parse_cd;

// ----- parseArray helpers --------------------------------------------

/// Configuration knobs passed to [`parse_array`]. Mirrors the second
/// argument of upstream's `parseArray`.
#[derive(Default, Clone)]
struct ParseArrayConfig {
    hskip_before_and_after: Option<bool>,
    add_jot: Option<bool>,
    cols: Option<Vec<AlignSpec>>,
    arraystretch: Option<f64>,
    col_separation_type: Option<ColSeparationType>,
    /// `Some(true)` = automatic numbering, `Some(false)` = manual tags
    /// allowed but no auto numbering, `None` = no tagging at all.
    auto_tag: Option<bool>,
    single_row: bool,
    empty_single_row: bool,
    max_num_cols: Option<usize>,
    leqno: Option<bool>,
}

/// Read consecutive `\hline` / `\hdashline` tokens. Each entry is `true`
/// for `\hdashline` and `false` for `\hline`. Mirrors upstream
/// `getHLines`.
fn get_h_lines(parser: &mut Parser<'_>) -> Result<Vec<bool>, ParseError> {
    let mut info: Vec<bool> = Vec::new();
    parser.consume_spaces()?;
    let mut nxt = parser.fetch()?.text.clone();
    if nxt.as_str() == "\\relax" {
        parser.consume();
        parser.consume_spaces()?;
        nxt = parser.fetch()?.text.clone();
    }
    while nxt.as_str() == "\\hline" || nxt.as_str() == "\\hdashline" {
        parser.consume();
        info.push(nxt.as_str() == "\\hdashline");
        parser.consume_spaces()?;
        nxt = parser.fetch()?.text.clone();
    }
    Ok(info)
}

fn validate_ams_environment_context(ctx: &EnvContext<'_, '_>) -> Result<(), ParseError> {
    if !ctx.parser.settings.display_mode {
        return Err(ParseError::new(format!(
            "{{{}}} can be used only in display mode.",
            ctx.env_name
        )));
    }
    Ok(())
}

/// `autoTag` from upstream: `Some(true)` (auto-numbered) when name has
/// no `*`; `Some(false)` (allowed manual, no auto) when it has `*`;
/// `None` (no tagging) when the name contains `ed` (e.g. `aligned`).
fn get_auto_tag(name: &str) -> Option<bool> {
    if name.contains("ed") {
        None
    } else {
        Some(!name.contains('*'))
    }
}

fn d_cell_style(env_name: &str) -> StyleStr {
    if env_name.starts_with('d') {
        StyleStr::Display
    } else {
        StyleStr::Text
    }
}

/// Body of `parseArray`. Returns a fully-built [`ParseNode::Array`].
fn parse_array(
    parser: &mut Parser<'_>,
    cfg: ParseArrayConfig,
    style: StyleStr,
) -> Result<ParseNode, ParseError> {
    let ParseArrayConfig {
        hskip_before_and_after,
        add_jot,
        mut cols,
        arraystretch,
        col_separation_type,
        auto_tag,
        single_row,
        empty_single_row,
        max_num_cols,
        leqno,
    } = cfg;

    parser.gullet.begin_group();
    if !single_row {
        // \cr is a synonym for \\ inside arrays. Map it to a `\\` token
        // followed by `\relax`. We do not depend on `\relax` having a
        // function handler — getHLines silently consumes it.
        parser.gullet.macros.set(
            "\\cr",
            Some(MacroDefinition::Source(SmolStr::new_static("\\\\\\relax"))),
            false,
        );
    }

    let arraystretch = match arraystretch {
        Some(v) => v,
        None => match parser.gullet.expand_macro_as_text("\\arraystretch")? {
            None => 1.0,
            Some(text) => {
                let parsed: f64 = text.trim().parse().unwrap_or(0.0);
                if parsed <= 0.0 {
                    return Err(ParseError::new(format!("Invalid \\arraystretch: {text}")));
                }
                parsed
            }
        },
    };

    // Start group for first cell
    parser.gullet.begin_group();

    let mode = parser.mode;
    let mut row: Vec<ParseNode> = Vec::new();
    let mut body: Vec<Vec<ParseNode>> = Vec::new();
    body.push(Vec::new());
    let mut row_gaps: Vec<Option<Measurement>> = Vec::new();
    let mut h_lines_before_row: Vec<HLineSpec> = Vec::new();
    let mut tags: Option<Vec<ArrayTag>> = if auto_tag.is_some() {
        Some(Vec::new())
    } else {
        None
    };

    let begin_row = |parser: &mut Parser<'_>| {
        if matches!(auto_tag, Some(true)) {
            parser.gullet.macros.set(
                "\\@eqnsw",
                Some(MacroDefinition::Source(SmolStr::new_static("1"))),
                true,
            );
        }
    };

    fn end_row(
        parser: &mut Parser<'_>,
        tags: &mut Option<Vec<ArrayTag>>,
        auto_tag: Option<bool>,
    ) -> Result<(), ParseError> {
        let Some(tags_vec) = tags else {
            return Ok(());
        };
        // Manual tag captured by \tag inside the row?
        if parser.gullet.macros.get("\\df@tag").is_some() {
            // Subparse the macro body to materialise the tag.
            let body = parser.subparse(vec![crate::token::Token::new(
                SmolStr::new_static("\\df@tag"),
                None,
            )])?;
            tags_vec.push(ArrayTag::Explicit(body));
            parser
                .gullet
                .macros
                .set("\\df@tag", None::<MacroDefinition>, true);
        } else {
            let auto = matches!(auto_tag, Some(true))
                && parser
                    .gullet
                    .macros
                    .get("\\@eqnsw")
                    .map(|d| matches!(d, MacroDefinition::Source(s) if s.as_str() == "1"))
                    .unwrap_or(false);
            if auto {
                tags_vec.push(ArrayTag::Auto);
            } else {
                tags_vec.push(ArrayTag::None);
            }
        }
        Ok(())
    }

    // `body[0]` is the live row slot; we keep `row` as the buffer to
    // push cells into, and copy it into `body.last_mut()` whenever a
    // row separator (`\\`) or `\end` is reached.
    body.clear();
    body.push(Vec::new());
    begin_row(parser);

    h_lines_before_row.push(get_h_lines(parser)?);

    loop {
        let break_on = if single_row {
            BreakToken::End
        } else {
            BreakToken::DoubleBackslash
        };
        let cell_body = parser.parse_expression(false, Some(break_on))?;
        parser.gullet.end_group()?;
        parser.gullet.begin_group();

        let mode = parser.mode;
        let mut cell: ParseNode = ParseNode::OrdGroup {
            mode,
            loc: None,
            body: cell_body,
            semisimple: false,
        };
        cell = ParseNode::Styling {
            mode,
            loc: None,
            style,
            body: vec![cell],
        };
        row.push(cell);

        let next_text = parser.fetch()?.text.clone();
        if next_text.as_str() == "&" {
            if let Some(max) = max_num_cols
                && row.len() == max
                && (single_row || col_separation_type.is_some())
            {
                return Err(ParseError::new("Too many tab characters: &"));
            }
            // For the `{array}` environment upstream calls
            // `reportNonstrict("textEnv", ...)` when over-running `max`;
            // we currently accept the extra column silently. Strict-mode
            // reporting is wired up in a later phase.
            parser.consume();
        } else if next_text.as_str() == "\\end" {
            end_row(parser, &mut tags, auto_tag)?;
            // Drop a trailing empty row exactly as upstream does. The
            // last cell pushed is a styling-wrapped empty ordgroup.
            let last_cell_is_empty = match row.last() {
                Some(ParseNode::Styling { body: inner, .. }) if inner.len() == 1 => {
                    matches!(
                        &inner[0],
                        ParseNode::OrdGroup { body, .. } if body.is_empty()
                    )
                }
                _ => false,
            };
            let drop_last = !body.is_empty()
                && row.len() == 1
                && last_cell_is_empty
                && (body.len() > 1 || !empty_single_row);

            if drop_last {
                row.clear();
            }
            // Push the (possibly emptied) `row` into `body`.
            if let Some(last) = body.last_mut() {
                *last = std::mem::take(&mut row);
            }
            if drop_last && !body.is_empty() {
                body.pop();
            }

            if h_lines_before_row.len() < body.len() + 1 {
                h_lines_before_row.push(Vec::new());
            }
            break;
        } else if next_text.as_str() == "\\\\" {
            parser.consume();
            let size = if parser.gullet.future()?.text.as_str() != " " {
                match parser.parse_size_group(true)? {
                    Some(ParseNode::Size {
                        value, is_blank, ..
                    }) if !is_blank => Some(value),
                    _ => None,
                }
            } else {
                None
            };
            row_gaps.push(size);
            end_row(parser, &mut tags, auto_tag)?;

            // Commit current row into body, then start a fresh row.
            if let Some(slot) = body.last_mut() {
                *slot = std::mem::take(&mut row);
            }

            h_lines_before_row.push(get_h_lines(parser)?);

            body.push(Vec::new());
            begin_row(parser);
        } else {
            return Err(ParseError::new("Expected & or \\\\ or \\cr or \\end"));
        }
    }

    // End cell group
    parser.gullet.end_group()?;
    // End array group defining \cr
    parser.gullet.end_group()?;

    // Some environments (matrix family) post-process `cols` after parse.
    let _ = (&mut cols, &h_lines_before_row);

    Ok(ParseNode::Array {
        mode,
        loc: None,
        col_separation_type,
        hskip_before_and_after,
        add_jot,
        cols,
        arraystretch,
        body,
        row_gaps,
        h_lines_before_row,
        tags,
        leqno,
        is_cd: None,
    })
}

// ----- Per-environment handlers --------------------------------------

/// `\begin{array}` / `\begin{darray}`: column spec is the first arg.
fn array_handler(
    ctx: EnvContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let arg0 = args
        .first()
        .ok_or_else(|| ParseError::new("array environment requires a column-spec argument"))?;
    let colalign: Vec<&ParseNode> = match arg0 {
        ParseNode::OrdGroup { body, .. } => body.iter().collect(),
        sym if sym.is_symbol_node() => vec![sym],
        _ => return Err(ParseError::new("Invalid column alignment")),
    };
    let mut cols: Vec<AlignSpec> = Vec::with_capacity(colalign.len());
    for nde in colalign {
        let ca: SmolStr = match nde {
            ParseNode::Atom { text, .. }
            | ParseNode::MathOrd { text, .. }
            | ParseNode::TextOrd { text, .. }
            | ParseNode::Spacing { text, .. }
            | ParseNode::AccentToken { text, .. }
            | ParseNode::OpToken { text, .. } => text.clone(),
            _ => return Err(ParseError::new("Unknown column alignment")),
        };
        let s = ca.as_str();
        if s == "l" || s == "c" || s == "r" {
            cols.push(AlignSpec::Align {
                align: ca,
                pregap: None,
                postgap: None,
            });
        } else if s == "|" {
            cols.push(AlignSpec::Separator {
                separator: SmolStr::new_static("|"),
            });
        } else if s == ":" {
            cols.push(AlignSpec::Separator {
                separator: SmolStr::new_static(":"),
            });
        } else {
            return Err(ParseError::new(format!("Unknown column alignment: {s}")));
        }
    }
    let max_num_cols = cols.len();
    let cfg = ParseArrayConfig {
        cols: Some(cols),
        hskip_before_and_after: Some(true),
        max_num_cols: Some(max_num_cols),
        ..Default::default()
    };
    let style = d_cell_style(ctx.env_name);
    parse_array(ctx.parser, cfg, style)
}

/// matrix / pmatrix / bmatrix / Bmatrix / vmatrix / Vmatrix (and the
/// `*`-suffixed mathtools variants).
fn matrix_handler(
    ctx: EnvContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let env_mode = ctx.parser.mode;
    let env_name = ctx.env_name.to_string();
    let stripped: String = env_name.replace('*', "");
    let delimiters: Option<(&'static str, &'static str)> = match stripped.as_str() {
        "matrix" => None,
        "pmatrix" => Some(("(", ")")),
        "bmatrix" => Some(("[", "]")),
        "Bmatrix" => Some(("\\{", "\\}")),
        "vmatrix" => Some(("|", "|")),
        "Vmatrix" => Some(("\\Vert", "\\Vert")),
        _ => None,
    };
    let mut col_align = SmolStr::new_static("c");
    let mut payload = ParseArrayConfig {
        hskip_before_and_after: Some(false),
        cols: Some(vec![AlignSpec::Align {
            align: col_align.clone(),
            pregap: None,
            postgap: None,
        }]),
        ..Default::default()
    };

    if env_name.ends_with('*') {
        // mathtools starred — optional `[lcr]` alignment argument.
        ctx.parser.consume_spaces()?;
        if ctx.parser.fetch()?.text.as_str() == "[" {
            ctx.parser.consume();
            ctx.parser.consume_spaces()?;
            let align_text = ctx.parser.fetch()?.text.clone();
            if !matches!(align_text.as_str(), "l" | "c" | "r") {
                return Err(ParseError::new("Expected l or c or r"));
            }
            col_align = align_text;
            ctx.parser.consume();
            ctx.parser.consume_spaces()?;
            ctx.parser.expect("]", true)?;
            payload.cols = Some(vec![AlignSpec::Align {
                align: col_align.clone(),
                pregap: None,
                postgap: None,
            }]);
        }
    }
    let style = d_cell_style(&env_name);
    let mut res = parse_array(ctx.parser, payload, style)?;

    // Populate cols with the correct number of column alignment specs.
    if let ParseNode::Array { body, cols, .. } = &mut res {
        let num_cols = body.iter().map(|r| r.len()).max().unwrap_or(0);
        let new_cols: Vec<AlignSpec> = (0..num_cols)
            .map(|_| AlignSpec::Align {
                align: col_align.clone(),
                pregap: None,
                postgap: None,
            })
            .collect();
        *cols = Some(new_cols);
    }

    Ok(match delimiters {
        Some((l, r)) => ParseNode::LeftRight {
            mode: env_mode,
            loc: None,
            body: vec![res],
            left: SmolStr::new_static(l),
            right: SmolStr::new_static(r),
            right_color: None,
        },
        None => res,
    })
}

fn smallmatrix_handler(
    ctx: EnvContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let cfg = ParseArrayConfig {
        arraystretch: Some(0.5),
        col_separation_type: Some(ColSeparationType::Small),
        ..Default::default()
    };
    parse_array(ctx.parser, cfg, StyleStr::Script)
}

fn subarray_handler(
    ctx: EnvContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let arg0 = args
        .first()
        .ok_or_else(|| ParseError::new("subarray environment requires a column-spec argument"))?;
    let colalign: Vec<&ParseNode> = match arg0 {
        ParseNode::OrdGroup { body, .. } => body.iter().collect(),
        sym if sym.is_symbol_node() => vec![sym],
        _ => return Err(ParseError::new("Invalid column alignment")),
    };
    let mut cols: Vec<AlignSpec> = Vec::with_capacity(colalign.len());
    for nde in colalign {
        let ca: SmolStr = match nde {
            ParseNode::Atom { text, .. }
            | ParseNode::MathOrd { text, .. }
            | ParseNode::TextOrd { text, .. }
            | ParseNode::Spacing { text, .. }
            | ParseNode::AccentToken { text, .. }
            | ParseNode::OpToken { text, .. } => text.clone(),
            _ => return Err(ParseError::new("Unknown column alignment")),
        };
        let s = ca.as_str();
        if s == "l" || s == "c" {
            cols.push(AlignSpec::Align {
                align: ca,
                pregap: None,
                postgap: None,
            });
        } else {
            return Err(ParseError::new(format!("Unknown column alignment: {s}")));
        }
    }
    if cols.len() > 1 {
        return Err(ParseError::new("{subarray} can contain only one column"));
    }
    let cfg = ParseArrayConfig {
        cols: Some(cols),
        hskip_before_and_after: Some(false),
        arraystretch: Some(0.5),
        ..Default::default()
    };
    let res = parse_array(ctx.parser, cfg, StyleStr::Script)?;
    if let ParseNode::Array { body, .. } = &res
        && let Some(first) = body.first()
        && first.len() > 1
    {
        return Err(ParseError::new("{subarray} can contain only one column"));
    }
    Ok(res)
}

fn cases_handler(
    ctx: EnvContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    let env_name = ctx.env_name.to_string();
    let env_mode = ctx.parser.mode;
    let cfg = ParseArrayConfig {
        arraystretch: Some(1.2),
        cols: Some(vec![
            AlignSpec::Align {
                align: SmolStr::new_static("l"),
                pregap: Some(0.0),
                postgap: Some(1.0),
            },
            AlignSpec::Align {
                align: SmolStr::new_static("l"),
                pregap: Some(0.0),
                postgap: Some(0.0),
            },
        ]),
        ..Default::default()
    };
    let style = d_cell_style(&env_name);
    let res = parse_array(ctx.parser, cfg, style)?;
    let (left, right) = if env_name.contains('r') {
        (SmolStr::new_static("."), SmolStr::new_static("\\}"))
    } else {
        (SmolStr::new_static("\\{"), SmolStr::new_static("."))
    };
    Ok(ParseNode::LeftRight {
        mode: env_mode,
        loc: None,
        body: vec![res],
        left,
        right,
        right_color: None,
    })
}

fn aligned_handler(
    ctx: EnvContext<'_, '_>,
    args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    if !ctx.env_name.contains("ed") {
        validate_ams_environment_context(&ctx)?;
    }
    let env_name = ctx.env_name.to_string();
    let separation_type = if env_name.contains("at") {
        ColSeparationType::Alignat
    } else {
        ColSeparationType::Align
    };
    let is_split = env_name == "split";
    let cfg = ParseArrayConfig {
        cols: Some(Vec::new()),
        add_jot: Some(true),
        auto_tag: if is_split {
            None
        } else {
            get_auto_tag(&env_name)
        },
        empty_single_row: true,
        col_separation_type: Some(separation_type),
        max_num_cols: if is_split { Some(2) } else { None },
        leqno: Some(ctx.parser.settings.leqno),
        ..Default::default()
    };
    let mut res = parse_array(ctx.parser, cfg, StyleStr::Display)?;

    // Determine the alignment columns. Same logic as upstream.
    let mut num_maths: usize = 0;
    let mut num_cols: usize = 0;
    let env_mode = ctx.parser.mode;
    let empty_group = ParseNode::OrdGroup {
        mode: env_mode,
        loc: None,
        body: Vec::new(),
        semisimple: false,
    };
    if let Some(arg0) = args.first()
        && let ParseNode::OrdGroup { body, .. } = arg0
    {
        let mut digits = String::new();
        for n in body {
            match n {
                ParseNode::TextOrd { text, .. } => digits.push_str(text),
                _ => return Err(ParseError::new("Expected digits as alignat argument")),
            }
        }
        num_maths = digits.parse().unwrap_or(0);
        num_cols = num_maths * 2;
    }
    let is_aligned = num_cols == 0;
    if let ParseNode::Array { body, .. } = &mut res {
        for row in body.iter_mut() {
            // Prepend an empty group at every odd index so operators
            // become binary, mirroring amsmath's `\start@aligned`.
            let mut i = 1;
            while i < row.len() {
                if let ParseNode::Styling { body: inner, .. } = &mut row[i]
                    && let Some(ParseNode::OrdGroup { body: og_body, .. }) = inner.first_mut()
                {
                    og_body.insert(0, empty_group.clone());
                }
                i += 2;
            }
            if !is_aligned {
                let cur_maths = row.len().div_ceil(2);
                if num_maths < cur_maths {
                    return Err(ParseError::new(format!(
                        "Too many math in a row: expected {num_maths}, but got {cur_maths}"
                    )));
                }
            } else if num_cols < row.len() {
                num_cols = row.len();
            }
        }
    }

    if let ParseNode::Array {
        cols,
        col_separation_type,
        ..
    } = &mut res
    {
        let mut new_cols: Vec<AlignSpec> = Vec::with_capacity(num_cols);
        for i in 0..num_cols {
            let (align, pregap) = if i % 2 == 1 {
                ("l", 0.0)
            } else if i > 0 && is_aligned {
                ("r", 1.0)
            } else {
                ("r", 0.0)
            };
            new_cols.push(AlignSpec::Align {
                align: SmolStr::new_static(align),
                pregap: Some(pregap),
                postgap: Some(0.0),
            });
        }
        *cols = Some(new_cols);
        *col_separation_type = Some(if is_aligned {
            ColSeparationType::Align
        } else {
            ColSeparationType::Alignat
        });
    }
    Ok(res)
}

fn gathered_handler(
    ctx: EnvContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    if matches!(ctx.env_name, "gather" | "gather*") {
        validate_ams_environment_context(&ctx)?;
    }
    let cfg = ParseArrayConfig {
        cols: Some(vec![AlignSpec::Align {
            align: SmolStr::new_static("c"),
            pregap: None,
            postgap: None,
        }]),
        add_jot: Some(true),
        col_separation_type: Some(ColSeparationType::Gather),
        auto_tag: get_auto_tag(ctx.env_name),
        empty_single_row: true,
        leqno: Some(ctx.parser.settings.leqno),
        ..Default::default()
    };
    parse_array(ctx.parser, cfg, StyleStr::Display)
}

fn equation_handler(
    ctx: EnvContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    validate_ams_environment_context(&ctx)?;
    let cfg = ParseArrayConfig {
        auto_tag: get_auto_tag(ctx.env_name),
        empty_single_row: true,
        single_row: true,
        max_num_cols: Some(1),
        leqno: Some(ctx.parser.settings.leqno),
        ..Default::default()
    };
    parse_array(ctx.parser, cfg, StyleStr::Display)
}

fn cd_handler(
    ctx: EnvContext<'_, '_>,
    _args: &[ParseNode],
    _opt_args: &[Option<ParseNode>],
) -> Result<ParseNode, ParseError> {
    validate_ams_environment_context(&ctx)?;
    parse_cd(ctx.parser)
}

// ----- MathML builder for ParseNode::Array ---------------------------

const ALIGN_MAP: &[(&str, &str)] = &[("c", "center "), ("l", "left "), ("r", "right ")];

fn align_str_for(c: &str) -> &'static str {
    for (key, v) in ALIGN_MAP {
        if *key == c {
            return v;
        }
    }
    "center "
}

pub fn array_mathml_builder(group: &ParseNode, options: &Options) -> MathMlNode {
    let ParseNode::Array {
        body,
        row_gaps: _,
        h_lines_before_row,
        cols,
        col_separation_type,
        arraystretch,
        add_jot,
        tags,
        leqno,
        ..
    } = group
    else {
        return MathMlElement::new("mtable").into_node();
    };

    let glue_cell = || {
        MathMlElement::new("mtd")
            .with_attribute("class", "mtr-glue")
            .into_node()
    };
    let tag_cell = || {
        MathMlElement::new("mtd")
            .with_attribute("class", "mml-eqn-num")
            .into_node()
    };

    let mut tbl: Vec<MathMlNode> = Vec::with_capacity(body.len());
    for (i, rw) in body.iter().enumerate() {
        let mut row_children: Vec<MathMlNode> = rw
            .iter()
            .map(|cell| {
                MathMlElement::with_children(
                    "mtd",
                    vec![crate::build_mathml::build_group(cell, options)],
                )
                .into_node()
            })
            .collect();
        let row_has_tag = tags
            .as_ref()
            .and_then(|t| t.get(i))
            .map(|t| !matches!(t, ArrayTag::None))
            .unwrap_or(false);
        if row_has_tag {
            row_children.insert(0, glue_cell());
            row_children.push(glue_cell());
            if matches!(leqno, Some(true)) {
                row_children.insert(0, tag_cell());
            } else {
                row_children.push(tag_cell());
            }
        }
        tbl.push(MathMlElement::with_children("mtr", row_children).into_node());
    }
    let mut table = MathMlElement::with_children("mtable", tbl);

    // Row spacing.
    let gap = if (*arraystretch - 0.5).abs() < f64::EPSILON {
        0.1
    } else {
        0.16 + arraystretch - 1.0
            + if matches!(add_jot, Some(true)) {
                0.09
            } else {
                0.0
            }
    };
    table.set_attribute("rowspacing", make_em(gap));

    // Column alignment / column-lines / menclose.
    let mut menclose = String::new();
    let mut align = String::new();

    if let Some(cols_vec) = cols
        && !cols_vec.is_empty()
    {
        let mut column_lines = String::new();
        let mut prev_was_align = false;
        let mut i_start = 0usize;
        let mut i_end = cols_vec.len();
        if matches!(cols_vec[0], AlignSpec::Separator { .. }) {
            menclose.push_str("top ");
            i_start = 1;
        }
        if matches!(cols_vec.last(), Some(AlignSpec::Separator { .. })) {
            menclose.push_str("bottom ");
            i_end -= 1;
        }
        for col in &cols_vec[i_start..i_end] {
            match col {
                AlignSpec::Align { align: a, .. } => {
                    align.push_str(align_str_for(a.as_str()));
                    if prev_was_align {
                        column_lines.push_str("none ");
                    }
                    prev_was_align = true;
                }
                AlignSpec::Separator { separator } => {
                    if prev_was_align {
                        column_lines.push_str(if separator.as_str() == "|" {
                            "solid "
                        } else {
                            "dashed "
                        });
                        prev_was_align = false;
                    }
                }
            }
        }
        table.set_attribute("columnalign", align.trim().to_string());
        if column_lines.contains('s') || column_lines.contains('d') {
            table.set_attribute("columnlines", column_lines.trim().to_string());
        }
    }

    // Column spacing.
    match col_separation_type {
        Some(ColSeparationType::Align) => {
            let cols_len = cols.as_ref().map(|c| c.len()).unwrap_or(0);
            let mut spacing = String::new();
            for i in 1..cols_len {
                spacing.push_str(if i % 2 == 1 { "0em " } else { "1em " });
            }
            table.set_attribute("columnspacing", spacing.trim().to_string());
        }
        Some(ColSeparationType::Alignat) | Some(ColSeparationType::Gather) => {
            table.set_attribute("columnspacing", "0em");
        }
        Some(ColSeparationType::Small) => {
            table.set_attribute("columnspacing", "0.2778em");
        }
        Some(ColSeparationType::Cd) => {
            table.set_attribute("columnspacing", "0.5em");
        }
        _ => {
            table.set_attribute("columnspacing", "1em");
        }
    }

    // Row lines / menclose top/bottom.
    let mut row_lines = String::new();
    if let Some(first) = h_lines_before_row.first()
        && !first.is_empty()
    {
        menclose.push_str("left ");
    }
    if let Some(last) = h_lines_before_row.last()
        && !last.is_empty()
    {
        menclose.push_str("right ");
    }
    if h_lines_before_row.len() >= 2 {
        for hline in &h_lines_before_row[1..h_lines_before_row.len() - 1] {
            row_lines.push_str(if hline.is_empty() {
                "none "
            } else if hline[0] {
                "dashed "
            } else {
                "solid "
            });
        }
    }
    if row_lines.contains('s') || row_lines.contains('d') {
        table.set_attribute("rowlines", row_lines.trim().to_string());
    }

    let mut node = if menclose.is_empty() {
        table.into_node()
    } else {
        let mut menc = MathMlElement::with_children("menclose", vec![table.into_node()]);
        menc.set_attribute("notation", menclose.trim().to_string());
        menc.into_node()
    };

    if *arraystretch < 1.0 {
        let mut mstyle = MathMlElement::with_children("mstyle", vec![node]);
        mstyle.set_attribute("scriptlevel", "1");
        node = mstyle.into_node();
    }
    node
}

// ----- Spec table -----------------------------------------------------

const ARRAY_BUILDER: MathmlBuilder = array_mathml_builder;

const fn spec(
    names: &'static [&'static str],
    num_args: usize,
    handler: crate::define_environment::EnvHandler,
) -> EnvSpec {
    EnvSpec {
        node_type: NodeType::Array,
        names,
        num_args,
        arg_types: &[],
        allowed_in_text: false,
        num_optional_args: 0,
        handler,
        mathml_builder: Some(ARRAY_BUILDER),
    }
}

const ARRAY_NAMES: &[&str] = &["array", "darray"];
const MATRIX_NAMES: &[&str] = &[
    "matrix", "pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix", "matrix*", "pmatrix*",
    "bmatrix*", "Bmatrix*", "vmatrix*", "Vmatrix*",
];
const SMALLMATRIX_NAMES: &[&str] = &["smallmatrix"];
const SUBARRAY_NAMES: &[&str] = &["subarray"];
const CASES_NAMES: &[&str] = &["cases", "dcases", "rcases", "drcases"];
const ALIGN_NAMES: &[&str] = &["align", "align*", "aligned", "split"];
const GATHERED_NAMES: &[&str] = &["gathered", "gather", "gather*"];
const ALIGNAT_NAMES: &[&str] = &["alignat", "alignat*", "alignedat"];
const EQUATION_NAMES: &[&str] = &["equation", "equation*"];
const CD_NAMES: &[&str] = &["CD"];

pub const SPECS: &[EnvSpec] = &[
    spec(ARRAY_NAMES, 1, array_handler),
    spec(MATRIX_NAMES, 0, matrix_handler),
    spec(SMALLMATRIX_NAMES, 0, smallmatrix_handler),
    spec(SUBARRAY_NAMES, 1, subarray_handler),
    spec(CASES_NAMES, 0, cases_handler),
    spec(ALIGN_NAMES, 0, aligned_handler),
    spec(GATHERED_NAMES, 0, gathered_handler),
    spec(ALIGNAT_NAMES, 1, aligned_handler),
    spec(EQUATION_NAMES, 0, equation_handler),
    spec(CD_NAMES, 0, cd_handler),
];

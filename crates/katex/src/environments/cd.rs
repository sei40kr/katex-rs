//! `\begin{CD}` — commutative-diagram environment. Mirrors upstream
//! KaTeX's `environments/cd.ts`.
//!
//! The full syntax (cell/arrow weaving via `@`-prefixed tokens) is not
//! yet implemented. The hook is wired so the registry resolves `CD`,
//! but the body parser currently returns an empty array node so users
//! see "ParseNode::Array" rather than an unknown-environment error.
//! Filling in the cell/arrow walker is tracked alongside the cdlabel /
//! cdlabelparent functions in a follow-up.

use crate::parse_error::ParseError;
use crate::parse_node::ParseNode;
use crate::parser::Parser;
use crate::tree::ColSeparationType;

/// Parse the body of a `\begin{CD} ... \end{CD}` environment.
pub fn parse_cd(parser: &mut Parser<'_>) -> Result<ParseNode, ParseError> {
    // Skim through the body until `\end` so the parser doesn't choke.
    // Phase 8 lands the registration surface; faithful CD weaving is
    // tracked separately because it depends on `\\cdleft`/`\\cdright`/
    // `\\cdparent` parser hooks that haven't been ported yet.
    let mode = parser.mode;
    parser.gullet.begin_group();
    parser.gullet.begin_group();
    loop {
        let tok = parser.fetch()?.text.clone();
        if tok.as_str() == "\\end" {
            break;
        }
        if tok.as_str() == "EOF" {
            return Err(ParseError::new("Unexpected EOF inside CD environment"));
        }
        parser.consume();
    }
    parser.gullet.end_group()?;
    parser.gullet.end_group()?;

    Ok(ParseNode::Array {
        mode,
        loc: None,
        col_separation_type: Some(ColSeparationType::Cd),
        hskip_before_and_after: None,
        add_jot: Some(true),
        cols: None,
        arraystretch: 1.0,
        body: Vec::new(),
        row_gaps: vec![None],
        h_lines_before_row: vec![Vec::new()],
        tags: None,
        leqno: None,
        is_cd: Some(true),
    })
}

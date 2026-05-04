//! TeX/CSS measurement units.
//!
//! Mirrors upstream KaTeX's `units.ts`. Upstream models units as bare strings
//! validated against two dictionaries (`ptPerUnit` for absolute units,
//! `relativeUnit` for relative ones); we use a closed enum so that:
//! - invalid units are unrepresentable rather than runtime-checked, and
//! - the parser can `match` exhaustively over them.
//!
//! `pt-per-em` conversion (`calculateSize` upstream) requires `Options` and
//! `fontMetrics`, which land in Phase 6+; this module covers the leaf data
//! type and string parsing only.

use crate::parse_error::ParseError;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Unit {
    // Absolute units.
    /// TeX point.
    Pt,
    /// Millimeter.
    Mm,
    /// Centimeter.
    Cm,
    /// Inch.
    In,
    /// Big (PostScript) point.
    Bp,
    /// Pica.
    Pc,
    /// Didot.
    Dd,
    /// Cicero (12 didot).
    Cc,
    /// New didot.
    Nd,
    /// New cicero (12 new didot).
    Nc,
    /// Scaled point (TeX's internal smallest unit).
    Sp,
    /// CSS pixel; in upstream this defaults to 1 bp (`\pdfpxdimen`).
    Px,
    // Relative units.
    /// x-height of the current font.
    Ex,
    /// em of the current font.
    Em,
    /// mu — math unit; 1/18 em in textstyle, scales for script styles.
    Mu,
}

impl Unit {
    /// True for absolute units (those in upstream's `ptPerUnit` table).
    pub const fn is_absolute(self) -> bool {
        // Exhaustive `match` (rather than `matches!`) so adding a new
        // variant forces a compile error here.
        match self {
            Unit::Pt
            | Unit::Mm
            | Unit::Cm
            | Unit::In
            | Unit::Bp
            | Unit::Pc
            | Unit::Dd
            | Unit::Cc
            | Unit::Nd
            | Unit::Nc
            | Unit::Sp
            | Unit::Px => true,
            Unit::Ex | Unit::Em | Unit::Mu => false,
        }
    }

    /// Lowercase canonical TeX abbreviation, e.g. `Unit::Pt` -> `"pt"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Unit::Pt => "pt",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::In => "in",
            Unit::Bp => "bp",
            Unit::Pc => "pc",
            Unit::Dd => "dd",
            Unit::Cc => "cc",
            Unit::Nd => "nd",
            Unit::Nc => "nc",
            Unit::Sp => "sp",
            Unit::Px => "px",
            Unit::Ex => "ex",
            Unit::Em => "em",
            Unit::Mu => "mu",
        }
    }

    /// Number of TeX `pt` per one of this unit, for absolute units only.
    /// Numeric values copied verbatim from upstream `units.ts:ptPerUnit`.
    pub const fn pt_per_unit(self) -> Option<f64> {
        Some(match self {
            Unit::Pt => 1.0,
            Unit::Mm => 7227.0 / 2540.0,
            Unit::Cm => 7227.0 / 254.0,
            Unit::In => 72.27,
            Unit::Bp => 803.0 / 800.0,
            Unit::Pc => 12.0,
            Unit::Dd => 1238.0 / 1157.0,
            Unit::Cc => 14856.0 / 1157.0,
            Unit::Nd => 685.0 / 642.0,
            Unit::Nc => 1370.0 / 107.0,
            Unit::Sp => 1.0 / 65536.0,
            Unit::Px => 803.0 / 800.0,
            Unit::Ex | Unit::Em | Unit::Mu => return None,
        })
    }
}

impl std::str::FromStr for Unit {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pt" => Unit::Pt,
            "mm" => Unit::Mm,
            "cm" => Unit::Cm,
            "in" => Unit::In,
            "bp" => Unit::Bp,
            "pc" => Unit::Pc,
            "dd" => Unit::Dd,
            "cc" => Unit::Cc,
            "nd" => Unit::Nd,
            "nc" => Unit::Nc,
            "sp" => Unit::Sp,
            "px" => Unit::Px,
            "ex" => Unit::Ex,
            "em" => Unit::Em,
            "mu" => Unit::Mu,
            other => return Err(ParseError::new(format!("Invalid unit: '{other}'"))),
        })
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A `(number, unit)` pair as parsed from LaTeX size arguments.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Measurement {
    pub number: f64,
    pub unit: Unit,
}

impl Measurement {
    pub const fn new(number: f64, unit: Unit) -> Self {
        Self { number, unit }
    }
}

/// Round `n` to 4 decimal places and append the literal `"em"`. Mirrors
/// upstream `makeEm` in `units.ts`. The `+n.toFixed(4)` JS idiom strips
/// trailing zeros; Rust's `{:.4}` formatter does not, so we trim manually.
pub fn make_em(n: f64) -> String {
    // `{:.4}` always emits a decimal point, so trimming `0` then `.`
    // collapses e.g. "1.0000" -> "1" and "0.5000" -> "0.5" without a
    // length check.
    let s = format!("{n:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    format!("{trimmed}em")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_absolute_units() {
        assert_eq!(Unit::from_str("pt").unwrap(), Unit::Pt);
        assert_eq!(Unit::from_str("px").unwrap(), Unit::Px);
        assert_eq!(Unit::from_str("sp").unwrap(), Unit::Sp);
    }

    #[test]
    fn parses_relative_units() {
        assert_eq!(Unit::from_str("em").unwrap(), Unit::Em);
        assert_eq!(Unit::from_str("ex").unwrap(), Unit::Ex);
        assert_eq!(Unit::from_str("mu").unwrap(), Unit::Mu);
    }

    #[test]
    fn rejects_unknown_units() {
        let err = Unit::from_str("xx").unwrap_err();
        assert!(err.to_string().contains("Invalid unit: 'xx'"));
    }

    #[test]
    fn is_absolute_classifies_correctly() {
        assert!(Unit::Pt.is_absolute());
        assert!(Unit::Cm.is_absolute());
        assert!(!Unit::Em.is_absolute());
        assert!(!Unit::Mu.is_absolute());
    }

    #[test]
    fn pt_per_unit_matches_upstream_table() {
        assert_eq!(Unit::Pt.pt_per_unit(), Some(1.0));
        assert_eq!(Unit::In.pt_per_unit(), Some(72.27));
        assert_eq!(Unit::Bp.pt_per_unit(), Some(803.0 / 800.0));
        assert_eq!(Unit::Mm.pt_per_unit(), Some(7227.0 / 2540.0));
        assert_eq!(Unit::Em.pt_per_unit(), None);
        assert_eq!(Unit::Mu.pt_per_unit(), None);
    }

    #[test]
    fn as_str_round_trips() {
        for u in [
            Unit::Pt,
            Unit::Mm,
            Unit::Cm,
            Unit::In,
            Unit::Bp,
            Unit::Pc,
            Unit::Dd,
            Unit::Cc,
            Unit::Nd,
            Unit::Nc,
            Unit::Sp,
            Unit::Px,
            Unit::Ex,
            Unit::Em,
            Unit::Mu,
        ] {
            assert_eq!(Unit::from_str(u.as_str()).unwrap(), u);
        }
    }

    #[test]
    fn make_em_strips_trailing_zeros() {
        // Upstream `+n.toFixed(4)` collapses 0.5000 -> 0.5, 1.0000 -> 1.
        assert_eq!(make_em(0.5), "0.5em");
        assert_eq!(make_em(1.0), "1em");
        assert_eq!(make_em(0.0), "0em");
        assert_eq!(make_em(0.0001), "0.0001em");
        assert_eq!(make_em(1.234567), "1.2346em"); // rounds 5th decimal up
    }
}

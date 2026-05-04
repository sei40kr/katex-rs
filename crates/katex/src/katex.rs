//! Top-level public API. Mirrors upstream KaTeX's `katex.ts`
//! `renderToString` / `renderToDomTree` entry points, scoped to the
//! Phase-6 MathML-only milestone.

use crate::build_mathml::build_math_ml;
use crate::options::Options;
use crate::parse_error::ParseError;
use crate::parse_node::ParseNode;
use crate::parser::Parser;
use crate::settings::Settings;

/// Parse a TeX expression to a [`Vec<ParseNode>`]. Mirrors upstream's
/// `katex.__parse` debug entry point.
pub fn parse(tex: &str, settings: &Settings) -> Result<Vec<ParseNode>, ParseError> {
    let mut parser = Parser::new(tex.to_string(), settings);
    parser.parse()
}

/// Render a TeX expression to MathML markup. Mirrors upstream's
/// `katex.renderToString` when invoked with `output: "mathml"`.
pub fn render_to_mathml_string(tex: &str, settings: &Settings) -> Result<String, ParseError> {
    let tree = parse(tex, settings)?;
    let options = Options::root_for(settings);
    let element = build_math_ml(&tree, tex, &options, settings, false);
    Ok(element.to_markup())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_one_two_round_trips() {
        let s = Settings::default();
        let mml = render_to_mathml_string("\\frac{1}{2}", &s).expect("render");
        assert!(
            mml.contains("<mfrac><mn>1</mn><mn>2</mn></mfrac>"),
            "got: {mml}"
        );
        assert!(mml.contains("<annotation"), "missing annotation: {mml}");
    }

    #[test]
    fn parse_returns_a_node() {
        let s = Settings::default();
        let body = parse("x", &s).expect("parse");
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn parse_error_propagates() {
        let s = Settings::default();
        let err = render_to_mathml_string("\\notacommand", &s).unwrap_err();
        assert!(err.raw_message.contains("Undefined control sequence"));
    }
}

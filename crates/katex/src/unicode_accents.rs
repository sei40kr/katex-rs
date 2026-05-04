//! Unicode combining-accent → LaTeX-source mapping. Mirrors upstream
//! `unicodeAccents.js`. `math` is `None` for accents that have no
//! math-mode counterpart (e.g. `\H`, `\c`).

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UnicodeAccent {
    pub text: &'static str,
    pub math: Option<&'static str>,
}

pub static UNICODE_ACCENTS: phf::Map<char, UnicodeAccent> =
    include!(concat!(env!("OUT_DIR"), "/unicode_accents.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acute_accent() {
        let a = UNICODE_ACCENTS.get(&'\u{0301}').unwrap();
        assert_eq!(a.text, "\\'");
        assert_eq!(a.math, Some("\\acute"));
    }

    #[test]
    fn cedilla_has_no_math_form() {
        let a = UNICODE_ACCENTS.get(&'\u{0327}').unwrap();
        assert_eq!(a.text, "\\c");
        assert_eq!(a.math, None);
    }
}

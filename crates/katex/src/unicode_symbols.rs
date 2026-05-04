//! Precomposed-Unicode → LaTeX-source mapping for accented letters.
//!
//! Mirrors the table generated at upstream startup by
//! `unicodeSymbols.js` (combines the supported letters with each
//! `unicodeAccents` entry, keeps every NFC-normalized result that is
//! still a single codepoint). The Parser uses this to rewrite a
//! precomposed character like `é` into its LaTeX equivalent `\\'e`
//! before symbol lookup.

pub static UNICODE_SYMBOLS: phf::Map<char, &'static str> =
    include!(concat!(env!("OUT_DIR"), "/unicode_symbols.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e_acute_decomposes() {
        // U+00E9 (é) -> "e" + combining acute (U+0301)
        assert_eq!(UNICODE_SYMBOLS.get(&'é'), Some(&"e\u{0301}"));
    }

    #[test]
    fn unaccented_letter_misses() {
        assert!(UNICODE_SYMBOLS.get(&'a').is_none());
    }
}

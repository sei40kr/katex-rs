//! Unicode script families supported in `\text{}`.
//!
//! Mirrors upstream KaTeX's `unicodeScripts.ts`.

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Script {
    pub name: &'static str,
    /// Inclusive `(lo, hi)` Unicode-block ranges this script covers.
    pub blocks: &'static [(u32, u32)],
}

pub static SCRIPT_DATA: &[Script] = include!(concat!(env!("OUT_DIR"), "/unicode_scripts.rs"));

/// Mirrors upstream `scriptFromCodepoint`.
pub fn script_from_codepoint(c: char) -> Option<&'static str> {
    let cp = c as u32;
    // Every block in `SCRIPT_DATA` starts at U+0100 or above; bail
    // early on ASCII + Latin-1, which the parser sees on the hot path.
    if cp < 0x0100 {
        return None;
    }
    for s in SCRIPT_DATA {
        for (lo, hi) in s.blocks {
            if cp >= *lo && cp <= *hi {
                return Some(s.name);
            }
        }
    }
    None
}

/// Mirrors upstream `supportedCodepoint`.
pub fn supported_codepoint(c: char) -> bool {
    script_from_codepoint(c).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyrillic_a_is_cyrillic() {
        assert_eq!(script_from_codepoint('\u{0410}'), Some("cyrillic"));
    }

    #[test]
    fn latin_extended_is_latin() {
        assert_eq!(script_from_codepoint('\u{0100}'), Some("latin"));
    }

    #[test]
    fn ascii_is_unsupported() {
        assert!(!supported_codepoint('A'));
    }

    #[test]
    fn far_out_codepoint_is_unsupported() {
        assert!(!supported_codepoint('\u{1F600}'));
    }
}

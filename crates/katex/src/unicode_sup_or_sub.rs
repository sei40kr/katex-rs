//! Unicode (sub|super)script characters and the regex that detects a
//! leading subscript codepoint. Mirrors upstream `unicodeSupOrSub.ts`.

include!(concat!(env!("OUT_DIR"), "/unicode_sup_or_sub.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscript_two_maps_to_two() {
        assert_eq!(U_SUBS_AND_SUPS.get(&'\u{2082}'), Some(&"2")); // ₂
    }

    #[test]
    fn superscript_two_maps_to_two() {
        assert_eq!(U_SUBS_AND_SUPS.get(&'\u{00b2}'), Some(&"2")); // ²
    }

    #[test]
    fn pattern_starts_anchored() {
        // Upstream's `unicodeSubRegEx = /^[...]/`.
        assert!(UNICODE_SUB_REGEX.starts_with('^'));
    }
}

//! Built-in string-replacement macros.
//!
//! Mirrors upstream `macros.ts` for the `defineMacro("\\name",
//! "<body>")` calls — i.e. macros whose expansion is a fixed source
//! string. Function-form macros (`defineMacro("\\foo", ctx => ...)`)
//! stay in code and will be wired into [`MacroExpander`]'s built-ins
//! map as the parser-side phases land.
//!
//! [`MacroExpander`]: crate::macro_expander::MacroExpander

pub static MACROS: phf::Map<&'static str, &'static str> =
    include!(concat!(env!("OUT_DIR"), "/macros.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgroup_expands_to_open_brace() {
        assert_eq!(MACROS.get("\\bgroup"), Some(&"{"));
        assert_eq!(MACROS.get("\\egroup"), Some(&"}"));
    }

    #[test]
    fn aa_expands_to_r_a() {
        assert_eq!(MACROS.get("\\aa"), Some(&"\\r a"));
    }

    #[test]
    fn unknown_macro_misses() {
        assert!(MACROS.get("\\definitelyNotAMacro").is_none());
    }
}

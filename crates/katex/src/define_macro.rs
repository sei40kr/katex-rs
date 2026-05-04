//! Macro-spec shape used to register entries in the global macro
//! dispatch table. Mirrors upstream KaTeX's `defineMacro.ts`.
//!
//! The macro **runtime** (the `Namespace<MacroDefinition>` and the
//! expansion engine) already exists in [`crate::macro_expander`]. This
//! module is purely the *registration* shape: a name → definition pair
//! that Phase 2's codegen output and Phase 5's per-feature macro lists
//! emit. The same "explicit slice → Lazy HashMap" pattern as
//! [`crate::define_function`] applies here.

use crate::macro_expander::MacroDefinition;

/// One entry in the built-in macro dispatch table.
///
/// Mirrors upstream's `defineMacro(name, body)` call shape — a single
/// name plus its definition. The single-name form (rather than the
/// function/environment "names: &[&str]" form) is intentional and
/// matches upstream: macro names are typically aliased via separate
/// `\let` definitions rather than shared at registration.
#[derive(Clone)]
pub struct MacroSpec {
    pub name: &'static str,
    pub definition: MacroDefinition,
}

impl MacroSpec {
    pub const fn new(name: &'static str, definition: MacroDefinition) -> Self {
        Self { name, definition }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smol_str::SmolStr;

    #[test]
    fn macro_spec_holds_source_definition() {
        let spec = MacroSpec::new("\\RR", MacroDefinition::Source(SmolStr::new("\\mathbb{R}")));
        match spec.definition {
            MacroDefinition::Source(s) => assert_eq!(s, "\\mathbb{R}"),
            _ => panic!("expected source"),
        }
    }
}

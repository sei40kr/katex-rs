//! Function dispatch registry. Mirrors upstream KaTeX's `functions.ts`,
//! which re-exports the global `_functions` dictionary populated by the
//! per-handler files in `functions/*.ts`.
//!
//! # Why explicit slices instead of `defineFunction(...)` side effects
//!
//! Upstream calls `defineFunction(...)` at module-load time from each of
//! ~25 handler files; the dictionary's contents depend on the import
//! graph being walked in a particular order. We reject this for three
//! reasons:
//!
//! 1. **Registration-order bugs are silent.** A late import wins; an
//!    accidental missing import drops a function. There is no compile
//!    failure for either.
//! 2. **`wasm32-unknown-unknown`** has no portable analogue of
//!    `inventory`/`linkme` (the linker-section trick that approximates
//!    auto-registration on native targets), and we have an explicit
//!    no-`wasm-bindgen`-in-core rule anyway.
//! 3. **The data is small.** ~150 function specs is fine to walk once
//!    at startup; a `LazyLock<HashMap<...>>` does that lazily.
//!
//! Phase 5 fills [`FUNCTION_SLICES`] with one inner slice per upstream
//! `functions/*.ts` file. Phase 3 leaves it empty — the registry shape
//! works, the dispatch table is populated when handlers exist.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::define_function::FunctionSpec;

/// Built-in function specs, partitioned by upstream source file.
/// Each inner slice is owned by one Phase 5 module (e.g.
/// `functions/accent.rs`, `functions/sqrt.rs`); its slice is `pub`
/// from that module and added here.
///
/// Empty for Phase 3.
const FUNCTION_SLICES: &[&[FunctionSpec]] = &[];

/// Name → spec lookup table built by walking [`FUNCTION_SLICES`] at
/// first use. Mirrors upstream's `_functions` dict.
pub static FUNCTIONS: LazyLock<FunctionRegistry> =
    LazyLock::new(|| FunctionRegistry::from_slices(FUNCTION_SLICES));

/// Read-only function registry. Built once from a `&[&[FunctionSpec]]`
/// slice; afterwards lookup is `O(1)` against an interned name string.
pub struct FunctionRegistry {
    map: HashMap<&'static str, &'static FunctionSpec>,
}

impl FunctionRegistry {
    /// Build a registry from an explicit slice-of-slices. Each inner
    /// slice is treated as one logical "module" of functions; specs
    /// across slices share the same name space.
    ///
    /// **Panics** if two specs declare the same name. Upstream's
    /// last-write-wins semantics let one handler silently overwrite
    /// another, which has caused real bugs in the JS codebase; we
    /// surface the conflict instead.
    pub fn from_slices(slices: &'static [&'static [FunctionSpec]]) -> Self {
        let mut map: HashMap<&'static str, &'static FunctionSpec> = HashMap::new();
        for slice in slices {
            for spec in *slice {
                for name in spec.names {
                    if map.insert(*name, spec).is_some() {
                        panic!("duplicate function registration for `{name}`");
                    }
                }
            }
        }
        Self { map }
    }

    pub fn get(&self, name: &str) -> Option<&'static FunctionSpec> {
        self.map.get(name).copied()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.map.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_error::ParseError;
    use crate::parse_node::{NodeType, ParseNode};
    use crate::types::Mode;

    fn dummy_handler(
        _ctx: crate::define_function::FunctionContext<'_>,
        _args: &[ParseNode],
        _opt_args: &[Option<ParseNode>],
    ) -> Result<ParseNode, ParseError> {
        Ok(ParseNode::Internal {
            mode: Mode::Math,
            loc: None,
        })
    }

    const FOO_SPEC: FunctionSpec = FunctionSpec {
        node_type: NodeType::Internal,
        names: &["\\foo", "\\foobar"],
        num_args: 0,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: false,
        allowed_in_text: false,
        allowed_in_math: true,
        infix: false,
        primitive: false,
        handler: Some(dummy_handler),
        mathml_builder: None,
        html_builder: None,
    };

    const BAR_SPEC: FunctionSpec = FunctionSpec {
        node_type: NodeType::Internal,
        names: &["\\bar"],
        num_args: 1,
        num_optional_args: 0,
        arg_types: &[],
        allowed_in_argument: true,
        allowed_in_text: true,
        allowed_in_math: true,
        infix: false,
        primitive: true,
        handler: Some(dummy_handler),
        mathml_builder: None,
        html_builder: None,
    };

    static MODULE_A: &[FunctionSpec] = &[FOO_SPEC];
    static MODULE_B: &[FunctionSpec] = &[BAR_SPEC];
    static SLICES: &[&[FunctionSpec]] = &[MODULE_A, MODULE_B];

    #[test]
    fn round_trip_insert_and_lookup() {
        let reg = FunctionRegistry::from_slices(SLICES);
        // All three names from the two specs registered.
        assert_eq!(reg.len(), 3);
        assert!(reg.contains("\\foo"));
        assert!(reg.contains("\\foobar"));
        assert!(reg.contains("\\bar"));
        assert!(!reg.contains("\\baz"));

        // Aliased names point at the same underlying spec.
        let foo = reg.get("\\foo").unwrap();
        let foobar = reg.get("\\foobar").unwrap();
        assert!(std::ptr::eq(foo, foobar));
        assert_eq!(foo.node_type, NodeType::Internal);
        assert_eq!(foo.num_args, 0);

        // Different spec, different fields.
        let bar = reg.get("\\bar").unwrap();
        assert_eq!(bar.num_args, 1);
        assert!(bar.primitive);
        assert!(bar.allowed_in_text);
    }

    #[test]
    fn default_registry_initialises_from_function_slices() {
        // The crate-wide `FUNCTIONS` registry is built from
        // `FUNCTION_SLICES` on first access. Today `FUNCTION_SLICES` is
        // empty, so the registry contains no entries; this test mostly
        // exercises that the LazyLock initialiser doesn't panic.
        assert!(FUNCTIONS.is_empty());
        assert!(FUNCTIONS.get("\\frac").is_none());
    }

    static DUP_A: &[FunctionSpec] = &[FOO_SPEC];
    static DUP_B: &[FunctionSpec] = &[FOO_SPEC];
    static DUP_SLICES: &[&[FunctionSpec]] = &[DUP_A, DUP_B];

    #[test]
    #[should_panic(expected = "duplicate function registration")]
    fn duplicate_registration_panics() {
        let _ = FunctionRegistry::from_slices(DUP_SLICES);
    }
}

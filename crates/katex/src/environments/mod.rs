//! Environment dispatch registry. Mirrors upstream KaTeX's
//! `environments.ts`, which re-exports the `_environments` dict
//! populated by the per-environment files in `environments/*.ts`.
//!
//! Like [`crate::functions`], registration is explicit slices instead
//! of upstream's load-order-dependent side effects. See the deviation
//! note in [`crate::functions`] for the full rationale.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::define_environment::EnvSpec;

pub mod array;
pub mod cd;

/// Built-in environment specs, partitioned by upstream source file.
const ENVIRONMENT_SLICES: &[&[EnvSpec]] = &[array::SPECS];

/// Name → spec lookup table, built lazily from [`ENVIRONMENT_SLICES`]
/// on first access. Mirrors upstream's `_environments` dict.
pub static ENVIRONMENTS: LazyLock<EnvironmentRegistry> =
    LazyLock::new(|| EnvironmentRegistry::from_slices(ENVIRONMENT_SLICES));

/// Read-only environment registry. Built once from a `&[&[EnvSpec]]`
/// slice; afterwards lookup is `O(1)` against an interned name string.
pub struct EnvironmentRegistry {
    map: HashMap<&'static str, &'static EnvSpec>,
}

impl EnvironmentRegistry {
    pub fn from_slices(slices: &'static [&'static [EnvSpec]]) -> Self {
        let mut map: HashMap<&'static str, &'static EnvSpec> = HashMap::new();
        for slice in slices {
            for spec in *slice {
                for name in spec.names {
                    if map.insert(*name, spec).is_some() {
                        panic!("duplicate environment registration for `{name}`");
                    }
                }
            }
        }
        Self { map }
    }

    pub fn get(&self, name: &str) -> Option<&'static EnvSpec> {
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

    #[test]
    fn registry_contains_core_environments() {
        assert!(!ENVIRONMENTS.is_empty());
        for name in [
            "array",
            "matrix",
            "pmatrix",
            "bmatrix",
            "Bmatrix",
            "vmatrix",
            "Vmatrix",
            "smallmatrix",
            "subarray",
            "cases",
            "dcases",
            "rcases",
            "drcases",
            "align",
            "align*",
            "aligned",
            "split",
            "gathered",
            "gather",
            "gather*",
            "alignat",
            "alignat*",
            "alignedat",
            "equation",
            "equation*",
            "CD",
        ] {
            assert!(ENVIRONMENTS.contains(name), "missing environment `{name}`");
        }
    }
}

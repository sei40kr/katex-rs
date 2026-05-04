//! Scope-stack mapping used by the macro expander (and, eventually, by
//! anything else that needs TeX-style group-local definitions).
//!
//! Mirrors upstream KaTeX's `Namespace.ts`. `get`/local `set` are O(1);
//! global `set` walks every active group frame and is therefore O(depth),
//! exactly as upstream. The undo stack stores one `HashMap` per group:
//! each entry is the value that should be restored when the group ends, or
//! `None` if the entry was absent before the group began (so endGroup
//! deletes it).
//!
//! Deviations from upstream:
//! - JavaScript's `null` and `undefined` collapse to a single `Option<V>`
//!   in Rust; both upstream sentinels mean "delete" and we model that
//!   directly.
//! - Builtins are owned by the namespace (a `HashMap<String, V>`). Phase 2
//!   will swap this for a `&'static phf::Map` once codegen lands; the
//!   public API does not change.
//! - The undo-frame entry type `Option<V>` (Some = restore that value,
//!   None = delete) requires `V: Clone` so global `set` can stash the
//!   incoming value into the innermost frame as upstream does. For the
//!   target value type (`MacroDefinition`) this is cheap.

use std::collections::HashMap;

use crate::parse_error::ParseError;

/// `set` semantics summary:
/// - `local set`: capture the pre-group value into the top frame as undo
///   (only on the *first* set within this group), then mutate `current`.
/// - `global set`: drop any pending undo for `name` from every frame so
///   the change survives all `endGroup` calls, then on the innermost
///   frame only, install the *new* value as the undo entry — so if a
///   later `local set` re-overrides it within the same group, ending the
///   group still leaves the global value in place.
/// - A `None` value means delete (matches upstream `null`/`undefined`).
pub struct Namespace<V> {
    current: HashMap<String, V>,
    builtins: HashMap<String, V>,
    /// One frame per active group; each entry is the undo action to take
    /// when the group ends.
    undef_stack: Vec<HashMap<String, Option<V>>>,
}

impl<V: Clone> Namespace<V> {
    pub fn new(builtins: HashMap<String, V>, globals: HashMap<String, V>) -> Self {
        Self {
            current: globals,
            builtins,
            undef_stack: Vec::new(),
        }
    }

    pub fn begin_group(&mut self) {
        self.undef_stack.push(HashMap::new());
    }

    pub fn end_group(&mut self) -> Result<(), ParseError> {
        let frame = self.undef_stack.pop().ok_or_else(|| {
            ParseError::new(
                "Unbalanced namespace destruction: attempt to pop global namespace; \
                 please report this as a bug",
            )
        })?;
        for (name, prev) in frame {
            match prev {
                Some(v) => {
                    self.current.insert(name, v);
                }
                None => {
                    self.current.remove(&name);
                }
            }
        }
        Ok(())
    }

    pub fn end_groups(&mut self) -> Result<(), ParseError> {
        while !self.undef_stack.is_empty() {
            self.end_group()?;
        }
        Ok(())
    }

    pub fn has(&self, name: &str) -> bool {
        self.current.contains_key(name) || self.builtins.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&V> {
        self.current.get(name).or_else(|| self.builtins.get(name))
    }

    /// Set or delete (`value = None`) `name`, optionally globally.
    pub fn set(&mut self, name: impl Into<String>, value: Option<V>, global: bool) {
        let name = name.into();
        if global {
            // Erase pending undos for this name in every frame so the
            // change can't be locally rolled back.
            for frame in &mut self.undef_stack {
                frame.remove(&name);
            }
            // On the innermost frame, install the new value as undo too —
            // that way a later local set within this group can still be
            // rolled back without losing the global value.
            if let Some(top) = self.undef_stack.last_mut() {
                top.insert(name.clone(), value.clone());
            }
        } else if let Some(top) = self.undef_stack.last_mut() {
            // First local set in this group records the pre-group state;
            // subsequent sets must not overwrite that record.
            if !top.contains_key(&name) {
                let prev = self.current.get(&name).cloned();
                top.insert(name.clone(), prev);
            }
        }
        match value {
            Some(v) => {
                self.current.insert(name, v);
            }
            None => {
                self.current.remove(&name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns() -> Namespace<i32> {
        let mut builtins = HashMap::new();
        builtins.insert("e".to_string(), 100);
        Namespace::new(builtins, HashMap::new())
    }

    #[test]
    fn get_falls_back_to_builtins() {
        let n = ns();
        assert_eq!(n.get("e"), Some(&100));
        assert_eq!(n.get("x"), None);
        assert!(n.has("e"));
        assert!(!n.has("x"));
    }

    #[test]
    fn local_set_rolls_back_at_end_group() {
        let mut n = ns();
        n.set("x", Some(1), false);
        n.begin_group();
        n.set("x", Some(2), false);
        assert_eq!(n.get("x"), Some(&2));
        n.end_group().unwrap();
        assert_eq!(n.get("x"), Some(&1));
    }

    #[test]
    fn local_set_can_delete_within_group() {
        let mut n = ns();
        n.set("x", Some(1), false);
        n.begin_group();
        n.set("x", None, false);
        assert_eq!(n.get("x"), None);
        n.end_group().unwrap();
        assert_eq!(n.get("x"), Some(&1));
    }

    #[test]
    fn first_local_set_in_group_captures_undo_only_once() {
        let mut n = ns();
        n.set("x", Some(1), false);
        n.begin_group();
        n.set("x", Some(2), false);
        n.set("x", Some(3), false); // must not overwrite the captured undo
        n.end_group().unwrap();
        assert_eq!(n.get("x"), Some(&1));
    }

    #[test]
    fn global_set_survives_end_group() {
        let mut n = ns();
        n.begin_group();
        n.set("x", Some(7), true);
        n.end_group().unwrap();
        assert_eq!(n.get("x"), Some(&7));
    }

    #[test]
    fn global_set_survives_local_override_then_end_group() {
        // A subsequent local set within the same group is rolled back to
        // the global value, not to the pre-group state.
        let mut n = ns();
        n.begin_group();
        n.set("x", Some(7), true);
        n.set("x", Some(8), false);
        assert_eq!(n.get("x"), Some(&8));
        n.end_group().unwrap();
        assert_eq!(n.get("x"), Some(&7));
    }

    #[test]
    fn global_set_with_nested_groups() {
        let mut n = ns();
        n.set("x", Some(1), false);
        n.begin_group();
        n.begin_group();
        n.set("x", Some(9), true);
        n.end_group().unwrap();
        n.end_group().unwrap();
        assert_eq!(n.get("x"), Some(&9));
    }

    #[test]
    fn end_group_on_empty_stack_errors() {
        let mut n = ns();
        let err = n.end_group().unwrap_err();
        assert!(err.raw_message.contains("Unbalanced namespace destruction"));
    }

    #[test]
    fn end_groups_pops_all_remaining_groups() {
        let mut n = ns();
        n.set("x", Some(1), false);
        n.begin_group();
        n.set("x", Some(2), false);
        n.begin_group();
        n.set("x", Some(3), false);
        n.end_groups().unwrap();
        assert_eq!(n.get("x"), Some(&1));
    }
}

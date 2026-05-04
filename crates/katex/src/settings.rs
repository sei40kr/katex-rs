//! User-facing render options.
//!
//! Mirrors upstream KaTeX's `Settings.ts`. Field names are snake-cased and
//! types are made concrete, but every option from upstream's
//! `SETTINGS_SCHEMA` has a counterpart here with the same default value.
//!
//! Deviations from upstream, recorded per the project's "deviate
//! deliberately" rule:
//! - `Settings` is `#[non_exhaustive]` so future fields can be added without
//!   a major version bump.
//! - `max_size` and `max_expand` use `Option` (`None` == unlimited) rather
//!   than upstream's sentinel `Infinity`. This is more idiomatic in Rust and
//!   keeps `max_expand` integral.
//! - `strict` and `trust` upstream accept callbacks
//!   (`StrictFunction`/`TrustFunction`); for Phase 0 we model only the data
//!   variants. Function callbacks will return when the parser/builder lands
//!   and a stable token/parse-node API is available to pass to them.
//! - `macros` upstream is a `MacroMap` of `string | MacroExpansion`; Phase 0
//!   exposes only string-substitution macros. The full type lands when
//!   `defineMacro` is ported.
//!
//! `reportNonstrict`, `useStrictBehavior`, and `isTrusted` from upstream are
//! Parser/Builder concerns and are deferred to later phases.

use std::collections::HashMap;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum OutputFormat {
    /// Default. Matches upstream `"htmlAndMathml"`.
    #[default]
    HtmlAndMathml,
    Html,
    Mathml,
}

/// Strict-mode behavior. Upstream encodes this as `boolean | "ignore" |
/// "warn" | "error" | function`. We model the data variants only; `true` and
/// `"error"` collapse to a single `Error` variant per upstream's
/// `reportNonstrict` (which treats them identically).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum StrictMode {
    /// `false` upstream — silently allow non-LaTeX-compatible input.
    Disabled,
    /// `"ignore"` — same as `Disabled` but explicit. Kept for round-trip
    /// fidelity with upstream config files.
    Ignore,
    /// `"warn"` — log a warning. Upstream default.
    #[default]
    Warn,
    /// `true` / `"error"` — raise a `ParseError`.
    Error,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", default))]
#[non_exhaustive]
pub struct Settings {
    pub display_mode: bool,
    pub output: OutputFormat,
    pub leqno: bool,
    pub fleqn: bool,
    pub throw_on_error: bool,
    pub error_color: String,
    pub macros: HashMap<String, String>,
    pub min_rule_thickness: f64,
    pub color_is_text_color: bool,
    pub strict: StrictMode,
    pub trust: bool,
    /// Cap on user-specified sizes, in ems. `None` is upstream `Infinity`.
    pub max_size: Option<f64>,
    /// Cap on macro expansions. `None` is upstream `Infinity`. Default 1000.
    pub max_expand: Option<u32>,
    pub global_group: bool,
}

impl Default for Settings {
    fn default() -> Self {
        // Defaults mirror upstream `SETTINGS_SCHEMA` + `getDefaultValue`.
        Self {
            display_mode: false,
            output: OutputFormat::default(),
            leqno: false,
            fleqn: false,
            throw_on_error: true,
            error_color: "#cc0000".to_string(),
            macros: HashMap::new(),
            min_rule_thickness: 0.0,
            color_is_text_color: false,
            strict: StrictMode::default(),
            trust: false,
            max_size: None,
            max_expand: Some(1000),
            global_group: false,
        }
    }
}

impl Settings {
    pub fn builder() -> SettingsBuilder {
        SettingsBuilder::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct SettingsBuilder {
    settings: Settings,
}

impl SettingsBuilder {
    pub fn display_mode(mut self, v: bool) -> Self {
        self.settings.display_mode = v;
        self
    }
    pub fn output(mut self, v: OutputFormat) -> Self {
        self.settings.output = v;
        self
    }
    pub fn leqno(mut self, v: bool) -> Self {
        self.settings.leqno = v;
        self
    }
    pub fn fleqn(mut self, v: bool) -> Self {
        self.settings.fleqn = v;
        self
    }
    pub fn throw_on_error(mut self, v: bool) -> Self {
        self.settings.throw_on_error = v;
        self
    }
    pub fn error_color(mut self, v: impl Into<String>) -> Self {
        self.settings.error_color = v.into();
        self
    }
    pub fn macros(mut self, v: HashMap<String, String>) -> Self {
        self.settings.macros = v;
        self
    }
    pub fn add_macro(mut self, name: impl Into<String>, expansion: impl Into<String>) -> Self {
        self.settings.macros.insert(name.into(), expansion.into());
        self
    }
    /// Clamps to `>= 0` to match upstream's `Math.max(0, t)` processor.
    pub fn min_rule_thickness(mut self, v: f64) -> Self {
        self.settings.min_rule_thickness = v.max(0.0);
        self
    }
    pub fn color_is_text_color(mut self, v: bool) -> Self {
        self.settings.color_is_text_color = v;
        self
    }
    pub fn strict(mut self, v: StrictMode) -> Self {
        self.settings.strict = v;
        self
    }
    pub fn trust(mut self, v: bool) -> Self {
        self.settings.trust = v;
        self
    }
    /// Clamps to `>= 0` to match upstream's `Math.max(0, s)` processor.
    /// Pass `None` for unlimited (upstream `Infinity`).
    pub fn max_size(mut self, v: Option<f64>) -> Self {
        self.settings.max_size = v.map(|s| s.max(0.0));
        self
    }
    /// Pass `None` for unlimited (upstream `Infinity`).
    pub fn max_expand(mut self, v: Option<u32>) -> Self {
        self.settings.max_expand = v;
        self
    }
    pub fn global_group(mut self, v: bool) -> Self {
        self.settings.global_group = v;
        self
    }

    pub fn build(self) -> Settings {
        self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_upstream_schema() {
        let s = Settings::default();
        assert!(!s.display_mode);
        assert_eq!(s.output, OutputFormat::HtmlAndMathml);
        assert!(!s.leqno);
        assert!(!s.fleqn);
        assert!(s.throw_on_error);
        assert_eq!(s.error_color, "#cc0000");
        assert!(s.macros.is_empty());
        assert_eq!(s.min_rule_thickness, 0.0);
        assert!(!s.color_is_text_color);
        assert_eq!(s.strict, StrictMode::Warn);
        assert!(!s.trust);
        assert_eq!(s.max_size, None);
        assert_eq!(s.max_expand, Some(1000));
        assert!(!s.global_group);
    }

    #[test]
    fn builder_overrides_defaults() {
        let s = Settings::builder()
            .display_mode(true)
            .output(OutputFormat::Mathml)
            .throw_on_error(false)
            .error_color("#ff0000")
            .strict(StrictMode::Error)
            .trust(true)
            .max_size(Some(500.0))
            .max_expand(None)
            .add_macro("\\RR", "\\mathbb{R}")
            .build();
        assert!(s.display_mode);
        assert_eq!(s.output, OutputFormat::Mathml);
        assert!(!s.throw_on_error);
        assert_eq!(s.error_color, "#ff0000");
        assert_eq!(s.strict, StrictMode::Error);
        assert!(s.trust);
        assert_eq!(s.max_size, Some(500.0));
        assert_eq!(s.max_expand, None);
        assert_eq!(
            s.macros.get("\\RR").map(String::as_str),
            Some("\\mathbb{R}")
        );
    }

    #[test]
    fn builder_clamps_negative_values_to_zero() {
        let s = Settings::builder()
            .max_size(Some(-1.0))
            .min_rule_thickness(-0.5)
            .build();
        assert_eq!(s.max_size, Some(0.0));
        assert_eq!(s.min_rule_thickness, 0.0);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trips() {
        let s = Settings::builder()
            .display_mode(true)
            .output(OutputFormat::Html)
            .strict(StrictMode::Error)
            .build();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_uses_camel_case_keys() {
        let s = Settings::builder().display_mode(true).leqno(true).build();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"displayMode\":true"), "got: {json}");
        assert!(!json.contains("display_mode"), "got: {json}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_partial_input_fills_in_defaults() {
        let s: Settings = serde_json::from_str(r#"{"displayMode": true}"#).unwrap();
        let expected = Settings::builder().display_mode(true).build();
        assert_eq!(s, expected);
    }
}

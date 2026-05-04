//! Rendering options threaded through the MathML (and, in Phase 10,
//! HTML+CSS) builders.
//!
//! Mirrors upstream KaTeX's `Options.ts`. Phase 6 populates only the
//! fields the MathML output actually reads — style, color, size,
//! font/family/weight/shape, and the two settings-level numeric caps.
//! Phase 10 will extend this with font-size table lookups, glue-spacing
//! state, and other HTML-only fields.
//!
//! # Deviations from upstream
//!
//! - Upstream represents a single instance with a JS class and uses
//!   `extend({...})` to copy with overrides. We use plain
//!   value-style `Clone` plus typed builder helpers
//!   ([`Options::having_style`] etc.) which return `Self`.
//! - Upstream's `font: ""` sentinel for "no override" becomes
//!   [`Option::None`] here; the same goes for the per-style overrides.
//! - The constructor is `pub(crate)`. Outside callers obtain an
//!   `Options` indirectly through [`Options::root_for`] /
//!   [`crate::render_to_mathml_string`], matching upstream's
//!   "no public Options constructor" stance.

use smol_str::SmolStr;

use crate::settings::Settings;
use crate::style::Style;

/// Render-time options threaded into builders. Cheap to clone — the
/// `SmolStr` fields are inline up to 23 bytes and copy in O(1).
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Current math style (display/text/script/scriptscript, with the
    /// cramped variants).
    pub style: Style,
    /// Inherited foreground color (hex or named). `None` = no override.
    pub color: Option<SmolStr>,
    /// Sizing index (1..=11), matching upstream's `\Huge` / `\large` table.
    pub size: u8,
    /// Specific math font (`"mathbb"`, `"mathfrak"`, …). `None` = none.
    pub font: Option<SmolStr>,
    /// CSS-style font family override (`"KaTeX_Main"`, …).
    pub font_family: Option<SmolStr>,
    /// `"bold"`, `"normal"`, …
    pub font_weight: Option<SmolStr>,
    /// `"italic"`, `"normal"`, …
    pub font_shape: Option<SmolStr>,
    /// Cap on user-specified sizes, in ems. `None` = unlimited
    /// (upstream `Infinity`).
    pub max_size: Option<f64>,
    /// Minimum rule thickness in ems.
    pub min_rule_thickness: f64,
}

impl Options {
    /// Construct with the explicit fields; `pub(crate)` so external
    /// callers funnel through [`Options::root_for`] / the public render
    /// entry points.
    pub(crate) const fn new(style: Style, max_size: Option<f64>, min_rule_thickness: f64) -> Self {
        Self {
            style,
            color: None,
            size: 5,
            font: None,
            font_family: None,
            font_weight: None,
            font_shape: None,
            max_size,
            min_rule_thickness,
        }
    }

    /// Default options for a top-level render. `display_mode` chooses
    /// the initial style (Display vs. Text); the numeric caps come from
    /// [`Settings`].
    pub fn root_for(settings: &Settings) -> Self {
        let style = if settings.display_mode {
            Style::Display
        } else {
            Style::Text
        };
        Self::new(style, settings.max_size, settings.min_rule_thickness)
    }

    /// Clone with `style` overridden. Mirrors upstream `havingStyle`.
    pub fn having_style(&self, style: Style) -> Self {
        let mut out = self.clone();
        out.style = style;
        out
    }

    /// Clone with `color` overridden. `None` clears any inherited color.
    pub fn with_color(&self, color: Option<SmolStr>) -> Self {
        let mut out = self.clone();
        out.color = color;
        out
    }

    /// Clone with `font` overridden.
    pub fn with_font(&self, font: Option<SmolStr>) -> Self {
        let mut out = self.clone();
        out.font = font;
        out
    }

    /// Clone with the four font fields cleared. Mirrors upstream's
    /// `withTextFontFamily`-style resets used when entering a `\text{...}`
    /// block in math mode.
    pub fn reset_fonts(&self) -> Self {
        let mut out = self.clone();
        out.font = None;
        out.font_family = None;
        out.font_weight = None;
        out.font_shape = None;
        out
    }
}

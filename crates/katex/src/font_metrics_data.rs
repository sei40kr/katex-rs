//! Per-glyph font metrics, keyed by `(font_name, codepoint)`.
//!
//! Mirrors upstream KaTeX's `fontMetricsData.js`. Behind `feature =
//! "html"` the full upstream table is emitted; the default MathML
//! build emits an empty map (Phase 6 may identify a small MathML
//! subset to surface here).

/// Five-tuple of em-units: `[depth, height, italic, skew, width]`.
/// Order matches upstream `fontMetrics.js`.
pub type CharacterMetrics = [f64; 5];

include!(concat!(env!("OUT_DIR"), "/font_metrics_data.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "html")]
    #[test]
    fn ams_regular_a_metrics_match_upstream() {
        // From fontMetricsData.js: AMS-Regular['65'] = [0, 0.68889, 0, 0, 0.72222]
        let m = FONT_METRICS_DATA
            .get("AMS-Regular")
            .unwrap()
            .get(&65)
            .unwrap();
        assert_eq!(m[0], 0.0);
        assert_eq!(m[1], 0.68889);
        assert_eq!(m[4], 0.72222);
    }

    #[cfg(not(feature = "html"))]
    #[test]
    fn empty_without_html_feature() {
        assert!(FONT_METRICS_DATA.get("AMS-Regular").is_none());
    }
}

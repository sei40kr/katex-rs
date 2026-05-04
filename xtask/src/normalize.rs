//! Canonicalize a MathML string so byte equality is meaningful when
//! comparing the Rust renderer's output against upstream KaTeX's
//! `renderToString({ output: "mathml" })`.
//!
//! Lives in `xtask/` (rather than `crates/katex/src/`) because it is
//! purely test-tooling — `crates/katex` must stay
//! environment-independent. The Rust integration test pulls this file
//! in via `#[path]` so there is exactly one normalizer.

use std::collections::BTreeMap;

/// Normalize a MathML string for snapshot comparison.
///
/// - Strips an enclosing `<span class="katex">…</span>` if present
///   (upstream wraps; our renderer does not).
/// - Sorts attributes alphabetically inside every tag.
/// - Collapses runs of ASCII whitespace inside tags into single spaces,
///   and trims whitespace adjacent to `<` / `>`.
/// - Trims one trailing newline.
pub fn normalize_mathml(input: &str) -> String {
    let stripped = strip_outer_span(input.trim());
    let mut out = String::with_capacity(stripped.len());
    let bytes = stripped.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'<' {
            let end = match find_tag_end(bytes, i) {
                Some(j) => j,
                None => {
                    out.push_str(&stripped[i..]);
                    break;
                }
            };
            let tag = &stripped[i..=end];
            out.push_str(&canonicalize_tag(tag));
            i = end + 1;
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

fn strip_outer_span(s: &str) -> &str {
    const OPEN: &str = "<span class=\"katex\">";
    const CLOSE: &str = "</span>";
    if let Some(rest) = s.strip_prefix(OPEN)
        && let Some(inner) = rest.strip_suffix(CLOSE)
    {
        return inner;
    }
    s
}

fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let mut in_dq = false;
    let mut in_sq = false;
    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_sq => in_dq = !in_dq,
            b'\'' if !in_dq => in_sq = !in_sq,
            b'>' if !in_dq && !in_sq => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn canonicalize_tag(tag: &str) -> String {
    debug_assert!(tag.starts_with('<') && tag.ends_with('>'));
    let inner = &tag[1..tag.len() - 1];

    if inner.starts_with('!') || inner.starts_with('?') {
        return tag.to_string();
    }

    let (inner_body, self_closing) = match inner.strip_suffix('/') {
        Some(rest) => (rest.trim_end(), true),
        None => (inner, false),
    };
    let is_close = inner_body.starts_with('/');
    let body = if is_close {
        &inner_body[1..]
    } else {
        inner_body
    };

    let (name, attrs_part) = split_name(body);
    if attrs_part.trim().is_empty() {
        let mut s = String::with_capacity(tag.len());
        s.push('<');
        if is_close {
            s.push('/');
        }
        s.push_str(name);
        if self_closing {
            s.push_str(" /");
        }
        s.push('>');
        return s;
    }

    let mut attrs = parse_attrs(attrs_part);
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    let dedup: BTreeMap<String, String> = attrs.into_iter().collect();

    let mut s = String::with_capacity(tag.len());
    s.push('<');
    if is_close {
        s.push('/');
    }
    s.push_str(name);
    for (k, v) in &dedup {
        s.push(' ');
        s.push_str(k);
        s.push_str("=\"");
        s.push_str(v);
        s.push('"');
    }
    if self_closing {
        s.push_str(" /");
    }
    s.push('>');
    s
}

fn split_name(body: &str) -> (&str, &str) {
    for (i, c) in body.char_indices() {
        if c.is_ascii_whitespace() {
            return (&body[..i], &body[i..]);
        }
    }
    (body, "")
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = &s[name_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            // Boolean attribute.
            out.push((name.to_string(), String::new()));
            continue;
        }
        i += 1; // '='
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            out.push((name.to_string(), String::new()));
            break;
        }
        let quote = bytes[i];
        let value = if quote == b'"' || quote == b'\'' {
            i += 1;
            let v_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            let v = &s[v_start..i];
            if i < bytes.len() {
                i += 1;
            }
            v.to_string()
        } else {
            let v_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            s[v_start..i].to_string()
        };
        out.push((name.to_string(), value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_span_wrapper() {
        let upstream = r#"<span class="katex"><math xmlns="http://www.w3.org/1998/Math/MathML"><mn>1</mn></math></span>"#;
        let ours = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mn>1</mn></math>"#;
        assert_eq!(normalize_mathml(upstream), normalize_mathml(ours));
    }

    #[test]
    fn sorts_attributes() {
        let a = r#"<mo b="2" a="1">+</mo>"#;
        let b = r#"<mo a="1" b="2">+</mo>"#;
        assert_eq!(normalize_mathml(a), normalize_mathml(b));
    }

    #[test]
    fn collapses_attribute_spacing() {
        let a = "<mo  a=\"1\"   b=\"2\">+</mo>";
        let b = "<mo a=\"1\" b=\"2\">+</mo>";
        assert_eq!(normalize_mathml(a), normalize_mathml(b));
    }

    #[test]
    fn preserves_text_content() {
        let s = "<mn>1</mn><mo>+</mo><mn>2</mn>";
        assert_eq!(normalize_mathml(s), s);
    }
}

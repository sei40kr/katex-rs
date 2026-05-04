//! MathML output tree.
//!
//! Mirrors upstream KaTeX's `mathMLTree.ts`. Three node kinds —
//! [`MathMlNode::Element`], [`MathMlNode::Text`], and
//! [`MathMlNode::Space`] — cover every MathML construct the renderer
//! emits.
//!
//! # ADR — no `Display` / `ToString` impl
//!
//! Upstream's MathML tree exposes a single `toMarkup()` that performs
//! its own escaping. We follow the same policy with
//! [`MathMlElement::write_markup`] / [`MathMlNode::write_markup`] and a
//! convenience [`MathMlElement::to_markup`]. We deliberately do **not**
//! implement [`std::fmt::Display`]: that would invite `format!("{node}")`
//! at call sites, but MathML markup must be escaped differently
//! depending on where it lands (XML attribute value vs. text content),
//! and a single `Display` impl cannot pick the right policy. Keeping
//! the conversion explicit forces the choice at every call site.
//!
//! # Deviations from upstream
//!
//! - Upstream keeps a class-hierarchy of `MathNode`, `TextNode`,
//!   `SpaceNode`. We collapse those into a single enum so building a
//!   tree never crosses a trait-object boundary.
//! - Attributes upstream are a `string → string` object. We use
//!   `Vec<(SmolStr, SmolStr)>` so attribute order is deterministic and
//!   matches the order the builder added them — important for stable
//!   snapshot diffs against upstream output.

use std::fmt;

use smol_str::SmolStr;

use crate::units::make_em;

/// One MathML element. Carries its tag, an ordered list of attributes,
/// and an ordered list of child nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct MathMlElement {
    pub tag: SmolStr,
    pub attributes: Vec<(SmolStr, SmolStr)>,
    pub children: Vec<MathMlNode>,
}

/// A MathML tree node. Mirrors upstream's `MathDomNode` hierarchy
/// (`MathNode | TextNode | SpaceNode`).
#[derive(Clone, Debug, PartialEq)]
pub enum MathMlNode {
    /// `<tag ...>children</tag>`.
    Element(Box<MathMlElement>),
    /// Text content. Escaped on serialisation (no embedded markup).
    Text(String),
    /// Explicit horizontal space, in ems. Serialises as
    /// `<mspace width="…em"/>`. Mirrors upstream's `SpaceNode`.
    Space(f64),
}

impl MathMlElement {
    /// Empty element with a given tag.
    pub fn new(tag: impl Into<SmolStr>) -> Self {
        Self {
            tag: tag.into(),
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Element with the given children.
    pub fn with_children(tag: impl Into<SmolStr>, children: Vec<MathMlNode>) -> Self {
        Self {
            tag: tag.into(),
            attributes: Vec::new(),
            children,
        }
    }

    /// Set an attribute. If the key already exists its value is
    /// replaced; otherwise the pair is appended to preserve insertion
    /// order. Upstream's `setAttribute` has the same semantics.
    pub fn set_attribute(&mut self, key: impl Into<SmolStr>, value: impl Into<SmolStr>) {
        let key = key.into();
        let value = value.into();
        if let Some(slot) = self.attributes.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.attributes.push((key, value));
        }
    }

    /// Builder-style variant of [`set_attribute`].
    pub fn with_attribute(mut self, key: impl Into<SmolStr>, value: impl Into<SmolStr>) -> Self {
        self.set_attribute(key, value);
        self
    }

    /// Append a child.
    pub fn push(&mut self, child: MathMlNode) {
        self.children.push(child);
    }

    /// Convert to a [`MathMlNode::Element`].
    pub fn into_node(self) -> MathMlNode {
        MathMlNode::Element(Box::new(self))
    }

    /// Stream the XML markup for this element to `w`. Tags, attributes,
    /// and text content are escaped per the policy at the module top;
    /// children are written recursively. Self-closing form is used for
    /// elements with no children — same as upstream.
    pub fn write_markup(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        w.write_char('<')?;
        w.write_str(&self.tag)?;
        for (k, v) in &self.attributes {
            w.write_char(' ')?;
            w.write_str(k)?;
            w.write_str("=\"")?;
            escape_attr(v, w)?;
            w.write_char('"')?;
        }
        if self.children.is_empty() {
            w.write_str("/>")?;
            return Ok(());
        }
        w.write_char('>')?;
        for child in &self.children {
            child.write_markup(w)?;
        }
        w.write_str("</")?;
        w.write_str(&self.tag)?;
        w.write_char('>')
    }

    /// Allocate a fresh `String` and stream the markup into it.
    pub fn to_markup(&self) -> String {
        let mut s = String::new();
        self.write_markup(&mut s)
            .expect("write to String is infallible");
        s
    }
}

impl MathMlNode {
    /// Stream the XML markup for this node to `w`.
    pub fn write_markup(&self, w: &mut dyn fmt::Write) -> fmt::Result {
        match self {
            MathMlNode::Element(el) => el.write_markup(w),
            MathMlNode::Text(s) => escape_text(s, w),
            MathMlNode::Space(em) => {
                // Upstream emits `<mspace width="…em"/>` for non-zero
                // widths and an empty string for zero. We always emit
                // the `<mspace>` because Phase 6 callers create Space
                // nodes only when they want one. `make_em` already
                // appends the `em` suffix.
                w.write_str("<mspace width=\"")?;
                w.write_str(&make_em(*em))?;
                w.write_str("\"/>")
            }
        }
    }

    /// Allocate a fresh `String` and stream the markup into it.
    pub fn to_markup(&self) -> String {
        let mut s = String::new();
        self.write_markup(&mut s)
            .expect("write to String is infallible");
        s
    }
}

fn escape_text(s: &str, w: &mut dyn fmt::Write) -> fmt::Result {
    for ch in s.chars() {
        match ch {
            '<' => w.write_str("&lt;")?,
            '>' => w.write_str("&gt;")?,
            '&' => w.write_str("&amp;")?,
            _ => w.write_char(ch)?,
        }
    }
    Ok(())
}

fn escape_attr(s: &str, w: &mut dyn fmt::Write) -> fmt::Result {
    for ch in s.chars() {
        match ch {
            '<' => w.write_str("&lt;")?,
            '>' => w.write_str("&gt;")?,
            '&' => w.write_str("&amp;")?,
            '"' => w.write_str("&quot;")?,
            _ => w.write_char(ch)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_element_self_closes() {
        let el = MathMlElement::new("mspace");
        assert_eq!(el.to_markup(), "<mspace/>");
    }

    #[test]
    fn element_with_children_emits_open_close() {
        let el = MathMlElement::with_children(
            "mrow",
            vec![
                MathMlElement::with_children("mn", vec![MathMlNode::Text("1".into())]).into_node(),
                MathMlElement::with_children("mn", vec![MathMlNode::Text("2".into())]).into_node(),
            ],
        );
        assert_eq!(el.to_markup(), "<mrow><mn>1</mn><mn>2</mn></mrow>");
    }

    #[test]
    fn attributes_preserve_insertion_order_and_escape() {
        let el = MathMlElement::new("math")
            .with_attribute("xmlns", "http://www.w3.org/1998/Math/MathML")
            .with_attribute("display", "block");
        assert_eq!(
            el.to_markup(),
            "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\"/>"
        );
    }

    #[test]
    fn set_attribute_replaces_existing() {
        let mut el = MathMlElement::new("mo");
        el.set_attribute("stretchy", "false");
        el.set_attribute("stretchy", "true");
        assert_eq!(el.attributes.len(), 1);
        assert_eq!(el.to_markup(), "<mo stretchy=\"true\"/>");
    }

    #[test]
    fn text_content_is_xml_escaped() {
        let el =
            MathMlElement::with_children("mtext", vec![MathMlNode::Text("a < b & \"c\"".into())]);
        assert_eq!(el.to_markup(), "<mtext>a &lt; b &amp; \"c\"</mtext>");
    }

    #[test]
    fn attribute_value_escapes_quotes() {
        let el = MathMlElement::new("mo").with_attribute("title", "x \"y\" <z>");
        assert_eq!(el.to_markup(), "<mo title=\"x &quot;y&quot; &lt;z&gt;\"/>");
    }

    #[test]
    fn space_node_serialises_with_em_width() {
        let s = MathMlNode::Space(0.5);
        assert_eq!(s.to_markup(), "<mspace width=\"0.5em\"/>");
        let z = MathMlNode::Space(0.0);
        assert_eq!(z.to_markup(), "<mspace width=\"0em\"/>");
    }
}

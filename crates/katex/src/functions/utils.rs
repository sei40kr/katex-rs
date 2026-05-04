//! Small helpers shared by the function handlers. Mirrors the parts of
//! upstream KaTeX's `utils.ts` that the parser-side handlers depend on.

use crate::parse_node::ParseNode;

/// Walk through `ordgroup`/`color`/`font` wrappers around a single child
/// to find the innermost element. Mirrors upstream `getBaseElem`.
pub fn get_base_elem(group: &ParseNode) -> &ParseNode {
    match group {
        ParseNode::OrdGroup { body, .. } if body.len() == 1 => get_base_elem(&body[0]),
        ParseNode::Color { body, .. } if body.len() == 1 => get_base_elem(&body[0]),
        ParseNode::Font { body, .. } => get_base_elem(body),
        _ => group,
    }
}

/// `true` when `group`'s innermost element is a single character. Mirrors
/// upstream `isCharacterBox`. Used by mclass-style functions to decide
/// whether to treat the body as a single nucleus.
pub fn is_character_box(group: &ParseNode) -> bool {
    matches!(
        get_base_elem(group),
        ParseNode::MathOrd { .. } | ParseNode::TextOrd { .. } | ParseNode::Atom { .. }
    )
}

/// Pick `mclass` for `\boldsymbol` / `\@binrel` / `\stackrel` …, based on
/// the family of the inner atom. Mirrors upstream `binrelClass` in
/// `mclass.ts`. Defaults to `"mord"` when the body is not an atom.
pub fn binrel_class(arg: &ParseNode) -> &'static str {
    let atom = match arg {
        ParseNode::OrdGroup { body, .. } if !body.is_empty() => &body[0],
        other => other,
    };
    if let ParseNode::Atom { family, .. } = atom {
        match family {
            crate::tree::Atom::Bin => "mbin",
            crate::tree::Atom::Rel => "mrel",
            crate::tree::Atom::Open => "mopen",
            crate::tree::Atom::Close => "mclose",
            crate::tree::Atom::Punct => "mpunct",
            crate::tree::Atom::Inner => "minner",
        }
    } else {
        "mord"
    }
}

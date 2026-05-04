//! TeX math styles (display, text, script, scriptscript, plus cramped variants).
//!
//! Mirrors upstream KaTeX's `Style.js`/`Style.ts`. Upstream models styles as
//! eight singleton instances of a `Style` class indexed by integer id; we use
//! a `#[repr(u8)]` enum whose discriminants match upstream's id constants
//! `D=0, Dc=1, T=2, Tc=3, S=4, Sc=5, SS=6, SSc=7`, so `self as u8` is the
//! upstream id and the transition tables can index `[Style; 8]` directly.

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Style {
    Display = 0,
    DisplayCramped = 1,
    Text = 2,
    TextCramped = 3,
    Script = 4,
    ScriptCramped = 5,
    ScriptScript = 6,
    ScriptScriptCramped = 7,
}

use Style::*;

// Lookup tables, indexed by Style discriminant, copied verbatim from
// upstream Style.js. Order is [D, Dc, T, Tc, S, Sc, SS, SSc].
const SUP: [Style; 8] = [
    Script,
    ScriptCramped,
    Script,
    ScriptCramped,
    ScriptScript,
    ScriptScriptCramped,
    ScriptScript,
    ScriptScriptCramped,
];
const SUB: [Style; 8] = [
    ScriptCramped,
    ScriptCramped,
    ScriptCramped,
    ScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
];
const FRAC_NUM: [Style; 8] = [
    Text,
    TextCramped,
    Script,
    ScriptCramped,
    ScriptScript,
    ScriptScriptCramped,
    ScriptScript,
    ScriptScriptCramped,
];
const FRAC_DEN: [Style; 8] = [
    TextCramped,
    TextCramped,
    ScriptCramped,
    ScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
];
const CRAMP: [Style; 8] = [
    DisplayCramped,
    DisplayCramped,
    TextCramped,
    TextCramped,
    ScriptCramped,
    ScriptCramped,
    ScriptScriptCramped,
    ScriptScriptCramped,
];
const TEXT: [Style; 8] = [
    Display,
    DisplayCramped,
    Text,
    TextCramped,
    Text,
    TextCramped,
    Text,
    TextCramped,
];

impl Style {
    /// Numeric id matching upstream `Style.id` (0 through 7).
    #[inline]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// Style "size": same for cramped/uncramped pairs. 0=display, 1=text,
    /// 2=script, 3=scriptscript.
    #[inline]
    pub const fn size(self) -> u8 {
        self.id() / 2
    }

    #[inline]
    pub const fn cramped(self) -> bool {
        self.id() % 2 == 1
    }

    #[inline]
    pub fn sup(self) -> Style {
        SUP[self as usize]
    }

    #[inline]
    pub fn sub(self) -> Style {
        SUB[self as usize]
    }

    #[inline]
    pub fn frac_num(self) -> Style {
        FRAC_NUM[self as usize]
    }

    #[inline]
    pub fn frac_den(self) -> Style {
        FRAC_DEN[self as usize]
    }

    #[inline]
    pub fn cramp(self) -> Style {
        CRAMP[self as usize]
    }

    #[inline]
    pub fn text(self) -> Style {
        TEXT[self as usize]
    }

    /// Tightly spaced (script/scriptscript). Matches upstream `isTight()`.
    #[inline]
    pub const fn is_tight(self) -> bool {
        self.size() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::Style::*;

    #[test]
    fn ids_are_sequential() {
        let pairs = [
            (Display, 0),
            (DisplayCramped, 1),
            (Text, 2),
            (TextCramped, 3),
            (Script, 4),
            (ScriptCramped, 5),
            (ScriptScript, 6),
            (ScriptScriptCramped, 7),
        ];
        for (s, id) in pairs {
            assert_eq!(s.id(), id);
        }
    }

    #[test]
    fn size_and_cramped_match_upstream() {
        // upstream Style.js singletons:
        //   new Style(D,  0, false)
        //   new Style(Dc, 0, true)
        //   new Style(T,  1, false)
        //   new Style(Tc, 1, true)
        //   new Style(S,  2, false)
        //   new Style(Sc, 2, true)
        //   new Style(SS, 3, false)
        //   new Style(SSc,3, true)
        let expected = [
            (Display, 0, false),
            (DisplayCramped, 0, true),
            (Text, 1, false),
            (TextCramped, 1, true),
            (Script, 2, false),
            (ScriptCramped, 2, true),
            (ScriptScript, 3, false),
            (ScriptScriptCramped, 3, true),
        ];
        for (s, size, cramped) in expected {
            assert_eq!(s.size(), size, "{s:?}");
            assert_eq!(s.cramped(), cramped, "{s:?}");
        }
    }

    const ALL: [super::Style; 8] = [
        Display,
        DisplayCramped,
        Text,
        TextCramped,
        Script,
        ScriptCramped,
        ScriptScript,
        ScriptScriptCramped,
    ];

    #[test]
    fn sup_table_matches_upstream() {
        // const sup = [S, Sc, S, Sc, SS, SSc, SS, SSc];
        let expected = [
            Script,
            ScriptCramped,
            Script,
            ScriptCramped,
            ScriptScript,
            ScriptScriptCramped,
            ScriptScript,
            ScriptScriptCramped,
        ];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(ALL[i].sup(), *want);
        }
    }

    #[test]
    fn sub_table_matches_upstream() {
        // const sub = [Sc, Sc, Sc, Sc, SSc, SSc, SSc, SSc];
        let expected = [ScriptCramped; 4]
            .into_iter()
            .chain([ScriptScriptCramped; 4])
            .collect::<Vec<_>>();
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(ALL[i].sub(), *want);
        }
    }

    #[test]
    fn frac_num_table_matches_upstream() {
        // const fracNum = [T, Tc, S, Sc, SS, SSc, SS, SSc];
        let expected = [
            Text,
            TextCramped,
            Script,
            ScriptCramped,
            ScriptScript,
            ScriptScriptCramped,
            ScriptScript,
            ScriptScriptCramped,
        ];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(ALL[i].frac_num(), *want);
        }
    }

    #[test]
    fn frac_den_table_matches_upstream() {
        // const fracDen = [Tc, Tc, Sc, Sc, SSc, SSc, SSc, SSc];
        let expected = [
            TextCramped,
            TextCramped,
            ScriptCramped,
            ScriptCramped,
            ScriptScriptCramped,
            ScriptScriptCramped,
            ScriptScriptCramped,
            ScriptScriptCramped,
        ];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(ALL[i].frac_den(), *want);
        }
    }

    #[test]
    fn cramp_table_matches_upstream() {
        // const cramp = [Dc, Dc, Tc, Tc, Sc, Sc, SSc, SSc];
        let expected = [
            DisplayCramped,
            DisplayCramped,
            TextCramped,
            TextCramped,
            ScriptCramped,
            ScriptCramped,
            ScriptScriptCramped,
            ScriptScriptCramped,
        ];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(ALL[i].cramp(), *want);
        }
    }

    #[test]
    fn text_table_matches_upstream() {
        // const text = [D, Dc, T, Tc, T, Tc, T, Tc];
        let expected = [
            Display,
            DisplayCramped,
            Text,
            TextCramped,
            Text,
            TextCramped,
            Text,
            TextCramped,
        ];
        for (i, want) in expected.iter().enumerate() {
            assert_eq!(ALL[i].text(), *want);
        }
    }

    #[test]
    fn is_tight_matches_upstream() {
        assert!(!Display.is_tight());
        assert!(!DisplayCramped.is_tight());
        assert!(!Text.is_tight());
        assert!(!TextCramped.is_tight());
        assert!(Script.is_tight());
        assert!(ScriptCramped.is_tight());
        assert!(ScriptScript.is_tight());
        assert!(ScriptScriptCramped.is_tight());
    }
}

//! `eden-theme` — theme schema, parser, and built-in themes.
//!
//! A [`Theme`] is a name, an [`Appearance`], and a [`Palette`] of named colours.
//! Themes are plain TOML (so power users can hand-edit them) and there are three
//! hand-tuned built-ins: [`Theme::eden_day`], [`Theme::eden_dusk`], and
//! [`Theme::eden_noir`]. Palettes interpolate ([`Palette::lerp`]) so the UI can
//! crossfade between themes with the motion system rather than cutting.

mod color;

pub use color::{ColorParseError, Rgba8};

use serde::{Deserialize, Serialize};

/// Whether a theme reads as light or dark. Affects surface treatment (e.g. dark
/// themes get a soft shadow under floating panels; light themes stay flat).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// A light theme.
    Light,
    /// A dark theme.
    Dark,
}

/// The named colours a theme provides. Kept to what the editor chrome needs
/// today; it grows as later phases add surfaces (syntax, diagnostics, git).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    /// The editor canvas background.
    pub background: Rgba8,
    /// Panel surfaces such as the sidebar.
    pub surface: Rgba8,
    /// Raised surfaces such as the title bar and tab strip.
    pub surface_raised: Rgba8,
    /// The status bar background.
    pub status_bar: Rgba8,
    /// Primary text/foreground.
    pub text: Rgba8,
    /// Secondary, de-emphasised text.
    pub text_muted: Rgba8,
    /// Hairline dividers (carries its own low alpha).
    pub divider: Rgba8,
    /// The brand accent.
    pub accent: Rgba8,
    /// A secondary accent.
    pub accent_soft: Rgba8,
    /// Background of the active tab.
    pub tab_active: Rgba8,
    /// Selection / highlight wash (carries its own low alpha).
    pub selection: Rgba8,
}

impl Palette {
    /// Interpolates every colour toward `other` by `t` (clamped `0.0..=1.0`).
    #[must_use]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            background: self.background.lerp(other.background, t),
            surface: self.surface.lerp(other.surface, t),
            surface_raised: self.surface_raised.lerp(other.surface_raised, t),
            status_bar: self.status_bar.lerp(other.status_bar, t),
            text: self.text.lerp(other.text, t),
            text_muted: self.text_muted.lerp(other.text_muted, t),
            divider: self.divider.lerp(other.divider, t),
            accent: self.accent.lerp(other.accent, t),
            accent_soft: self.accent_soft.lerp(other.accent_soft, t),
            tab_active: self.tab_active.lerp(other.tab_active, t),
            selection: self.selection.lerp(other.selection, t),
        }
    }
}

/// Per-category syntax-highlighting colours. The UI maps tree-sitter highlight
/// kinds onto these fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Syntax {
    /// Language keywords.
    pub keyword: Rgba8,
    /// Function and method names.
    pub function: Rgba8,
    /// Types and type-like constructors.
    pub type_: Rgba8,
    /// Variables, parameters, fields.
    pub variable: Rgba8,
    /// Constants and builtins.
    pub constant: Rgba8,
    /// String literals (and escapes).
    pub string: Rgba8,
    /// Comments.
    pub comment: Rgba8,
    /// Operators.
    pub operator: Rgba8,
    /// Brackets, delimiters, punctuation.
    pub punctuation: Rgba8,
    /// Attributes / annotations.
    pub attribute: Rgba8,
}

impl Syntax {
    /// Interpolates every colour toward `other` by `t` (clamped `0.0..=1.0`).
    #[must_use]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            keyword: self.keyword.lerp(other.keyword, t),
            function: self.function.lerp(other.function, t),
            type_: self.type_.lerp(other.type_, t),
            variable: self.variable.lerp(other.variable, t),
            constant: self.constant.lerp(other.constant, t),
            string: self.string.lerp(other.string, t),
            comment: self.comment.lerp(other.comment, t),
            operator: self.operator.lerp(other.operator, t),
            punctuation: self.punctuation.lerp(other.punctuation, t),
            attribute: self.attribute.lerp(other.attribute, t),
        }
    }
}

/// A complete theme: identity plus colour [`Palette`] and [`Syntax`] tables.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// Human-readable name, e.g. `"Eden Day"`.
    pub name: String,
    /// Light or dark.
    pub appearance: Appearance,
    /// The chrome colour palette.
    pub palette: Palette,
    /// The syntax-highlighting colours.
    pub syntax: Syntax,
}

impl Theme {
    /// Parses a theme from a TOML document.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`toml`] error if the document is malformed or a
    /// colour fails to parse.
    pub fn from_toml_str(toml: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml)
    }

    /// Serialises this theme to a TOML document.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`toml`] error if serialisation fails.
    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// The three first-party themes, in presentation order.
    #[must_use]
    pub fn builtins() -> [Theme; 3] {
        [Self::eden_day(), Self::eden_dusk(), Self::eden_noir()]
    }

    /// **Eden Day** — warm paper white, kingfisher-blue accent (§6).
    #[must_use]
    pub fn eden_day() -> Self {
        Self {
            name: "Eden Day".to_owned(),
            appearance: Appearance::Light,
            palette: Palette {
                background: Rgba8::rgb(0xFB, 0xF8, 0xF3),
                surface: Rgba8::rgb(0xF4, 0xEF, 0xE7),
                surface_raised: Rgba8::rgb(0xFD, 0xFB, 0xF7),
                status_bar: Rgba8::rgb(0xF0, 0xEA, 0xE0),
                text: Rgba8::rgb(0x1B, 0x1B, 0x1F),
                text_muted: Rgba8::rgb(0x6B, 0x68, 0x62),
                // design: ink at ~8% — "dividers are text * 0.08, never pure black".
                divider: Rgba8::rgba(0x1B, 0x1B, 0x1F, 0x14),
                accent: Rgba8::rgb(0x2A, 0x6B, 0xC8),
                accent_soft: Rgba8::rgb(0xC7, 0x7B, 0x2C),
                tab_active: Rgba8::rgb(0xFB, 0xF8, 0xF3),
                selection: Rgba8::rgba(0x2A, 0x6B, 0xC8, 0x24),
            },
            syntax: Syntax {
                keyword: Rgba8::rgb(0x8A, 0x4F, 0xA0),
                function: Rgba8::rgb(0x2A, 0x6B, 0xC8),
                type_: Rgba8::rgb(0x2E, 0x7D, 0x6E),
                variable: Rgba8::rgb(0x2A, 0x2A, 0x30),
                constant: Rgba8::rgb(0xB2, 0x5E, 0x2A),
                string: Rgba8::rgb(0x3E, 0x7D, 0x44),
                comment: Rgba8::rgb(0x8A, 0x85, 0x7A),
                operator: Rgba8::rgb(0x6B, 0x68, 0x62),
                punctuation: Rgba8::rgb(0x6B, 0x68, 0x62),
                attribute: Rgba8::rgb(0x9A, 0x6B, 0x2C),
            },
        }
    }

    /// **Eden Dusk** — desaturated navy, off-white text, teal/rose accents (§6).
    #[must_use]
    pub fn eden_dusk() -> Self {
        Self {
            name: "Eden Dusk".to_owned(),
            appearance: Appearance::Dark,
            palette: Palette {
                background: Rgba8::rgb(0x1A, 0x1F, 0x2E),
                surface: Rgba8::rgb(0x15, 0x1A, 0x26),
                surface_raised: Rgba8::rgb(0x1F, 0x25, 0x35),
                status_bar: Rgba8::rgb(0x12, 0x16, 0x1F),
                text: Rgba8::rgb(0xE6, 0xE4, 0xDC),
                text_muted: Rgba8::rgb(0x8A, 0x8E, 0x9A),
                divider: Rgba8::rgba(0xE6, 0xE4, 0xDC, 0x12),
                accent: Rgba8::rgb(0x3E, 0x9C, 0x92),
                accent_soft: Rgba8::rgb(0xC7, 0x7B, 0x86),
                tab_active: Rgba8::rgb(0x1A, 0x1F, 0x2E),
                selection: Rgba8::rgba(0x3E, 0x9C, 0x92, 0x2E),
            },
            syntax: Syntax {
                keyword: Rgba8::rgb(0xC7, 0x92, 0xDB),
                function: Rgba8::rgb(0x6F, 0xB8, 0xC9),
                type_: Rgba8::rgb(0x7F, 0xC9, 0xA6),
                variable: Rgba8::rgb(0xE6, 0xE4, 0xDC),
                constant: Rgba8::rgb(0xE0, 0xA6, 0x6B),
                string: Rgba8::rgb(0x9C, 0xC7, 0x8A),
                comment: Rgba8::rgb(0x5E, 0x66, 0x75),
                operator: Rgba8::rgb(0xA6, 0xAB, 0xBA),
                punctuation: Rgba8::rgb(0x8A, 0x8E, 0x9A),
                attribute: Rgba8::rgb(0xD8, 0xB2, 0x6B),
            },
        }
    }

    /// **Eden Noir** — near-black, high contrast, a single molten-gold accent (§6).
    #[must_use]
    pub fn eden_noir() -> Self {
        Self {
            name: "Eden Noir".to_owned(),
            appearance: Appearance::Dark,
            palette: Palette {
                background: Rgba8::rgb(0x0E, 0x0E, 0x10),
                surface: Rgba8::rgb(0x0B, 0x0B, 0x0D),
                surface_raised: Rgba8::rgb(0x14, 0x14, 0x17),
                status_bar: Rgba8::rgb(0x0A, 0x0A, 0x0C),
                text: Rgba8::rgb(0xED, 0xED, 0xEA),
                text_muted: Rgba8::rgb(0x7A, 0x7A, 0x80),
                divider: Rgba8::rgba(0xED, 0xED, 0xEA, 0x10),
                accent: Rgba8::rgb(0xD9, 0xA4, 0x41),
                accent_soft: Rgba8::rgb(0xB8, 0x86, 0x2F),
                tab_active: Rgba8::rgb(0x0E, 0x0E, 0x10),
                selection: Rgba8::rgba(0xD9, 0xA4, 0x41, 0x26),
            },
            syntax: Syntax {
                keyword: Rgba8::rgb(0xD9, 0xA4, 0x41),
                function: Rgba8::rgb(0xC9, 0xC4, 0xB8),
                type_: Rgba8::rgb(0xB8, 0xA9, 0x8C),
                variable: Rgba8::rgb(0xED, 0xED, 0xEA),
                constant: Rgba8::rgb(0xD9, 0xA4, 0x41),
                string: Rgba8::rgb(0x9A, 0xA8, 0x8C),
                comment: Rgba8::rgb(0x5C, 0x5C, 0x60),
                operator: Rgba8::rgb(0x9A, 0x9A, 0x90),
                punctuation: Rgba8::rgb(0x7A, 0x7A, 0x80),
                attribute: Rgba8::rgb(0xB8, 0x86, 0x2F),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_round_trip_through_toml() {
        for theme in Theme::builtins() {
            let toml = theme.to_toml_string().expect("serialise");
            let parsed = Theme::from_toml_str(&toml).expect("parse");
            assert_eq!(parsed, theme, "{} did not round-trip", theme.name);
        }
    }

    #[test]
    fn shipped_theme_files_match_builtins() {
        let day = Theme::from_toml_str(include_str!("../../../themes/eden-day.toml")).unwrap();
        let dusk = Theme::from_toml_str(include_str!("../../../themes/eden-dusk.toml")).unwrap();
        let noir = Theme::from_toml_str(include_str!("../../../themes/eden-noir.toml")).unwrap();
        assert_eq!(day, Theme::eden_day());
        assert_eq!(dusk, Theme::eden_dusk());
        assert_eq!(noir, Theme::eden_noir());
    }

    #[test]
    fn palette_lerp_is_identity_at_endpoints() {
        let a = Theme::eden_day().palette;
        let b = Theme::eden_noir().palette;
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
    }
}

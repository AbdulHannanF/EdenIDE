//! `eden-theme` — theme schema, parser, and built-in themes.
//!
//! A [`Theme`] is a name, an [`Appearance`], and a [`Palette`] of named colours.
//! Themes are plain TOML (so power users can hand-edit them) and there are five
//! hand-tuned built-ins: [`Theme::eden_brutal_dark`], [`Theme::eden_day`],
//! [`Theme::eden_dusk`], [`Theme::eden_noir`], and [`Theme::eden_nothing`].
//! Palettes interpolate ([`Palette::lerp`]) so the UI can crossfade between
//! themes with the motion system rather than cutting.

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
    /// Very muted text — separators, ghost elements, line numbers.
    #[serde(default)]
    pub fg_dim: Rgba8,
    /// Elevated surface for hover backgrounds (between surface and surface_raised).
    #[serde(default)]
    pub surface_alt: Rgba8,
    /// Stronger border for active element outlines.
    #[serde(default)]
    pub border_strong: Rgba8,
    /// Solid muted accent for selection backgrounds and subdued highlights.
    #[serde(default)]
    pub accent_muted: Rgba8,
    /// Low-alpha accent for bloom/glow effects.
    #[serde(default)]
    pub accent_glow: Rgba8,
    /// Inactive tab background.
    #[serde(default)]
    pub tab_inactive: Rgba8,
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
            fg_dim: self.fg_dim.lerp(other.fg_dim, t),
            surface_alt: self.surface_alt.lerp(other.surface_alt, t),
            border_strong: self.border_strong.lerp(other.border_strong, t),
            accent_muted: self.accent_muted.lerp(other.accent_muted, t),
            accent_glow: self.accent_glow.lerp(other.accent_glow, t),
            tab_inactive: self.tab_inactive.lerp(other.tab_inactive, t),
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
    /// Control-flow keywords: if, else, for, while, return, match.
    #[serde(default)]
    pub keyword_control: Rgba8,
    /// Numeric literals (integers, floats).
    #[serde(default)]
    pub number: Rgba8,
    /// Macro invocations: println!, vec!, format!.
    #[serde(default)]
    pub macro_call: Rgba8,
    /// Lifetime annotations: 'a, 'static.
    #[serde(default)]
    pub lifetime: Rgba8,
    /// The `self` keyword.
    #[serde(default)]
    pub self_kw: Rgba8,
    /// Doc comments (///).
    #[serde(default)]
    pub doc_comment: Rgba8,
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
            keyword_control: self.keyword_control.lerp(other.keyword_control, t),
            number: self.number.lerp(other.number, t),
            macro_call: self.macro_call.lerp(other.macro_call, t),
            lifetime: self.lifetime.lerp(other.lifetime, t),
            self_kw: self.self_kw.lerp(other.self_kw, t),
            doc_comment: self.doc_comment.lerp(other.doc_comment, t),
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

    /// The first-party themes, in presentation order.
    #[must_use]
    pub fn builtins() -> [Theme; 5] {
        [Self::eden_brutal_dark(), Self::eden_day(), Self::eden_dusk(), Self::eden_noir(), Self::eden_nothing()]
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
                fg_dim: Rgba8::rgb(0xB0, 0xAC, 0xA6),
                surface_alt: Rgba8::rgb(0xED, 0xE9, 0xE1),
                border_strong: Rgba8::rgb(0xC8, 0xC4, 0xBC),
                accent_muted: Rgba8::rgba(0x2A, 0x6B, 0xC8, 0x20),
                accent_glow: Rgba8::rgba(0x2A, 0x6B, 0xC8, 0x10),
                tab_inactive: Rgba8::rgb(0xF4, 0xEF, 0xE7),
            },
            // design (D2): kingfisher keyword, amber string, deep-teal function,
            // muted-violet type, rose number/constant, ink-grey operator.
            syntax: Syntax {
                keyword: Rgba8::rgb(0x2A, 0x6B, 0xC8),
                function: Rgba8::rgb(0x1A, 0x7A, 0x5E),
                type_: Rgba8::rgb(0x7B, 0x4C, 0xA8),
                variable: Rgba8::rgb(0x2A, 0x2A, 0x30),
                constant: Rgba8::rgb(0xC7, 0x4C, 0x3C),
                string: Rgba8::rgb(0xC7, 0x7B, 0x2C),
                comment: Rgba8::rgb(0x8A, 0x8A, 0x8A),
                operator: Rgba8::rgb(0x4A, 0x4A, 0x4F),
                punctuation: Rgba8::rgb(0x6B, 0x68, 0x62),
                attribute: Rgba8::rgb(0x9A, 0x6B, 0x2C),
                keyword_control: Rgba8::rgb(0x2A, 0x6B, 0xC8),
                number: Rgba8::rgb(0xC7, 0x4C, 0x3C),
                macro_call: Rgba8::rgb(0x9A, 0x6B, 0x2C),
                lifetime: Rgba8::rgb(0x7B, 0x4C, 0xA8),
                self_kw: Rgba8::rgb(0x2A, 0x6B, 0xC8),
                doc_comment: Rgba8::rgb(0x4A, 0x7A, 0x4A),
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
                fg_dim: Rgba8::rgb(0x3A, 0x40, 0x50),
                surface_alt: Rgba8::rgb(0x24, 0x2A, 0x3C),
                border_strong: Rgba8::rgb(0x2D, 0x36, 0x48),
                accent_muted: Rgba8::rgba(0x3E, 0x9C, 0x92, 0x30),
                accent_glow: Rgba8::rgba(0x3E, 0x9C, 0x92, 0x15),
                tab_inactive: Rgba8::rgb(0x15, 0x1A, 0x26),
            },
            // design (D2): muted sky keyword, warm-gold string, soft-teal
            // function, lavender type, peach number, slate operator.
            syntax: Syntax {
                keyword: Rgba8::rgb(0x7E, 0xB8, 0xD4),
                function: Rgba8::rgb(0x7E, 0xC4, 0xA8),
                type_: Rgba8::rgb(0xB8, 0x9E, 0xCC),
                variable: Rgba8::rgb(0xE6, 0xE4, 0xDC),
                constant: Rgba8::rgb(0xE0, 0x90, 0x6A),
                string: Rgba8::rgb(0xC4, 0xA8, 0x6A),
                comment: Rgba8::rgb(0x4F, 0x5A, 0x72),
                operator: Rgba8::rgb(0x8A, 0x95, 0xA8),
                punctuation: Rgba8::rgb(0x8A, 0x8E, 0x9A),
                attribute: Rgba8::rgb(0xD8, 0xB2, 0x6B),
                keyword_control: Rgba8::rgb(0x7E, 0xB8, 0xD4),
                number: Rgba8::rgb(0xE0, 0x90, 0x6A),
                macro_call: Rgba8::rgb(0xD8, 0xB2, 0x6B),
                lifetime: Rgba8::rgb(0xB8, 0x9E, 0xCC),
                self_kw: Rgba8::rgb(0x7E, 0xB8, 0xD4),
                doc_comment: Rgba8::rgb(0x4F, 0x6A, 0x58),
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
                fg_dim: Rgba8::rgb(0x2E, 0x2E, 0x32),
                surface_alt: Rgba8::rgb(0x1A, 0x1A, 0x1E),
                border_strong: Rgba8::rgb(0x22, 0x22, 0x26),
                accent_muted: Rgba8::rgba(0xD9, 0xA4, 0x41, 0x26),
                accent_glow: Rgba8::rgba(0xD9, 0xA4, 0x41, 0x15),
                tab_inactive: Rgba8::rgb(0x0B, 0x0B, 0x0D),
            },
            // design (D2): molten-gold keyword, sage-green string, warm-sand
            // function, steel type, copper number.
            syntax: Syntax {
                keyword: Rgba8::rgb(0xD4, 0xAA, 0x60),
                function: Rgba8::rgb(0xC8, 0xA8, 0x82),
                type_: Rgba8::rgb(0x9A, 0xAB, 0xB8),
                variable: Rgba8::rgb(0xED, 0xED, 0xEA),
                constant: Rgba8::rgb(0xD4, 0x7A, 0x60),
                string: Rgba8::rgb(0x8F, 0xBF, 0x6A),
                comment: Rgba8::rgb(0x5C, 0x5C, 0x60),
                operator: Rgba8::rgb(0x5A, 0x5A, 0x62),
                punctuation: Rgba8::rgb(0x7A, 0x7A, 0x80),
                attribute: Rgba8::rgb(0xB8, 0x86, 0x2F),
                keyword_control: Rgba8::rgb(0xD4, 0xAA, 0x60),
                number: Rgba8::rgb(0xD4, 0x7A, 0x60),
                macro_call: Rgba8::rgb(0xC8, 0xA8, 0x82),
                lifetime: Rgba8::rgb(0x9A, 0xAB, 0xB8),
                self_kw: Rgba8::rgb(0xD4, 0xAA, 0x60),
                doc_comment: Rgba8::rgb(0x4A, 0x5A, 0x3A),
            },
        }
    }

    /// **Eden Nothing** — a Nothing-design tribute: pure black, white text, a
    /// restrained grayscale syntax ramp punctuated by the signature Nothing red.
    #[must_use]
    pub fn eden_nothing() -> Self {
        // design: Nothing's language is monochrome + a single red (#D71921).
        // Colour is spent sparingly: red marks the accent, numbers, constants,
        // and attributes; everything else is a calibrated grey ramp on black.
        let red = Rgba8::rgb(0xD7, 0x19, 0x21);
        Self {
            name: "Eden Nothing".to_owned(),
            appearance: Appearance::Dark,
            palette: Palette {
                background: Rgba8::rgb(0x00, 0x00, 0x00),
                surface: Rgba8::rgb(0x0A, 0x0A, 0x0A),
                surface_raised: Rgba8::rgb(0x16, 0x16, 0x16),
                status_bar: Rgba8::rgb(0x00, 0x00, 0x00),
                text: Rgba8::rgb(0xF5, 0xF5, 0xF5),
                text_muted: Rgba8::rgb(0x8A, 0x8A, 0x8A),
                divider: Rgba8::rgba(0xFF, 0xFF, 0xFF, 0x14),
                accent: red,
                accent_soft: Rgba8::rgb(0xE5, 0x48, 0x4D),
                tab_active: Rgba8::rgb(0x00, 0x00, 0x00),
                selection: Rgba8::rgba(0xD7, 0x19, 0x21, 0x2E),
                fg_dim: Rgba8::rgb(0x40, 0x40, 0x40),
                surface_alt: Rgba8::rgb(0x1A, 0x1A, 0x1A),
                border_strong: Rgba8::rgb(0x28, 0x28, 0x28),
                accent_muted: Rgba8::rgba(0xD7, 0x19, 0x21, 0x2E),
                accent_glow: Rgba8::rgba(0xD7, 0x19, 0x21, 0x15),
                tab_inactive: Rgba8::rgb(0x08, 0x08, 0x08),
            },
            syntax: Syntax {
                keyword: Rgba8::rgb(0xF5, 0xF5, 0xF5),
                function: Rgba8::rgb(0xCF, 0xCF, 0xCF),
                type_: Rgba8::rgb(0xB0, 0xB0, 0xB0),
                variable: Rgba8::rgb(0xE0, 0xE0, 0xE0),
                constant: red,
                string: Rgba8::rgb(0x9A, 0x9A, 0x9A),
                comment: Rgba8::rgb(0x55, 0x55, 0x55),
                operator: Rgba8::rgb(0x7A, 0x7A, 0x7A),
                punctuation: Rgba8::rgb(0x6A, 0x6A, 0x6A),
                attribute: red,
                keyword_control: Rgba8::rgb(0xF5, 0xF5, 0xF5),
                number: red,
                macro_call: Rgba8::rgb(0xCF, 0xCF, 0xCF),
                lifetime: Rgba8::rgb(0xB0, 0xB0, 0xB0),
                self_kw: red,
                doc_comment: Rgba8::rgb(0x55, 0x55, 0x55),
            },
        }
    }

    /// **Eden Brutal Dark** — near-black, red-orange accent, monospace editorial.
    #[must_use]
    pub fn eden_brutal_dark() -> Self {
        Self {
            name: "Eden Brutal Dark".to_owned(),
            appearance: Appearance::Dark,
            palette: Palette {
                background: Rgba8::rgb(0x0A, 0x0A, 0x0C),
                surface: Rgba8::rgb(0x11, 0x11, 0x14),
                surface_raised: Rgba8::rgb(0x18, 0x18, 0x1C),
                surface_alt: Rgba8::rgb(0x1C, 0x1C, 0x20),
                status_bar: Rgba8::rgb(0x0A, 0x0A, 0x0C),
                text: Rgba8::rgb(0xE8, 0xE6, 0xE0),
                text_muted: Rgba8::rgb(0x6B, 0x6B, 0x72),
                fg_dim: Rgba8::rgb(0x3A, 0x3A, 0x40),
                divider: Rgba8::rgb(0x25, 0x25, 0x28),
                border_strong: Rgba8::rgb(0x33, 0x33, 0x38),
                accent: Rgba8::rgb(0xE8, 0x34, 0x1C),
                accent_soft: Rgba8::rgb(0xC8, 0xA8, 0x82),
                accent_muted: Rgba8::rgb(0x7A, 0x1A, 0x0C),
                accent_glow: Rgba8::rgba(0xE8, 0x34, 0x1C, 0x22),
                tab_active: Rgba8::rgb(0x18, 0x18, 0x1C),
                tab_inactive: Rgba8::rgb(0x0E, 0x0E, 0x11),
                selection: Rgba8::rgba(0xE8, 0x34, 0x1C, 0x28),
            },
            syntax: Syntax {
                keyword: Rgba8::rgb(0xE8, 0x34, 0x1C),
                keyword_control: Rgba8::rgb(0xE8, 0x34, 0x1C),
                function: Rgba8::rgb(0xE8, 0xD4, 0xB0),
                type_: Rgba8::rgb(0x9A, 0xAB, 0xB8),
                variable: Rgba8::rgb(0xE8, 0xE6, 0xE0),
                constant: Rgba8::rgb(0xD4, 0x7A, 0x60),
                number: Rgba8::rgb(0xD4, 0x7A, 0x60),
                string: Rgba8::rgb(0x8F, 0xBF, 0x6A),
                comment: Rgba8::rgb(0x3D, 0x3D, 0x44),
                doc_comment: Rgba8::rgb(0x4A, 0x5A, 0x3A),
                operator: Rgba8::rgb(0x66, 0x66, 0x70),
                punctuation: Rgba8::rgb(0x6B, 0x6B, 0x72),
                attribute: Rgba8::rgb(0x5A, 0x7A, 0x9A),
                macro_call: Rgba8::rgb(0xC8, 0xA8, 0x82),
                lifetime: Rgba8::rgb(0x8A, 0x6A, 0x4A),
                self_kw: Rgba8::rgb(0xE8, 0x34, 0x1C),
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
        let brutal =
            Theme::from_toml_str(include_str!("../../../themes/eden-brutal-dark.toml")).unwrap();
        let day = Theme::from_toml_str(include_str!("../../../themes/eden-day.toml")).unwrap();
        let dusk = Theme::from_toml_str(include_str!("../../../themes/eden-dusk.toml")).unwrap();
        let noir = Theme::from_toml_str(include_str!("../../../themes/eden-noir.toml")).unwrap();
        let nothing =
            Theme::from_toml_str(include_str!("../../../themes/eden-nothing.toml")).unwrap();
        assert_eq!(brutal, Theme::eden_brutal_dark());
        assert_eq!(day, Theme::eden_day());
        assert_eq!(dusk, Theme::eden_dusk());
        assert_eq!(noir, Theme::eden_noir());
        assert_eq!(nothing, Theme::eden_nothing());
    }

    /// Regenerates the shipped `themes/*.toml` files from the built-ins. Ignored
    /// by default; run with `cargo test -p eden-theme regenerate -- --ignored`
    /// after editing a built-in palette.
    #[test]
    #[ignore]
    fn regenerate_theme_files() {
        for (theme, file) in [
            (Theme::eden_brutal_dark(), "eden-brutal-dark.toml"),
            (Theme::eden_day(), "eden-day.toml"),
            (Theme::eden_dusk(), "eden-dusk.toml"),
            (Theme::eden_noir(), "eden-noir.toml"),
            (Theme::eden_nothing(), "eden-nothing.toml"),
        ] {
            let toml = theme.to_toml_string().expect("serialise");
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../themes/");
            std::fs::write(format!("{path}{file}"), toml).expect("write theme");
        }
    }

    #[test]
    fn palette_lerp_is_identity_at_endpoints() {
        let a = Theme::eden_day().palette;
        let b = Theme::eden_noir().palette;
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
    }
}

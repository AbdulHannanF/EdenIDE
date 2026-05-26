//! sRGB colour with alpha, serialised as a CSS-style hex string.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An 8-bit-per-channel sRGB colour with straight (non-premultiplied) alpha.
///
/// Serialises to and from `#RRGGBB` or `#RRGGBBAA` so themes read naturally in
/// TOML. Interpolation ([`Rgba8::lerp`]) is done in sRGB space — not strictly
/// gamma-correct, but the right trade for short UI crossfades where it is
/// imperceptible and a great deal cheaper.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgba8 {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (`255` = opaque).
    pub a: u8,
}

impl Rgba8 {
    /// An opaque colour from red, green, and blue channels.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// A colour from red, green, blue, and alpha channels.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` (the leading `#` is optional).
    ///
    /// # Errors
    ///
    /// Returns [`ColorParseError`] if the string is not 6 or 8 hex digits.
    pub fn from_hex(s: &str) -> Result<Self, ColorParseError> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if !h.is_ascii() {
            return Err(ColorParseError(format!("not a hex colour: {s:?}")));
        }
        let byte = |i: usize| -> Result<u8, ColorParseError> {
            u8::from_str_radix(&h[i..i + 2], 16)
                .map_err(|_| ColorParseError(format!("invalid hex digits in {s:?}")))
        };
        match h.len() {
            6 => Ok(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Ok(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => Err(ColorParseError(format!(
                "expected #RRGGBB or #RRGGBBAA, got {s:?}"
            ))),
        }
    }

    /// Formats as `#RRGGBB` when opaque, otherwise `#RRGGBBAA`.
    #[must_use]
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    /// The four channels as `0.0..=1.0` floats, in `[r, g, b, a]` order.
    #[must_use]
    pub fn channels_f32(self) -> [f32; 4] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        ]
    }

    /// Linearly interpolates toward `other` by `t` (clamped to `0.0..=1.0`).
    #[must_use]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| {
            let a = f64::from(a);
            let b = f64::from(b);
            (a + (b - a) * t).round() as u8
        };
        Self {
            r: mix(self.r, other.r),
            g: mix(self.g, other.g),
            b: mix(self.b, other.b),
            a: mix(self.a, other.a),
        }
    }
}

impl fmt::Debug for Rgba8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgba8({})", self.to_hex())
    }
}

impl TryFrom<String> for Rgba8 {
    type Error = ColorParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_hex(&value)
    }
}

impl From<Rgba8> for String {
    fn from(value: Rgba8) -> Self {
        value.to_hex()
    }
}

/// Error returned when a hex colour string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorParseError(String);

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ColorParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_hex() {
        let c = Rgba8::rgb(0x2A, 0x6B, 0xC8);
        assert_eq!(c.to_hex(), "#2A6BC8");
        assert_eq!(Rgba8::from_hex("#2A6BC8").unwrap(), c);
        assert_eq!(Rgba8::from_hex("2a6bc8").unwrap(), c);
    }

    #[test]
    fn round_trips_with_alpha() {
        let c = Rgba8::rgba(0x1B, 0x1B, 0x1F, 0x14);
        assert_eq!(c.to_hex(), "#1B1B1F14");
        assert_eq!(Rgba8::from_hex(&c.to_hex()).unwrap(), c);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Rgba8::from_hex("#fff").is_err());
        assert!(Rgba8::from_hex("#gggggg").is_err());
        assert!(Rgba8::from_hex("").is_err());
    }

    #[test]
    fn lerp_endpoints_and_midpoint() {
        let a = Rgba8::rgb(0, 0, 0);
        let b = Rgba8::rgb(255, 255, 255);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), Rgba8::rgb(128, 128, 128));
    }
}

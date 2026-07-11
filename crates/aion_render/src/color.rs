//! Simple RGBA8 color for draw lists.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color(pub u32); // 0xRRGGBBAA

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32)
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 0xFF)
    }

    /// Parses "#RRGGBB" or "#RRGGBBAA".
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#')?;
        match s.len() {
            6 => {
                let v = u32::from_str_radix(s, 16).ok()?;
                Some(Color((v << 8) | 0xFF))
            }
            8 => u32::from_str_radix(s, 16).ok().map(Color),
            _ => None,
        }
    }

    pub const fn r(&self) -> u8 {
        (self.0 >> 24) as u8
    }
    pub const fn g(&self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub const fn b(&self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn a(&self) -> u8 {
        self.0 as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(Color::from_hex("#26a69a"), Some(Color::rgb(0x26, 0xa6, 0x9a)));
        assert_eq!(Color::from_hex("#26a69a80"), Some(Color::rgba(0x26, 0xa6, 0x9a, 0x80)));
        assert_eq!(Color::from_hex("oops"), None);
    }
}

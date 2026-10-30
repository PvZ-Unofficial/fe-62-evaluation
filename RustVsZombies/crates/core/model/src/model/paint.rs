//! Value types for native game-canvas drawing; no resources or global state.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Argb(pub u32);
impl Argb {
    pub const fn new(alpha: u8, red: u8, green: u8, blue: u8) -> Self {
        Self(u32::from_be_bytes([alpha, red, green, blue]))
    }
    pub const fn alpha(self) -> u8 {
        (self.0 >> 24) as u8
    }
    pub const fn red(self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub const fn green(self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub const fn blue(self) -> u8 {
        self.0 as u8
    }
}

/// Native pixel height, independent of the external modifier's UI font and zoom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaintFont<'a> {
    family: &'a str,
    pixels: u16,
    bold: bool,
}
impl<'a> PaintFont<'a> {
    pub fn new(family: &'a str, pixels: u16, bold: bool) -> Result<Self, PaintArgumentError> {
        if pixels == 0 {
            return Err(PaintArgumentError::ZeroFontSize);
        }
        if family.contains('\0') {
            return Err(PaintArgumentError::EmbeddedNul);
        }
        // Windows LOGFONTW reserves 32 UTF-16 code units including terminator.
        if family.encode_utf16().count() >= 32 {
            return Err(PaintArgumentError::FontNameTooLong);
        }
        Ok(Self { family, pixels, bold })
    }
    pub const fn family(self) -> &'a str {
        self.family
    }
    pub const fn pixels(self) -> u16 {
        self.pixels
    }
    pub const fn bold(self) -> bool {
        self.bold
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PaintArgumentError {
    #[error("font height must be nonzero")]
    ZeroFontSize,
    #[error("font family contains an embedded NUL")]
    EmbeddedNul,
    #[error("font family exceeds the native name field")]
    FontNameTooLong,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argb_components_and_native_font_name_boundary() {
        let color = Argb::new(0x9a, 0xff, 0x12, 0x34);
        assert_eq!(color.0, 0x9aff1234);
        assert_eq!(
            [color.alpha(), color.red(), color.green(), color.blue()],
            [0x9a, 0xff, 0x12, 0x34]
        );
        assert!(PaintFont::new("", 17, false).is_ok());
        assert!(PaintFont::new("Arial", 0, false).is_err());
        assert!(PaintFont::new("A\0B", 17, false).is_err());
        assert!(PaintFont::new(&"字".repeat(31), 17, false).is_ok());
        assert!(PaintFont::new(&"字".repeat(32), 17, false).is_err());
    }
}

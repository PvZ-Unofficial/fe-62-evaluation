//! Checked literal hints for the native 128-byte message widget.
#[derive(Clone, Copy, Debug)]
pub struct HintCaption<'a> {
    text: &'a str,
    duration: i32,
}
impl<'a> HintCaption<'a> {
    pub fn new(text: &'a str, duration: u32) -> Result<Self, &'static str> {
        if text.len() > 126 || text.as_bytes().contains(&0) {
            return Err("native hint must contain at most 126 bytes and no NUL");
        }
        // TodStringTranslate expands leading resource identifiers before the
        // native fixed-buffer copy. This literal API cannot validate that expansion.
        if text.len() >= 3 && text.starts_with('[') {
            return Err("native hint requires literal text, not a translation identifier");
        }
        let duration = i32::try_from(duration).map_err(|_| "hint duration exceeds native counter range")?;
        Ok(Self { text, duration })
    }
    pub const fn text(self) -> &'a str {
        self.text
    }
    pub const fn duration(self) -> i32 {
        self.duration
    }
}

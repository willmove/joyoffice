//! Text records: the font (LOGFONT), the text string decoded from its character set, and the
//! neutral [`Text`] item. The metafile crate knows no fonts, so measuring, alignment against the
//! font's metrics and glyph drawing are left to the renderer.

/// A font chosen by a CREATEFONTINDIRECT record (MS-WMF 2.2.1.2 Font, MS-EMF 2.2.13 LogFont).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Font {
    /// The face name the file asks for (`"Arial"`, `"Wingdings"`); may be empty.
    pub family: String,
    /// LOGFONT height in text units: negative is the character (em) height, positive the cell
    /// height (ascent plus descent), 0 the default size.
    pub height: f32,
    /// Average character width in text units; 0 keeps the font's own proportions.
    pub width: f32,
    /// 100..=900 (400 regular, 700 bold); 0 is the default weight.
    pub weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// LOGFONT character set (0 ANSI, 2 SYMBOL, 238 EASTEUROPE…).
    pub charset: u8,
}

/// SYMBOL_CHARSET: byte codes are the font's own symbol codes.
pub const SYMBOL_CHARSET: u8 = 2;

/// Where the reference point sits on the string horizontally (TA_LEFT, TA_CENTER, TA_RIGHT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Where the reference point sits vertically (TA_TOP, TA_BASELINE, TA_BOTTOM): the top of the
/// character cell, the baseline or the bottom of the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Top,
    Baseline,
    Bottom,
}

/// One string drawn by EXTTEXTOUT, TEXTOUT or SMALLTEXTOUT.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    /// Text space to picture space as affine coefficients `[a, b, c, d, e, f]` (`x' = a·x + c·y + e`,
    /// `y' = b·x + d·y + f`, kurbo's order). Text space has x along the baseline and y down, in the
    /// metafile's logical units, with the reference point at the origin. Escapement and the
    /// mapping are folded in; glyphs are never mirrored.
    pub transform: [f64; 6],
    pub text: String,
    /// Advance after each character of `text`, in text units (the dx array). Empty when the file
    /// gives none: the font's own advances apply.
    pub dx: Vec<f32>,
    pub font: Font,
    pub color: [u8; 4],
    /// Colour filled behind the character cells (OPAQUE background mode); None when transparent.
    pub background: Option<[u8; 4]>,
    pub h_align: HAlign,
    pub v_align: VAlign,
    /// Clip rectangle `[x0, y0, x1, y1]` in picture units (ETO_CLIPPED).
    pub clip: Option<[f32; 4]>,
    /// Clip region from the device context, in picture units. None is the default clip. Drawn
    /// together with [`Text::clip`] when both are set.
    pub region: Option<Vec<[f32; 4]>>,
}

/// ETO_OPAQUE fills the rectangle with the background colour; ETO_CLIPPED clips the text to it.
pub(crate) const ETO_OPAQUE: u32 = 0x0002;
pub(crate) const ETO_CLIPPED: u32 = 0x0004;
/// The string holds glyph indices, not characters (drawn only when the font matches).
pub(crate) const ETO_GLYPH_INDEX: u32 = 0x0010;
/// The dx array holds (dx, dy) pairs.
pub(crate) const ETO_PDY: u32 = 0x2000;
/// Longest string kept from one record.
pub(crate) const MAX_CHARS: usize = 16_384;

pub(crate) use crate::text_encoding::{decode_bytes, decode_utf16};

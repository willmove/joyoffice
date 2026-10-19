//! Font identity (ARCHITECTURE §6.1). A font is the exact bytes plus the
//! face index; two fonts with the same family name but different bytes are
//! different fonts. Variation axes join the key when variable fonts arrive.

use std::fmt;
use std::sync::Arc;

use read_fonts::{FontRef, TableProvider};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontKey {
    hash: [u8; 32],
    index: u32,
}

impl FontKey {
    pub fn index(&self) -> u32 {
        self.index
    }

    /// A key that does not name a loaded face. Cache tests use it to show
    /// that two external contexts alias when a dependency flag is dropped.
    #[cfg(test)]
    pub(crate) fn from_raw_parts(hash: [u8; 32], index: u32) -> Self {
        Self { hash, index }
    }

    /// SHA-256 of the font file, hex encoded.
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl fmt::Debug for FontKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FontKey({}…#{})", &self.hash_hex()[..12], self.index)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontError {
    /// Not a font, or the face index does not exist.
    Unreadable,
    MissingTable(&'static str),
}

/// A loaded face with the metrics layout needs, in font units.
#[derive(Clone)]
pub struct LoadedFont {
    key: FontKey,
    data: Arc<[u8]>,
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
}

impl fmt::Debug for LoadedFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedFont")
            .field("key", &self.key)
            .field("units_per_em", &self.units_per_em)
            .finish()
    }
}

impl LoadedFont {
    pub fn from_bytes(data: Arc<[u8]>, index: u32) -> Result<Self, FontError> {
        let font = FontRef::from_index(&data, index).map_err(|_| FontError::Unreadable)?;
        let head = font.head().map_err(|_| FontError::MissingTable("head"))?;
        let hhea = font.hhea().map_err(|_| FontError::MissingTable("hhea"))?;
        let units_per_em = head.units_per_em();
        if units_per_em == 0 {
            return Err(FontError::Unreadable);
        }
        let (ascender, descender, line_gap) = (
            hhea.ascender().to_i16(),
            hhea.descender().to_i16(),
            hhea.line_gap().to_i16(),
        );
        let key = FontKey {
            hash: Sha256::digest(&data).into(),
            index,
        };
        Ok(Self {
            key,
            data,
            units_per_em,
            ascender,
            descender,
            line_gap,
        })
    }

    pub fn key(&self) -> FontKey {
        self.key
    }

    pub fn data(&self) -> &Arc<[u8]> {
        &self.data
    }

    pub fn font_ref(&self) -> FontRef<'_> {
        FontRef::from_index(&self.data, self.key.index).expect("validated when loaded")
    }

    /// Underline position and thickness for this face.
    ///
    /// OpenType `post` (`underlinePosition`, `underlineThickness`) is the
    /// primary source: the position is the top of the stroke relative to the
    /// baseline in y-up font units, and a negative value is below the
    /// baseline. The `post` specification says that thickness should match
    /// `OS/2.yStrikeoutSize`, so a missing or non-positive post thickness
    /// uses that strikeout size when it is positive.
    ///
    /// When neither table supplies a positive thickness, the thickness is
    /// `unitsPerEm / 20` (at least 1) and the top of the stroke sits one
    /// thickness below the baseline. A post table with a positive thickness
    /// keeps its position even when that position is zero (the stroke top
    /// is on the baseline). These fallbacks are JoyWord's, not a Word
    /// measurement.
    ///
    /// Sources: OpenType `post`
    /// <https://learn.microsoft.com/en-us/typography/opentype/spec/post> and
    /// `OS/2` <https://learn.microsoft.com/en-us/typography/opentype/spec/os2>.
    pub fn underline_metrics(&self) -> UnderlineMetrics {
        let post = self.font_ref().post().ok().map(|post| {
            (
                i32::from(post.underline_position().to_i16()),
                i32::from(post.underline_thickness().to_i16()),
            )
        });
        let strikeout = self
            .font_ref()
            .os2()
            .ok()
            .map(|table| i32::from(table.y_strikeout_size()));
        resolve_underline_metrics(post, strikeout, self.units_per_em)
    }
}

/// Where [`UnderlineMetrics`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnderlineMetricSource {
    /// `post.underlinePosition` and `post.underlineThickness`.
    Post,
    /// Thickness from `OS/2.yStrikeoutSize`. Position is the fallback below.
    Os2StrikeoutSize,
    /// `unitsPerEm / 20`, top one thickness below the baseline.
    EmFallback,
}

/// Resolved underline stroke in y-up font units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnderlineMetrics {
    /// Top of the stroke relative to the baseline. Negative is below.
    pub position: i32,
    pub thickness: i32,
    pub source: UnderlineMetricSource,
}

/// `post` is `(position, thickness)` when the table was read. `strikeout` is
/// `OS/2.yStrikeoutSize` when that table was read. See [`LoadedFont::underline_metrics`].
pub fn resolve_underline_metrics(
    post: Option<(i32, i32)>,
    strikeout: Option<i32>,
    units_per_em: u16,
) -> UnderlineMetrics {
    if let Some((position, thickness)) = post.filter(|(_, thickness)| *thickness > 0) {
        return UnderlineMetrics {
            position,
            thickness,
            source: UnderlineMetricSource::Post,
        };
    }
    let em = i32::from(units_per_em).max(1);
    let fallback_thickness = (em / 20).max(1);
    if let Some(thickness) = strikeout.filter(|thickness| *thickness > 0) {
        return UnderlineMetrics {
            position: -thickness,
            thickness,
            source: UnderlineMetricSource::Os2StrikeoutSize,
        };
    }
    UnderlineMetrics {
        position: -fallback_thickness,
        thickness: fallback_thickness,
        source: UnderlineMetricSource::EmFallback,
    }
}

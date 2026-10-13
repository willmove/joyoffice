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
}

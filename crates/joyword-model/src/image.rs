//! Raster image admission (QUALITY-005, CLIP-001, LAYOUT-006).
//!
//! Bytes are sniffed and decoded under a pixel budget before they can become
//! a resource. SVG, HTML, EMF, WMF, OLE and URL payloads are refused without
//! being interpreted. Nothing in this module opens a path or a network
//! address.
//!
//! Sources: PNG (ISO/IEC 15948), JPEG (ITU-T T.81), GIF89a, BMP and TIFF
//! signatures; ECMA-376 Part 1 §20.4.2.7 (`a:blip`) and §17.3.3.19 (`w:drawing`)
//! for the EMU extent that import stores separately.

use std::io::Cursor;
use std::sync::Arc;

use image::{DynamicImage, ImageFormat, ImageReader};

use joyword_foundation::Emu;

use crate::props::Measure;

/// One image, uncompressed, before it joins the package budget.
pub const MAX_IMAGE_BYTES: u64 = 32 << 20;
/// Width times height. Checked on the header before a pixel buffer is allocated.
pub const MAX_IMAGE_PIXELS: u64 = 25_000_000;
pub const MAX_IMAGE_DIMENSION: u32 = 16_384;
/// A drawing extent larger than this is not a page image this slice will place.
const MAX_EXTENT_EMU: i64 = 100 * Emu::PER_INCH;
/// Newly inserted images are shown at 96 dpi, then shrunk so the long side
/// is at most six inches. The stored bytes stay the original file.
const INSERT_DPI: i64 = 96;
const MAX_INSERT_SIDE: i64 = 6 * Emu::PER_INCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageLimits {
    pub max_bytes: u64,
    pub max_pixels: u64,
    pub max_dimension: u32,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_IMAGE_BYTES,
            max_pixels: MAX_IMAGE_PIXELS,
            max_dimension: MAX_IMAGE_DIMENSION,
        }
    }
}

impl ImageLimits {
    fn capped(self) -> Self {
        let hard = Self::default();
        Self {
            max_bytes: self.max_bytes.min(hard.max_bytes),
            max_pixels: self.max_pixels.min(hard.max_pixels),
            max_dimension: self.max_dimension.min(hard.max_dimension),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedImage {
    pub media_type: &'static str,
    pub width_px: u32,
    pub height_px: u32,
    pub data: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRefuse {
    Empty,
    OversizeBytes {
        size: u64,
        limit: u64,
    },
    OversizePixels {
        pixels: u64,
        limit: u64,
    },
    OversizeDimension {
        edge: u32,
        limit: u32,
    },
    Malformed,
    /// SVG, HTML, EMF, WMF, OLE or another payload this slice will not run.
    Active,
    /// The bytes are a URL or file reference, not an image.
    Remote,
}

impl ImageRefuse {
    pub fn reason(self) -> &'static str {
        match self {
            Self::Empty => "empty image",
            Self::OversizeBytes { .. } => "image exceeds the byte budget",
            Self::OversizePixels { .. } => "image exceeds the pixel budget",
            Self::OversizeDimension { .. } => "image exceeds the dimension budget",
            Self::Malformed => "malformed image",
            Self::Active => "active image content",
            Self::Remote => "remote image",
        }
    }
}

impl std::fmt::Display for ImageRefuse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

#[derive(Clone, Copy)]
enum RasterKind {
    Png,
    Jpeg,
    Gif,
    Bmp,
    Tiff,
}

impl RasterKind {
    fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
        }
    }

    fn format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Gif => ImageFormat::Gif,
            Self::Bmp => ImageFormat::Bmp,
            Self::Tiff => ImageFormat::Tiff,
        }
    }
}

/// Admit `bytes` as a still raster image, or refuse them.
///
/// The pixel buffer is decoded to prove the file is complete, then dropped.
/// Call [`decode_rgba`] when a consumer needs the pixels.
pub fn accept_image(bytes: &[u8], limits: &ImageLimits) -> Result<AcceptedImage, ImageRefuse> {
    let (image, _) = inspect(bytes, limits)?;
    Ok(image)
}

/// Decode an already-admitted raster to straight RGBA8.
pub fn decode_rgba(bytes: &[u8], limits: &ImageLimits) -> Result<DecodedImage, ImageRefuse> {
    let (image, decoded) = inspect(bytes, limits)?;
    let rgba = decoded.to_rgba8();
    Ok(DecodedImage {
        width: image.width_px,
        height: image.height_px,
        rgba: rgba.into_raw(),
    })
}

/// Display size for a newly inserted image: 96 dpi, long side at most 6 inches.
pub fn display_extent(width_px: u32, height_px: u32) -> (Measure, Measure) {
    let emu = |px: u32| (i64::from(px)).saturating_mul(Emu::PER_INCH / INSERT_DPI);
    let mut width = emu(width_px).max(1);
    let mut height = emu(height_px).max(1);
    let long = width.max(height);
    if long > MAX_INSERT_SIDE {
        width = width.saturating_mul(MAX_INSERT_SIDE) / long;
        height = height.saturating_mul(MAX_INSERT_SIDE) / long;
        width = width.max(1);
        height = height.max(1);
    }
    (Measure::Emu(width), Measure::Emu(height))
}

/// Page-relative top-left of a floating picture (ECMA-376 `wp:anchor`).
///
/// Offsets are EMU from the page origin. This slice stores no wrap mode and
/// no z-order: callers only build it for `wp:wrapNone` in front of the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageAnchor {
    pub x: Measure,
    pub y: Measure,
}

/// A page anchor inside the same extent ceiling as [`extent_emu`].
/// Zero is the page origin. Negative offsets are not this slice.
pub fn page_anchor(x: i64, y: i64) -> Result<ImageAnchor, ImageRefuse> {
    if x < 0 || y < 0 {
        return Err(ImageRefuse::Malformed);
    }
    if x > MAX_EXTENT_EMU || y > MAX_EXTENT_EMU {
        let edge = u32::try_from(x.max(y)).unwrap_or(u32::MAX);
        return Err(ImageRefuse::OversizeDimension {
            edge,
            limit: u32::try_from(MAX_EXTENT_EMU).unwrap_or(u32::MAX),
        });
    }
    Ok(ImageAnchor {
        x: Measure::Emu(x),
        y: Measure::Emu(y),
    })
}

/// An OOXML extent (`cx`/`cy` in EMU). Non-positive or absurd values are refused.
pub fn extent_emu(cx: i64, cy: i64) -> Result<(Measure, Measure), ImageRefuse> {
    if cx <= 0 || cy <= 0 {
        return Err(ImageRefuse::Malformed);
    }
    if cx > MAX_EXTENT_EMU || cy > MAX_EXTENT_EMU {
        let edge = u32::try_from(cx.max(cy)).unwrap_or(u32::MAX);
        return Err(ImageRefuse::OversizeDimension {
            edge,
            limit: u32::try_from(MAX_EXTENT_EMU).unwrap_or(u32::MAX),
        });
    }
    Ok((Measure::Emu(cx), Measure::Emu(cy)))
}

/// A valid PNG whose width is above [`MAX_IMAGE_DIMENSION`].
///
/// The header is enough to refuse it; callers must not need the pixel buffer.
pub fn sample_png_overwide() -> Result<Vec<u8>, &'static str> {
    let width = MAX_IMAGE_DIMENSION + 1;
    let image = image::RgbImage::from_raw(width, 1, vec![0; width as usize * 3]).ok_or("buffer")?;
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|_| "encode")?;
    Ok(bytes)
}

/// 1×1 opaque red PNG. The bytes are fixed so tests do not depend on an Office file.
pub fn sample_png_1x1() -> Vec<u8> {
    // Filter byte 0, then RGB 255,0,0. Generated once; admission tests decode it.
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2,
        0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 218, 99, 248, 207, 192, 0, 0,
        3, 1, 1, 0, 247, 3, 65, 67, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    PNG.to_vec()
}

fn inspect(
    bytes: &[u8],
    limits: &ImageLimits,
) -> Result<(AcceptedImage, DynamicImage), ImageRefuse> {
    let limits = limits.capped();
    if bytes.is_empty() {
        return Err(ImageRefuse::Empty);
    }
    let size = bytes.len() as u64;
    if size > limits.max_bytes {
        return Err(ImageRefuse::OversizeBytes {
            size,
            limit: limits.max_bytes,
        });
    }
    if is_remote_reference(bytes) {
        return Err(ImageRefuse::Remote);
    }
    if is_active(bytes) {
        return Err(ImageRefuse::Active);
    }
    let kind = sniff(bytes).ok_or(ImageRefuse::Malformed)?;
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), kind.format())
        .into_dimensions()
        .map_err(|_| ImageRefuse::Malformed)?;
    if width == 0 || height == 0 {
        return Err(ImageRefuse::Malformed);
    }
    if width > limits.max_dimension || height > limits.max_dimension {
        return Err(ImageRefuse::OversizeDimension {
            edge: width.max(height),
            limit: limits.max_dimension,
        });
    }
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > limits.max_pixels {
        return Err(ImageRefuse::OversizePixels {
            pixels,
            limit: limits.max_pixels,
        });
    }
    let decoded = ImageReader::with_format(Cursor::new(bytes), kind.format())
        .decode()
        .map_err(|_| ImageRefuse::Malformed)?;
    if decoded.width() != width || decoded.height() != height {
        return Err(ImageRefuse::Malformed);
    }
    Ok((
        AcceptedImage {
            media_type: kind.media_type(),
            width_px: width,
            height_px: height,
            data: Arc::from(bytes),
        },
        decoded,
    ))
}

fn sniff(bytes: &[u8]) -> Option<RasterKind> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(RasterKind::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(RasterKind::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(RasterKind::Gif)
    } else if bytes.starts_with(b"BM") {
        Some(RasterKind::Bmp)
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        Some(RasterKind::Tiff)
    } else {
        None
    }
}

fn is_remote_reference(bytes: &[u8]) -> bool {
    let text = trim_bom(bytes);
    let lower = text.to_ascii_lowercase();
    lower.starts_with(b"http://")
        || lower.starts_with(b"https://")
        || lower.starts_with(b"file://")
        || lower.starts_with(b"ftp://")
        || lower.starts_with(br"\\")
        || lower.starts_with(b"//")
}

fn is_active(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return true;
    }
    if bytes.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        return true;
    }
    if bytes.len() >= 44 && bytes.get(40..44) == Some(b" EMF") {
        return true;
    }
    let text = trim_bom(bytes);
    let head = text
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        .map(|index| &text[index..])
        .unwrap_or(text);
    let lower: Vec<u8> = head.iter().take(64).map(u8::to_ascii_lowercase).collect();
    lower.starts_with(b"<svg")
        || lower.starts_with(b"<html")
        || lower.starts_with(b"<script")
        || lower.starts_with(b"<?xml")
        || lower.starts_with(b"<!doctype")
        || lower.starts_with(b"{\\rtf")
        || lower.starts_with(b"%pdf-")
        || lower.starts_with(b"javascript:")
}

fn trim_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

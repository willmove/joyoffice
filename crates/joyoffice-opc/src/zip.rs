//! Minimal, strict ZIP reader and writer for OPC packages (PKWARE APPNOTE
//! 6.3.x structures; ECMA-376 Part 2 restricts packages to stored and
//! deflated items without encryption or spanning).
//!
//! The reader trusts nothing in the archive: the central directory is
//! bounds-checked, local headers must agree with it, item data ranges may not
//! overlap (which defeats overlapping-entry "zip bombs"), declared sizes cap
//! decompression, and the CRC-32 is verified. Budgets are applied before any
//! data is inflated.

use std::io::{Read, Write};
use std::ops::Range;

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

use crate::budget::{Budget, BudgetError};

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL: u32 = 0x0605_4b50;
const ZIP64_END: u32 = 0x0606_4b50;
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
const ZIP64_EXTRA: u16 = 0x0001;
const FLAG_ENCRYPTED: u16 = 0x0001;
const FLAG_STRONG_ENCRYPTION: u16 = 0x0040;
const FLAG_UTF8: u16 = 0x0800;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZipError {
    /// No end-of-central-directory record: not a ZIP archive.
    NotZip,
    /// A structure points outside the file or is cut short.
    Truncated(&'static str),
    /// Spanned archives, encryption or a compression method other than
    /// stored and deflate.
    Unsupported(&'static str),
    /// Structures disagree or overlap.
    Malformed(&'static str),
    /// Item data does not match its declared size or CRC-32.
    Corrupt {
        name: String,
    },
    Budget(BudgetError),
}

impl From<BudgetError> for ZipError {
    fn from(error: BudgetError) -> Self {
        ZipError::Budget(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Stored,
    Deflated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipEntry {
    /// Item name as stored (UTF-8).
    pub name: String,
    pub method: Method,
    pub crc32: u32,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
    /// Byte range of the item's local header and compressed data.
    data: Range<usize>,
}

/// Little-endian cursor that reports truncation instead of panicking.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    what: &'static str,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], at: usize, what: &'static str) -> Self {
        Self { bytes, at, what }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ZipError> {
        let end = self
            .at
            .checked_add(len)
            .ok_or(ZipError::Truncated(self.what))?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(ZipError::Truncated(self.what))?;
        self.at = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, ZipError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two bytes"),
        ))
    }

    fn u32(&mut self) -> Result<u32, ZipError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, ZipError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }
}

fn to_usize(value: u64, what: &'static str) -> Result<usize, ZipError> {
    usize::try_from(value).map_err(|_| ZipError::Truncated(what))
}

/// Locates the central directory: (entry count, offset, size).
fn central_directory(bytes: &[u8]) -> Result<(u64, u64, u64), ZipError> {
    // The EOCD record is 22 bytes plus a comment of at most 65 535 bytes.
    if bytes.len() < 22 {
        return Err(ZipError::NotZip);
    }
    let search_start = bytes.len().saturating_sub(22 + u16::MAX as usize);
    let eocd = (search_start..=bytes.len() - 22)
        .rev()
        .find(|&at| bytes.get(at..at + 4) == Some(&END_OF_CENTRAL.to_le_bytes()[..]))
        .ok_or(ZipError::NotZip)?;
    let mut cursor = Cursor::new(bytes, eocd + 4, "end of central directory");
    let disk = cursor.u16()?;
    let cd_disk = cursor.u16()?;
    let entries_here = cursor.u16()?;
    let entries = cursor.u16()?;
    let size = cursor.u32()?;
    let offset = cursor.u32()?;
    if disk != 0 || cd_disk != 0 || entries_here != entries {
        return Err(ZipError::Unsupported("spanned archive"));
    }
    let needs_zip64 = entries == u16::MAX || size == u32::MAX || offset == u32::MAX;
    let locator = eocd.checked_sub(20);
    let has_locator =
        locator.is_some_and(|at| bytes.get(at..at + 4) == Some(&ZIP64_LOCATOR.to_le_bytes()[..]));
    if !has_locator {
        if needs_zip64 {
            return Err(ZipError::Malformed("ZIP64 values without a ZIP64 record"));
        }
        return Ok((u64::from(entries), u64::from(offset), u64::from(size)));
    }
    let mut cursor = Cursor::new(bytes, locator.expect("checked") + 4, "ZIP64 locator");
    if cursor.u32()? != 0 {
        return Err(ZipError::Unsupported("spanned archive"));
    }
    let record = to_usize(cursor.u64()?, "ZIP64 locator")?;
    if cursor.u32()? > 1 {
        return Err(ZipError::Unsupported("spanned archive"));
    }
    let mut cursor = Cursor::new(bytes, record, "ZIP64 end of central directory");
    if cursor.u32()? != ZIP64_END {
        return Err(ZipError::Malformed("bad ZIP64 end of central directory"));
    }
    let _record_size = cursor.u64()?;
    let _made_by = cursor.u16()?;
    let _needed = cursor.u16()?;
    if cursor.u32()? != 0 || cursor.u32()? != 0 {
        return Err(ZipError::Unsupported("spanned archive"));
    }
    let entries_here = cursor.u64()?;
    let entries = cursor.u64()?;
    let size = cursor.u64()?;
    let offset = cursor.u64()?;
    if entries_here != entries {
        return Err(ZipError::Unsupported("spanned archive"));
    }
    Ok((entries, offset, size))
}

/// Parsed central directory over borrowed archive bytes.
pub struct ZipArchive<'a> {
    bytes: &'a [u8],
    entries: Vec<ZipEntry>,
}

impl<'a> ZipArchive<'a> {
    pub fn new(bytes: &'a [u8], budget: &Budget) -> Result<Self, ZipError> {
        let (count, offset, size) = central_directory(bytes)?;
        budget.check_entries(count)?;
        let start = to_usize(offset, "central directory")?;
        let end = start
            .checked_add(to_usize(size, "central directory")?)
            .filter(|&end| end <= bytes.len())
            .ok_or(ZipError::Truncated("central directory"))?;
        let mut cursor = Cursor::new(&bytes[..end], start, "central directory");
        let mut entries = Vec::with_capacity(count as usize);
        let mut declared_total: u64 = 0;
        for _ in 0..count {
            let entry = Self::central_entry(bytes, &mut cursor)?;
            budget.check_part(&entry.name, entry.compressed_size, entry.uncompressed_size)?;
            declared_total = declared_total.saturating_add(entry.uncompressed_size);
            budget.check_total(declared_total)?;
            entries.push(entry);
        }
        // Item data ranges (with their local headers) may not overlap each
        // other or the central directory.
        let mut ranges: Vec<Range<usize>> = entries.iter().map(|e| e.data.clone()).collect();
        ranges.sort_by_key(|range| range.start);
        for pair in ranges.windows(2) {
            if pair[0].end > pair[1].start {
                return Err(ZipError::Malformed("overlapping items"));
            }
        }
        if ranges.last().is_some_and(|last| last.end > start) {
            return Err(ZipError::Malformed(
                "item data overlaps the central directory",
            ));
        }
        Ok(Self { bytes, entries })
    }

    fn central_entry(bytes: &[u8], cursor: &mut Cursor<'_>) -> Result<ZipEntry, ZipError> {
        if cursor.u32()? != CENTRAL_HEADER {
            return Err(ZipError::Malformed("bad central directory header"));
        }
        let _made_by = cursor.u16()?;
        let _needed = cursor.u16()?;
        let flags = cursor.u16()?;
        let method = cursor.u16()?;
        let _time = cursor.u16()?;
        let _date = cursor.u16()?;
        let crc32 = cursor.u32()?;
        let mut compressed_size = u64::from(cursor.u32()?);
        let mut uncompressed_size = u64::from(cursor.u32()?);
        let name_len = usize::from(cursor.u16()?);
        let extra_len = usize::from(cursor.u16()?);
        let comment_len = usize::from(cursor.u16()?);
        let disk = cursor.u16()?;
        let _internal = cursor.u16()?;
        let _external = cursor.u32()?;
        let mut local_offset = u64::from(cursor.u32()?);
        let raw_name = cursor.take(name_len)?;
        let extra = cursor.take(extra_len)?;
        cursor.take(comment_len)?;

        if flags & (FLAG_ENCRYPTED | FLAG_STRONG_ENCRYPTION) != 0 {
            return Err(ZipError::Unsupported("encrypted item"));
        }
        let method = match method {
            STORED => Method::Stored,
            DEFLATED => Method::Deflated,
            _ => return Err(ZipError::Unsupported("compression method")),
        };
        // Names must be UTF-8 whether or not the language-encoding flag is
        // set; OPC part names are ASCII after percent-encoding.
        let name = std::str::from_utf8(raw_name)
            .map_err(|_| ZipError::Malformed("item name is not UTF-8"))?
            .to_owned();

        // ZIP64 extra field supplies the values stored as 0xFFFFFFFF.
        let mut extra_cursor = Cursor::new(extra, 0, "extra field");
        while extra_cursor.at + 4 <= extra.len() {
            let id = extra_cursor.u16()?;
            let len = usize::from(extra_cursor.u16()?);
            let data = extra_cursor.take(len)?;
            if id != ZIP64_EXTRA {
                continue;
            }
            let mut zip64 = Cursor::new(data, 0, "ZIP64 extra field");
            if uncompressed_size == u64::from(u32::MAX) {
                uncompressed_size = zip64.u64()?;
            }
            if compressed_size == u64::from(u32::MAX) {
                compressed_size = zip64.u64()?;
            }
            if local_offset == u64::from(u32::MAX) {
                local_offset = zip64.u64()?;
            }
        }
        if disk != 0 && disk != u16::MAX {
            return Err(ZipError::Unsupported("spanned archive"));
        }
        if method == Method::Stored && compressed_size != uncompressed_size {
            return Err(ZipError::Malformed("stored item sizes differ"));
        }

        let local = to_usize(local_offset, "local header")?;
        let mut header = Cursor::new(bytes, local, "local header");
        if header.u32()? != LOCAL_HEADER {
            return Err(ZipError::Malformed("bad local header"));
        }
        header.take(22)?;
        let local_name_len = usize::from(header.u16()?);
        let local_extra_len = usize::from(header.u16()?);
        if header.take(local_name_len)? != raw_name {
            return Err(ZipError::Malformed("local and central names differ"));
        }
        header.take(local_extra_len)?;
        let data_end = header
            .at
            .checked_add(to_usize(compressed_size, "item data")?)
            .filter(|&end| end <= bytes.len())
            .ok_or(ZipError::Truncated("item data"))?;
        Ok(ZipEntry {
            name,
            method,
            crc32,
            compressed_size,
            uncompressed_size,
            // Header-inclusive span, so overlap checks also cover local headers.
            data: local..data_end,
        })
    }

    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// Decompresses one item, never producing more than its declared size,
    /// and verifies the CRC-32.
    pub fn read(&self, index: usize) -> Result<Vec<u8>, ZipError> {
        let entry = &self.entries[index];
        let corrupt = || ZipError::Corrupt {
            name: entry.name.clone(),
        };
        let span = &self.bytes[entry.data.clone()];
        // Skip the local header (fixed 30 bytes plus name and extra).
        let name_len = usize::from(u16::from_le_bytes([span[26], span[27]]));
        let extra_len = usize::from(u16::from_le_bytes([span[28], span[29]]));
        let data = span.get(30 + name_len + extra_len..).ok_or_else(corrupt)?;
        let expected = to_usize(entry.uncompressed_size, "item data")?;
        let output = match entry.method {
            Method::Stored => data.to_vec(),
            Method::Deflated => {
                let mut output = Vec::with_capacity(expected.min(1 << 20));
                DeflateDecoder::new(data)
                    .take(entry.uncompressed_size.saturating_add(1))
                    .read_to_end(&mut output)
                    .map_err(|_| corrupt())?;
                output
            }
        };
        if output.len() != expected || crc32fast::hash(&output) != entry.crc32 {
            return Err(corrupt());
        }
        Ok(output)
    }
}

/// Writes a ZIP archive of the given items in order. Used for saving
/// packages and for building test packages; never emits ZIP64 or encryption.
pub fn write_archive(items: &[(&str, &[u8], Method)]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for &(name, data, method) in items {
        let compressed = match method {
            Method::Stored => data.to_vec(),
            Method::Deflated => {
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data)?;
                encoder.finish()?
            }
        };
        let too_large = |value: usize| u32::try_from(value).map_err(std::io::Error::other);
        let offset = too_large(out.len())?;
        let crc = crc32fast::hash(data);
        let code = match method {
            Method::Stored => STORED,
            Method::Deflated => DEFLATED,
        };
        let name_len = u16::try_from(name.len()).map_err(std::io::Error::other)?;
        let mut header = Vec::new();
        header.extend_from_slice(&20u16.to_le_bytes()); // version needed
        header.extend_from_slice(&FLAG_UTF8.to_le_bytes());
        header.extend_from_slice(&code.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes()); // time
        header.extend_from_slice(&0x21u16.to_le_bytes()); // date 1980-01-01
        header.extend_from_slice(&crc.to_le_bytes());
        header.extend_from_slice(&too_large(compressed.len())?.to_le_bytes());
        header.extend_from_slice(&too_large(data.len())?.to_le_bytes());
        header.extend_from_slice(&name_len.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes()); // extra length

        out.extend_from_slice(&LOCAL_HEADER.to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&compressed);

        central.extend_from_slice(&CENTRAL_HEADER.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // made by
        central.extend_from_slice(&header);
        central.extend_from_slice(&0u16.to_le_bytes()); // comment length
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        central.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(items.len()).map_err(std::io::Error::other)?;
    let cd_offset = u32::try_from(out.len()).map_err(std::io::Error::other)?;
    let cd_size = u32::try_from(central.len()).map_err(std::io::Error::other)?;
    out.extend_from_slice(&central);
    out.extend_from_slice(&END_OF_CENTRAL.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

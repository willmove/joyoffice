//! OPC part names (ECMA-376 Part 2, part naming and equivalence rules; the
//! clause numbers are recorded with the pinned edition in T004).
//!
//! A part name is an absolute path of non-empty segments. Segments may not be
//! `.` or `..` or end with a dot, may contain only URI path characters (or
//! non-ASCII IRI characters, which some producers store unencoded), and may
//! not percent-encode `/` or `\`. Names are equivalent under ASCII case
//! folding, so packages differing only in case are rejected by the reader.

use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartNameError {
    Empty,
    NotAbsolute,
    TrailingSlash,
    EmptySegment,
    DotSegment,
    SegmentEndsWithDot,
    InvalidCharacter(char),
    BadPercentEncoding,
    EncodedSeparator,
}

impl fmt::Display for PartNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartNameError::Empty => f.write_str("empty name"),
            PartNameError::NotAbsolute => f.write_str("does not start with '/'"),
            PartNameError::TrailingSlash => f.write_str("ends with '/'"),
            PartNameError::EmptySegment => f.write_str("empty segment"),
            PartNameError::DotSegment => f.write_str("'.' or '..' segment"),
            PartNameError::SegmentEndsWithDot => f.write_str("segment ends with '.'"),
            PartNameError::InvalidCharacter(ch) => write!(f, "invalid character {ch:?}"),
            PartNameError::BadPercentEncoding => f.write_str("malformed percent-encoding"),
            PartNameError::EncodedSeparator => f.write_str("percent-encoded '/' or '\\'"),
        }
    }
}

/// URI `pchar` minus `%` (handled separately): unreserved, sub-delims, `:`, `@`.
fn path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || "-._~!$&'()*+,;=:@".contains(ch)
        || (!ch.is_ascii() && !ch.is_control() && !ch.is_whitespace())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartName(Arc<str>);

impl PartName {
    pub fn new(name: &str) -> Result<Self, PartNameError> {
        if name.is_empty() {
            return Err(PartNameError::Empty);
        }
        let Some(path) = name.strip_prefix('/') else {
            return Err(PartNameError::NotAbsolute);
        };
        if path.is_empty() || path.ends_with('/') {
            return Err(PartNameError::TrailingSlash);
        }
        for segment in path.split('/') {
            if segment.is_empty() {
                return Err(PartNameError::EmptySegment);
            }
            if segment == "." || segment == ".." {
                return Err(PartNameError::DotSegment);
            }
            if segment.ends_with('.') {
                return Err(PartNameError::SegmentEndsWithDot);
            }
            let mut chars = segment.chars();
            while let Some(ch) = chars.next() {
                if ch == '%' {
                    let hex: String = chars.by_ref().take(2).collect();
                    let byte = (hex.len() == 2)
                        .then(|| u8::from_str_radix(&hex, 16).ok())
                        .flatten()
                        .ok_or(PartNameError::BadPercentEncoding)?;
                    if byte == b'/' || byte == b'\\' {
                        return Err(PartNameError::EncodedSeparator);
                    }
                } else if !path_char(ch) {
                    return Err(PartNameError::InvalidCharacter(ch));
                }
            }
        }
        Ok(Self(name.into()))
    }

    /// Part name of a ZIP item name (`word/document.xml`).
    pub fn from_item_name(item: &str) -> Result<Self, PartNameError> {
        if item.starts_with('/') {
            return Err(PartNameError::EmptySegment);
        }
        Self::new(&format!("/{item}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// ZIP item name, without the leading slash.
    pub fn item_name(&self) -> &str {
        &self.0[1..]
    }

    /// Equivalence key: ASCII case folding.
    pub fn key(&self) -> String {
        self.0.to_ascii_lowercase()
    }

    /// Directory containing the part, with a trailing slash (`/word/`).
    pub fn directory(&self) -> &str {
        let end = self.0.rfind('/').expect("absolute name") + 1;
        &self.0[..end]
    }

    pub fn file_name(&self) -> &str {
        &self.0[self.directory().len()..]
    }

    /// Extension of the last segment, lower-cased for content type lookup.
    pub fn extension(&self) -> Option<String> {
        let name = self.file_name();
        name.rfind('.')
            .map(|dot| name[dot + 1..].to_ascii_lowercase())
    }

    /// The relationships part of this part (`/word/_rels/document.xml.rels`).
    pub fn relationships_part(&self) -> PartName {
        Self(format!("{}_rels/{}.rels", self.directory(), self.file_name()).into())
    }

    /// For a relationships part, its source: `Some(None)` for the package
    /// relationships (`/_rels/.rels`), `Some(Some(part))` for a part's.
    pub fn relationships_source(&self) -> Option<Option<PartName>> {
        let directory = self.directory();
        let parent = directory.strip_suffix("_rels/")?;
        if !(parent.ends_with('/')) {
            return None;
        }
        let file = self.file_name();
        let suffix = file.get(file.len().checked_sub(5)?..)?;
        if !suffix.eq_ignore_ascii_case(".rels") {
            return None;
        }
        let source = &file[..file.len() - 5];
        if source.is_empty() {
            return (parent == "/").then_some(None);
        }
        PartName::new(&format!("{parent}{source}")).ok().map(Some)
    }

    /// Every ancestor directory as a part-name key (`/word`, `/word/media`),
    /// used to reject a part whose name is a prefix path of another.
    pub(crate) fn ancestor_keys(&self) -> impl Iterator<Item = String> + '_ {
        let key = self.key();
        let positions: Vec<usize> = key
            .match_indices('/')
            .map(|(at, _)| at)
            .filter(|&at| at > 0)
            .collect();
        positions.into_iter().map(move |at| key[..at].to_owned())
    }
}

impl fmt::Display for PartName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Resolves a relative reference against a base directory (`/word/`),
/// removing `.` and `..` segments; `None` if it climbs above the root.
pub(crate) fn resolve(base_directory: &str, reference: &str) -> Option<String> {
    let reference = reference.split(['#', '?']).next().unwrap_or_default();
    let joined = if reference.starts_with('/') {
        reference.to_owned()
    } else {
        format!("{base_directory}{reference}")
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/').skip(1) {
        match segment {
            "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(format!("/{}", segments.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_names_and_rejects_unsafe_ones() {
        for valid in [
            "/word/document.xml",
            "/%5Btrash%5D/0000.dat",
            "/word/media/图片1.png",
            "/a/b%20c.xml",
        ] {
            assert!(PartName::new(valid).is_ok(), "{valid}");
        }
        for (invalid, error) in [
            ("", PartNameError::Empty),
            ("word/document.xml", PartNameError::NotAbsolute),
            ("/word/", PartNameError::TrailingSlash),
            ("/word//a.xml", PartNameError::EmptySegment),
            ("/word/../a.xml", PartNameError::DotSegment),
            ("/word/a.", PartNameError::SegmentEndsWithDot),
            ("/word\\a.xml", PartNameError::InvalidCharacter('\\')),
            ("/a b.xml", PartNameError::InvalidCharacter(' ')),
            ("/a%2fb.xml", PartNameError::EncodedSeparator),
            ("/a%5Cb.xml", PartNameError::EncodedSeparator),
            ("/a%zz.xml", PartNameError::BadPercentEncoding),
            ("/a%2", PartNameError::BadPercentEncoding),
        ] {
            assert_eq!(PartName::new(invalid), Err(error), "{invalid:?}");
        }
        assert!(PartName::from_item_name("/abs.xml").is_err());
    }

    #[test]
    fn relationships_parts_map_to_their_sources() {
        let document = PartName::new("/word/document.xml").unwrap();
        let rels = document.relationships_part();
        assert_eq!(rels.as_str(), "/word/_rels/document.xml.rels");
        assert_eq!(rels.relationships_source(), Some(Some(document)));
        let package = PartName::new("/_rels/.rels").unwrap();
        assert_eq!(package.relationships_source(), Some(None));
        assert_eq!(
            PartName::new("/word/_rels/.rels")
                .unwrap()
                .relationships_source(),
            None
        );
        assert_eq!(
            PartName::new("/word/document.xml")
                .unwrap()
                .relationships_source(),
            None
        );
        assert_eq!(
            PartName::new("/_rels/图图").unwrap().relationships_source(),
            None
        );
        assert_eq!(
            PartName::new("/Word/Document.XML").unwrap().key(),
            "/word/document.xml"
        );
        assert_eq!(
            PartName::new("/word/media/a.PNG")
                .unwrap()
                .extension()
                .as_deref(),
            Some("png")
        );
    }

    #[test]
    fn references_resolve_without_escaping_the_root() {
        assert_eq!(
            resolve("/word/", "media/a.png").as_deref(),
            Some("/word/media/a.png")
        );
        assert_eq!(
            resolve("/word/", "../customXml/item1.xml").as_deref(),
            Some("/customXml/item1.xml")
        );
        assert_eq!(
            resolve("/word/", "/word/styles.xml#x").as_deref(),
            Some("/word/styles.xml")
        );
        assert_eq!(
            resolve("/", "word/document.xml").as_deref(),
            Some("/word/document.xml")
        );
        assert_eq!(resolve("/word/", "../../etc/passwd"), None);
    }
}

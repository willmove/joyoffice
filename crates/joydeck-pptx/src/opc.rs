//! Open Packaging Conventions: the zip container, part names, relationships and content types.

use std::collections::{BTreeMap, HashMap};
#[cfg(test)]
use std::io::Write;
use std::io::{Cursor, Read};
use std::sync::Arc;

use crate::PptxError;
pub use crate::package_writer::PackageOut;
pub use crate::relationships::RelsOut;
pub use crate::relationships::{Rel, Rels, dir_of, file_of, relative, rels_path, resolve};
use crate::xml;

/// Largest single part we decompress (media included).
pub const MAX_PART: u64 = 512 * 1024 * 1024;
/// Largest XML part we parse.
pub const MAX_XML_PART: usize = 96 * 1024 * 1024;
/// Total decompressed bytes accepted for one package.
pub const MAX_TOTAL: u64 = 2 * 1024 * 1024 * 1024;
/// Total bytes we are willing to decompress for one package, counting parts we then discard
/// (too large, or failing part-way): bounds inflate work, not just what is kept.
pub const MAX_INFLATE_WORK: u64 = 2 * MAX_TOTAL;
/// Maximum number of zip entries.
pub const MAX_ENTRIES: usize = 50_000;

pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
pub const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
pub const NS_P15: &str = "http://schemas.microsoft.com/office/powerpoint/2012/main";
pub const NS_P159: &str = "http://schemas.microsoft.com/office/powerpoint/2015/09/main";
pub const NS_A14: &str = "http://schemas.microsoft.com/office/drawing/2010/main";
/// Modern (threaded) comments, Office 2018/8 schema.
pub const NS_P188: &str = "http://schemas.microsoft.com/office/powerpoint/2018/8/main";

pub const RT_BASE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
pub const RT_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";
/// A 3-D model part (`.glb`). Office relationship, not an ECMA-376 base type.
pub const RT_MODEL3D: &str = "http://schemas.microsoft.com/office/2017/06/relationships/model3d";
pub const RT_CORE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

/// The files of a package, keyed by normalised part name (no leading slash, lowercase).
pub struct Package {
    parts: BTreeMap<String, (String, Arc<Vec<u8>>)>,
    ct_cache: std::sync::OnceLock<(HashMap<String, String>, HashMap<String, String>)>,
}

impl Package {
    /// Take ownership of a checked member inventory without reopening the ZIP.
    pub(super) fn from_parts(members: BTreeMap<String, Vec<u8>>) -> Result<Self, PptxError> {
        if members.len() > MAX_ENTRIES {
            return Err(PptxError::NotPptx("too many entries".into()));
        }
        let mut remaining = MAX_TOTAL;
        let mut parts = BTreeMap::new();
        for (name, bytes) in members {
            let size = bytes.len() as u64;
            if size > MAX_PART || size > remaining {
                return Err(PptxError::NotPptx(format!(
                    "part {name} exceeds size limits"
                )));
            }
            remaining -= size;
            if parts.insert(norm(&name), (name, Arc::new(bytes))).is_some() {
                return Err(PptxError::NotPptx("ambiguous package part name".into()));
            }
        }
        Ok(Self {
            parts,
            ct_cache: std::sync::OnceLock::new(),
        })
    }

    pub fn open(bytes: &[u8]) -> Result<Self, PptxError> {
        Self::open_within(bytes, MAX_PART, MAX_TOTAL, MAX_INFLATE_WORK)
    }
    /// [`Package::open`] with explicit limits: per part, kept in total, and inflated in total.
    fn open_within(
        bytes: &[u8],
        max_part: u64,
        max_total: u64,
        max_work: u64,
    ) -> Result<Self, PptxError> {
        Self::read_entries(
            bytes,
            max_part,
            max_total,
            max_work,
            &mut Default::default(),
        )
    }

    pub(super) fn open_verified(
        bytes: &[u8],
        mut members: std::collections::BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, PptxError> {
        Self::read_entries(bytes, MAX_PART, MAX_TOTAL, MAX_INFLATE_WORK, &mut members)
    }

    fn read_entries(
        bytes: &[u8],
        max_part: u64,
        max_total: u64,
        max_work: u64,
        members: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, PptxError> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| PptxError::NotPptx(format!("{e}")))?;
        if zip.len() > MAX_ENTRIES {
            return Err(PptxError::NotPptx(format!(
                "too many entries ({})",
                zip.len()
            )));
        }
        let mut parts = BTreeMap::new();
        let mut total: u64 = 0;
        let mut work: u64 = 0;
        for i in 0..zip.len() {
            if work >= max_work {
                log::warn!(
                    "pptx: decompression budget spent; skipping the remaining {} entries",
                    zip.len() - i
                );
                break;
            }
            let mut f = match zip.by_index(i) {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("pptx: skipping unreadable zip entry {i}: {e}");
                    continue;
                }
            };
            if f.is_dir() {
                continue;
            }
            let name = f
                .name()
                .map_err(|e| PptxError::NotPptx(e.to_string()))?
                .trim_start_matches('/')
                .to_string();
            let declared = f.size();
            if declared > max_part {
                log::warn!("pptx: skipping oversized part {name} ({declared} bytes)");
                continue;
            }
            let cap = max_part
                .min(max_total.saturating_sub(total))
                .min(max_work.saturating_sub(work));
            if declared <= cap
                && let Some(data) =
                    joyoffice_opc::take_matching_member(members, &name, declared, f.crc32())
            {
                total += declared;
                work += declared;
                parts.insert(norm(&name), (name.replace('\\', "/"), Arc::new(data)));
                continue;
            }
            let mut data = Vec::with_capacity(declared.min(16 * 1024 * 1024) as usize);
            let read = (&mut f).take(cap.saturating_add(1)).read_to_end(&mut data);
            // Charge what was inflated whether or not the part is kept.
            work = work.saturating_add(data.len() as u64);
            match read {
                Ok(_) if data.len() as u64 > cap => {
                    log::warn!("pptx: part {name} exceeds size limits; skipped");
                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("pptx: could not read {name}: {e}");
                    continue;
                }
            }
            total = total.saturating_add(data.len() as u64);
            // A name kept with `\` would reach code that splits paths on `/`.
            parts.insert(norm(&name), (name.replace('\\', "/"), Arc::new(data)));
        }
        Ok(Package {
            parts,
            ct_cache: std::sync::OnceLock::new(),
        })
    }
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.parts.get(&norm(name)).map(|(_, d)| d.as_slice())
    }
    pub(super) fn shared_member(&self, name: &str) -> Option<Arc<Vec<u8>>> {
        self.parts.get(&norm(name)).map(|(_, bytes)| bytes.clone())
    }

    /// An XML part's bytes, when it is within the XML size limit.
    pub fn xml_bytes(&self, name: &str) -> Option<&[u8]> {
        let b = self.get(name)?;
        if b.len() > MAX_XML_PART {
            log::warn!("pptx: XML part {name} too large");
            return None;
        }
        Some(b)
    }
    /// Parse an XML part.
    pub fn xml(&self, name: &str) -> Option<xml::Doc> {
        let b = self.get(name)?;
        if b.len() > MAX_XML_PART {
            log::warn!("pptx: XML part {name} too large");
            return None;
        }
        match xml::parse(b) {
            Ok(d) => Some(d),
            Err(e) => {
                log::warn!("pptx: {name}: {e}");
                None
            }
        }
    }
    /// Relationships of a part (`ppt/slides/slide1.xml` → `ppt/slides/_rels/slide1.xml.rels`).
    pub fn rels(&self, part: &str) -> Rels {
        let rp = rels_path(part);
        let Some(doc) = self.xml(&rp) else {
            return Rels::default();
        };
        crate::relationships::read(&doc.root, part)
    }
    /// The root relationship, rather than a conventional directory, identifies the main part.
    pub(crate) fn presentation_path(&self) -> Result<String, PptxError> {
        let rels = self.rels("");
        match rels.first_of("officeDocument") {
            Some(rel) if rel.external => Err(PptxError::NotPptx(
                "external presentation relationship is unsupported".into(),
            )),
            Some(rel) => Ok(rel.target.clone()),
            None => Ok("ppt/presentation.xml".into()),
        }
    }
    /// Content types: (extension defaults, part overrides).
    pub fn content_type(&self, part: &str) -> Option<String> {
        let (overrides, defaults) = self.ct_cache.get_or_init(|| {
            let mut ov = HashMap::new();
            let mut def = HashMap::new();
            if let Some(doc) = self.xml("[Content_Types].xml") {
                for o in doc.root.children_named("Override") {
                    if let (Some(n), Some(ct)) = (o.attr("PartName"), o.attr("ContentType")) {
                        ov.insert(n.to_ascii_lowercase(), ct.to_string());
                    }
                }
                for d in doc.root.children_named("Default") {
                    if let (Some(e), Some(ct)) = (d.attr("Extension"), d.attr("ContentType")) {
                        def.insert(e.to_ascii_lowercase(), ct.to_string());
                    }
                }
            }
            (ov, def)
        });
        let p = format!("/{}", norm(part));
        if let Some(ct) = overrides.get(&p) {
            return Some(ct.clone());
        }
        let ext = part.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        defaults.get(&ext).cloned()
    }
    pub fn original_name(&self, name: &str) -> Option<&str> {
        self.parts.get(&norm(name)).map(|(n, _)| n.as_str())
    }

    pub(super) fn members(&self) -> impl Iterator<Item = (&str, &str, &[u8])> {
        self.parts
            .iter()
            .map(|(key, (name, bytes))| (key.as_str(), name.as_str(), bytes.as_slice()))
    }
}

/// The relationships of `part` from the bytes of its rels part (kept parts carried by a chart).
pub fn rels_from_bytes(bytes: &[u8], part: &str) -> Rels {
    match xml::parse(bytes) {
        Ok(doc) => crate::relationships::read(&doc.root, part),
        Err(e) => {
            log::warn!("pptx: relationships of {part}: {e}");
            Rels::default()
        }
    }
}

pub fn norm(name: &str) -> String {
    name.trim_start_matches('/')
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// A part a chart may carry along beside its own `chartN.xml` (workbook, style, drawing, picture).
/// Chart documents themselves are rewritten, and nothing outside these folders is copied: a relationship
/// must not be able to replace `ppt/slides/slide1.xml` on save.
pub fn sidecar_part(name: &str) -> bool {
    let n = norm(name);
    let segs: Vec<&str> = n.split('/').filter(|s| !s.is_empty()).collect();
    if segs.iter().any(|s| *s == "." || *s == "..") {
        return false;
    }
    let (Some(root), Some(area)) = (segs.first().copied(), segs.get(1).copied()) else {
        return false;
    };
    if root != "ppt" || !matches!(area, "embeddings" | "charts" | "drawings" | "media") {
        return false;
    }
    let file = segs.last().copied().unwrap_or("");
    let stem = file.strip_prefix("chart").unwrap_or("");
    let num = stem
        .strip_suffix(".xml.rels")
        .or_else(|| stem.strip_suffix(".xml"))
        .unwrap_or("");
    if stem != file && !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn resolve_keeps_encoded_hash_and_strips_fragment() {
        assert_eq!(
            resolve("ppt/slides", "../media/a%23b.png"),
            "ppt/media/a#b.png"
        );
        assert_eq!(
            resolve("ppt/slides", "../media/a.png#frag"),
            "ppt/media/a.png"
        );
    }

    #[test]
    fn resolve_percent_decode_requires_two_hex_digits() {
        assert_eq!(
            resolve("ppt/slides", "../media/a%+5b.png"),
            "ppt/media/a%+5b.png"
        );
        assert_eq!(
            resolve("ppt/slides", "../media/a%20b.png"),
            "ppt/media/a b.png"
        );
    }

    fn zip_of(entries: &[(&str, usize)]) -> Vec<u8> {
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, len) in entries {
            zw.start_file(*name, o).unwrap();
            zw.write_all(&vec![b'x'; *len]).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn discarded_parts_count_against_the_inflate_budget() {
        // Kept bytes cap at 100: b, c and d are inflated past what's left and discarded. That
        // work is charged, so after d the budget (130) is spent and e is not read, although it
        // would have fit.
        let z = zip_of(&[("a", 80), ("b", 80), ("c", 80), ("d", 80), ("e", 5)]);
        let p = Package::open_within(&z, 1000, 100, 130).unwrap();
        assert!(p.get("a").is_some());
        assert!(p.get("b").is_none() && p.get("d").is_none());
        assert!(p.get("e").is_none(), "the inflate budget stops reading");
        // With room to spare the same small part is kept.
        let p = Package::open_within(&z, 1000, 100, 10_000).unwrap();
        assert!(p.get("e").is_some());
    }

    #[test]
    fn backslash_zip_entry_names_are_found() {
        let z = zip_of(&[("PPT\\Slides\\Slide1.xml", 3)]);
        let p = Package::open(&z).unwrap();
        assert!(p.get("ppt/slides/slide1.xml").is_some());
        assert_eq!(
            p.original_name("ppt/slides/slide1.xml"),
            Some("PPT/Slides/Slide1.xml")
        );
    }

    #[test]
    fn xml_bytes_applies_the_xml_limit() {
        let z = zip_of(&[("small.xml", 10)]);
        let p = Package::open(&z).unwrap();
        assert_eq!(p.xml_bytes("small.xml").map(<[u8]>::len), Some(10));
        assert!(p.xml_bytes("missing.xml").is_none());
    }

    #[test]
    fn resolve_backslash_target() {
        assert_eq!(
            resolve("ppt/slides", "..\\media\\image1.png"),
            "ppt/media/image1.png"
        );
        assert_eq!(
            resolve("ppt/slides", "\\ppt\\media\\a.png"),
            "ppt/media/a.png"
        );
    }

    #[test]
    fn path_resolution() {
        assert_eq!(
            resolve("ppt/slides", "../media/image1.png"),
            "ppt/media/image1.png"
        );
        assert_eq!(resolve("ppt", "slides/slide1.xml"), "ppt/slides/slide1.xml");
        assert_eq!(
            resolve("ppt/slides", "/ppt/media/a%20b.png"),
            "ppt/media/a b.png"
        );
        assert_eq!(resolve("", "../../../x.xml"), "x.xml");
        assert_eq!(
            rels_path("ppt/presentation.xml"),
            "ppt/_rels/presentation.xml.rels"
        );
        assert_eq!(
            rels_path("[Content_Types].xml"),
            "_rels/[Content_Types].xml.rels"
        );
        assert_eq!(
            relative("ppt/slides/slide1.xml", "ppt/media/image1.png"),
            "../media/image1.png"
        );
        assert_eq!(
            relative("ppt/presentation.xml", "ppt/slides/slide1.xml"),
            "slides/slide1.xml"
        );
        assert_eq!(
            relative("ppt/slides/slide1.xml", "ppt/slides/slide2.xml"),
            "slide2.xml"
        );
    }

    #[test]
    fn sidecar_parts_stay_inside_chart_folders() {
        assert!(sidecar_part(
            "ppt/embeddings/Microsoft_Excel_Worksheet1.xlsx"
        ));
        assert!(sidecar_part("ppt/charts/style1.xml"));
        assert!(sidecar_part("ppt/drawings/chartDrawing1.xml"));
        assert!(sidecar_part("ppt/media/image1.png"));
        assert!(!sidecar_part("ppt/charts/chart1.xml"));
        assert!(!sidecar_part("ppt/charts/_rels/chart1.xml.rels"));
        assert!(!sidecar_part("ppt/slides/slide1.xml"));
        assert!(!sidecar_part("ppt/embeddings/../../ppt/slides/slide1.xml"));
        assert!(!sidecar_part("/etc/passwd"));
    }
}

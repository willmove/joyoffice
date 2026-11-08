//! Bounded, read-only document session shared by the desktop and CLI.
use joydeck_model::Presentation;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_INPUT: u64 = 500 << 20;
pub(crate) const MAX_NATIVE_INPUT: u64 = 64 << 20;
const MAX_PPTX_TOTAL: u64 = 2 * MAX_INPUT;
const MAX_PIXELS: u64 = 16_000_000;
pub(crate) static RENDER_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub(crate) fn flag(&self) -> &AtomicBool {
        &self.0
    }
    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self(flag)
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<(), String> {
        if self.0.load(Ordering::Relaxed) {
            Err("operation cancelled".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SlideInfo {
    pub page: usize,
    pub title: String,
    pub shapes: usize,
    pub notes: String,
    pub objects: Vec<serde_json::Value>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DocumentInfo {
    pub name: String,
    pub format: String,
    pub read_only: bool,
    pub source_bytes: usize,
    pub width_pt: f64,
    pub height_pt: f64,
    pub media: usize,
    pub slides: Vec<SlideInfo>,
    pub warnings: Vec<String>,
}
#[derive(Clone)]
pub struct Document {
    pub presentation: Presentation,
    /// Exact original package, including unsupported/unknown parts. R1 never writes it back.
    pub original: Arc<[u8]>,
    pub info: DocumentInfo,
}

impl Document {
    pub fn open(path: &Path, cancel: &Cancellation) -> Result<Self, String> {
        cancel.check()?;
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_INPUT {
            return Err("input exceeds 500 MiB".into());
        }
        let mut bytes = Vec::new();
        let mut input = file.take(MAX_INPUT + 1);
        let mut buffer = [0; 64 * 1024];
        loop {
            cancel.check()?;
            let read = match input.read(&mut buffer) {
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.to_string()),
            };
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        Self::from_bytes(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            bytes,
            cancel,
        )
    }

    pub fn from_bytes(name: String, bytes: Vec<u8>, cancel: &Cancellation) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_INPUT {
            return Err("input exceeds 500 MiB".into());
        }
        cancel.check()?;
        if joydeck_legacy::sniff(&bytes) {
            let converted = joydeck_legacy::convert(&bytes, cancel.flag())
                .map_err(|e| format!("legacy migration refused: {e}"))?;
            let public = Self::from_bytes(name.clone(), converted.pptx.clone(), cancel)?;
            let mut model = public.presentation;
            crate::legacy_semantics::apply(&mut model, &converted)?;
            let mut doc = Self::from_model(name, model)?;
            doc.info.format = "joydeck-v1".into();
            doc.info.warnings.extend(converted.warnings);
            doc.original = bytes.into();
            doc.info.source_bytes = doc.original.len();
            return Ok(doc);
        }
        if crate::native::sniff(&bytes) {
            let native = crate::native::load(&bytes, cancel)?;
            let mut doc = Self::from_model(name, native.model)?;
            if let Some(report) =
                crate::native::parts_cancellable(&bytes, cancel)?.get("migration/report.json")
            {
                let report: serde_json::Value =
                    serde_json::from_slice(report).map_err(|e| e.to_string())?;
                if let Some(warnings) = report["warnings"].as_array() {
                    doc.info.warnings.extend(
                        warnings
                            .iter()
                            .filter_map(|w| w.as_str().map(str::to_owned))
                            .take(100),
                    );
                }
            }
            doc.original = bytes.into();
            doc.info.format = "joydeck".into();
            doc.info.source_bytes = doc.original.len();
            return Ok(doc);
        }
        if joydeck_pptx::is_encrypted(&bytes) {
            return Err("encrypted Office files are not supported; decrypt a copy first".into());
        }
        let pptx = joydeck_pptx::sniff(&bytes);
        let budget = joyoffice_opc::Budget {
            max_entries: 4096,
            max_part_bytes: if pptx { MAX_INPUT } else { 8 << 20 },
            max_total_bytes: if pptx { MAX_PPTX_TOTAL } else { 128 << 20 },
            max_xml_depth: 128,
            max_xml_nodes: 1_000_000,
            ..Default::default()
        };
        let archive = joyoffice_opc::ZipArchive::new(&bytes, &budget)
            .map_err(|e| format!("invalid package: {e:?}"))?;
        let mut names = BTreeSet::new();
        let mut manifest = None;
        let mut has_content_types = false;
        for (i, entry) in archive.entries().iter().enumerate() {
            cancel.check()?;
            let name = &entry.name;
            let path = name.trim_end_matches('/');
            if name.contains(['\\', ':', '\0'])
                || path
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || !names.insert(name.to_lowercase())
            {
                return Err(format!("unsafe or duplicate package path: {name}"));
            }
            let part = archive.read(i).map_err(|e| format!("{name}: {e:?}"))?;
            if name == "presentation.json" {
                manifest = Some(part.clone());
            }
            if name == "[Content_Types].xml" {
                has_content_types = true;
            }
            if name.ends_with(".xml") || name.ends_with(".rels") {
                validate_xml(name, &part, &budget)?;
            }
        }
        // Reject symlink entries rather than passing filesystem semantics to format readers.
        let mut zip =
            zip::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            cancel.check()?;
            if zip
                .by_index(i)
                .map_err(|e| e.to_string())?
                .unix_mode()
                .is_some_and(|m| m & 0o170000 == 0o120000)
            {
                return Err("symlink package entries are unsupported".into());
            }
        }
        let (format, p, mut warnings) = if let Some(json) = manifest {
            let value: serde_json::Value = serde_json::from_slice(&json)
                .map_err(|e| format!("invalid native manifest: {e}"))?;
            if value["format"] != "deckcraft" || value["version"].as_u64() != Some(1) {
                return Err(
                    "unsupported .deckcraft format/version (only version 1 is supported)".into(),
                );
            }
            let (p, w) = joydeck_format::load_checked(&bytes).map_err(|e| e.to_string())?;
            ("deckcraft", p, w)
        } else if has_content_types {
            let (p, w) = joydeck_pptx::import_checked(&bytes).map_err(|e| e.to_string())?;
            ("pptx", p, w)
        } else {
            return Err("unsupported package: expected PPTX or .deckcraft".into());
        };
        cancel.check()?;
        if !p.slide_size.width.is_finite()
            || !p.slide_size.height.is_finite()
            || !(1.0..=4096.0).contains(&p.slide_size.width)
            || !(1.0..=4096.0).contains(&p.slide_size.height)
            || p.slides.is_empty()
            || p.slides.len() > 1000
        {
            return Err("invalid slide dimensions/count (1–4096 pt; 1–1000 pages)".into());
        }
        warnings.push("Import/render compatibility: unsupported features may be approximated. Original package bytes are retained; preserving writeback validates the editable subset. Linked media is not fetched; fonts may be substituted.".into());
        audit_model(&p, &mut warnings, cancel)?;
        let slides = p
            .slides
            .iter()
            .enumerate()
            .map(|(i, s)| SlideInfo {
                page: i + 1,
                title: {
                    let t = s.title();
                    if !t.is_empty() {
                        t
                    } else if !s.name.is_empty() {
                        s.name.clone()
                    } else {
                        s.shapes
                            .iter()
                            .find_map(|shape| {
                                shape
                                    .text
                                    .as_ref()
                                    .map(|text| text.text())
                                    .filter(|text| !text.trim().is_empty())
                            })
                            .unwrap_or_default()
                            .chars()
                            .take(80)
                            .collect()
                    }
                },
                shapes: s.shapes.len(),
                notes: s.notes.text(),
                objects: objects(&p, i),
            })
            .collect();
        let info = DocumentInfo {
            name,
            format: format.into(),
            read_only: true,
            source_bytes: bytes.len(),
            width_pt: p.slide_size.width,
            height_pt: p.slide_size.height,
            media: p.media.len(),
            slides,
            warnings,
        };
        Ok(Self {
            presentation: p,
            original: bytes.into(),
            info,
        })
    }

    pub fn render(
        &self,
        page: usize,
        scale: f64,
        cancel: &Cancellation,
    ) -> Result<joydeck_render::Image, String> {
        self.render_output(page, scale, false, cancel)
    }

    pub fn render_output(
        &self,
        page: usize,
        scale: f64,
        transparent: bool,
        cancel: &Cancellation,
    ) -> Result<joydeck_render::Image, String> {
        cancel.check()?;
        if page >= self.presentation.slides.len() {
            return Err("page outside document".into());
        }
        let w = (self.info.width_pt * scale).ceil();
        let h = (self.info.height_pt * scale).ceil();
        if !scale.is_finite()
            || !(0.05..=8.0).contains(&scale)
            || w > 8192.
            || h > 8192.
            || w * h > MAX_PIXELS as f64
        {
            return Err("render exceeds scale/pixel budget (0.05–8; 8192 side; 16 MP)".into());
        }
        // Upstream reports caught renderer panics globally. Serialize to associate them with a job.
        let _guard = RENDER_LOCK
            .lock()
            .map_err(|_| "renderer unavailable after failure")?;
        cancel.check()?;
        let panics = joydeck_render::panics_caught();
        let result = joydeck_render::render_slide(
            &self.presentation,
            page,
            &joydeck_render::RenderOpts {
                scale,
                threads: 0,
                transparent_background: transparent,
                ..Default::default()
            },
        );
        cancel.check()?;
        if joydeck_render::panics_caught() != panics || result.pixels.is_empty() {
            return Err("renderer failed; no preview produced".into());
        }
        Ok(result)
    }
}

impl Document {
    pub fn from_model(name: String, presentation: Presentation) -> Result<Self, String> {
        let mut warnings = vec![];
        audit_model(&presentation, &mut warnings, &Cancellation::default())?;
        let info = DocumentInfo {
            name,
            format: "joydeck".into(),
            read_only: false,
            source_bytes: 0,
            width_pt: presentation.slide_size.width,
            height_pt: presentation.slide_size.height,
            media: presentation.media.len(),
            slides: presentation
                .slides
                .iter()
                .enumerate()
                .map(|(i, s)| SlideInfo {
                    page: i + 1,
                    title: {
                        let t = s.title();
                        if !t.is_empty() {
                            t
                        } else if !s.name.is_empty() {
                            s.name.clone()
                        } else {
                            s.shapes
                                .iter()
                                .find_map(|shape| {
                                    shape
                                        .text
                                        .as_ref()
                                        .map(|t| t.text())
                                        .filter(|t| !t.is_empty())
                                })
                                .unwrap_or_default()
                                .chars()
                                .take(80)
                                .collect()
                        }
                    },
                    shapes: s.shapes.len(),
                    notes: s.notes.text(),
                    objects: objects(&presentation, i),
                })
                .collect(),
            warnings,
        };
        Ok(Self {
            presentation,
            original: Arc::from([]),
            info,
        })
    }
}

fn objects(p: &Presentation, page: usize) -> Vec<serde_json::Value> {
    let Some(s) = p.slides.get(page) else {
        return vec![];
    };
    let Some(ctx) = joydeck_model::resolve::Ctx::for_slide(p, s) else {
        return vec![];
    };
    s.shapes.iter().map(|shape|serde_json::json!({"id":shape.id.0,"name":shape.name,"kind":shape.kind_name(),"text":shape.text.as_ref().map(|t|t.text()),"transform":joydeck_model::resolve::xfrm(&ctx,shape),"locked":shape.locked,"text_body":shape.text,"table":if let joydeck_model::ShapeKind::Table(t)=&shape.kind {Some(t)}else{None}})).collect()
}

fn audit_model(
    p: &Presentation,
    warnings: &mut Vec<String>,
    cancel: &Cancellation,
) -> Result<(), String> {
    let mut requested = BTreeSet::new();
    let mut shapes = Vec::new();
    for slide in &p.slides {
        shapes.extend(slide.shapes.iter().map(|s| (s, 0)));
    }
    for master in &p.masters {
        for set in [&master.theme.fonts.major, &master.theme.fonts.minor] {
            for family in [&set.latin, &set.ea, &set.cs] {
                requested.insert((family.clone(), "Regular"));
            }
        }
        shapes.extend(master.shapes.iter().map(|s| (s, 0)));
        for layout in &master.layouts {
            shapes.extend(layout.shapes.iter().map(|s| (s, 0)));
        }
    }
    let mut count = 0;
    while let Some((shape, depth)) = shapes.pop() {
        cancel.check()?;
        count += 1;
        if count > 20_000 || depth > 64 {
            return Err("document shape/depth budget exceeded".into());
        }
        if let joydeck_model::ShapeKind::Group { children, .. } = &shape.kind {
            shapes.extend(children.iter().map(|s| (s, depth + 1)));
        }
        if let Some(text) = &shape.text {
            for para in &text.paragraphs {
                for props in std::iter::once(&para.def_props)
                    .chain(std::iter::once(&para.end_props))
                    .chain(para.runs.iter().map(|r| &r.props))
                {
                    for family in [&props.font, &props.font_ea, &props.font_cs, &props.font_sym]
                        .into_iter()
                        .flatten()
                    {
                        let style =
                            match (props.bold.unwrap_or(false), props.italic.unwrap_or(false)) {
                                (true, true) => "Bold Italic",
                                (true, false) => "Bold",
                                (false, true) => "Italic",
                                (false, false) => "Regular",
                            };
                        requested.insert((family.clone(), style));
                    }
                }
            }
        }
        if matches!(
            shape.kind,
            joydeck_model::ShapeKind::Opaque { .. } | joydeck_model::ShapeKind::Model3d { .. }
        ) && warnings.len() < 200
        {
            warnings.push(format!(
                "Object {} uses a fallback/unsupported representation: {}",
                shape.id, shape.name
            ));
        }
    }
    let fonts = joydeck_fonts::FontDb::global();
    for (family, style) in requested
        .iter()
        .filter(|(family, _)| !family.is_empty() && !family.starts_with('+'))
        .take(128)
    {
        cancel.check()?;
        let resolved = fonts.document_match(family, style);
        if resolved.substituted {
            warnings.push(format!(
                "Font substitution: {family} ({style}) → {} ({}) [{}].",
                resolved.face.family,
                resolved.face.style,
                resolved.face.identity()
            ));
        }
        if !resolved.conflicts.is_empty() {
            let alternatives = resolved
                .conflicts
                .iter()
                .take(4)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            warnings.push(format!(
                "Font name collision: {} ({}). Selected [{}]; {} distinct alternative(s) [{}].",
                resolved.face.family,
                resolved.face.style,
                resolved.face.identity(),
                resolved.conflicts.len(),
                alternatives
            ));
        }
    }
    Ok(())
}

fn validate_xml(name: &str, bytes: &[u8], budget: &joyoffice_opc::Budget) -> Result<(), String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut depth = 0usize;
    for _ in 0..budget.max_xml_nodes {
        match reader
            .read_event()
            .map_err(|e| format!("{name}: invalid XML: {e}"))?
        {
            Event::Eof => {
                return if depth == 0 {
                    Ok(())
                } else {
                    Err(format!("{name}: unclosed XML element"))
                };
            }
            Event::Start(_) => {
                depth += 1;
                if depth > budget.max_xml_depth {
                    return Err(format!("{name}: XML depth budget exceeded"));
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::DocType(_) => return Err(format!("{name}: DTDs are unsupported")),
            _ => {}
        }
    }
    Err(format!("{name}: XML node budget exceeded"))
}

/// A new output directory is required. Clean up only files created by this call on failure.
pub fn render_to_directory(
    doc: &Document,
    directory: &Path,
    pages: &[usize],
    scale: f64,
    cancel: &Cancellation,
) -> Result<(), String> {
    if pages.is_empty() {
        return Err("no pages requested".into());
    }
    std::fs::create_dir(directory).map_err(|e| format!("output directory must be new: {e}"))?;
    let result = (|| {
        for &page in pages {
            let image = doc.render(page, scale, cancel)?;
            let png = image.to_png();
            if png.is_empty() {
                return Err("PNG encoding failed".into());
            }
            let mut file =
                std::fs::File::create_new(directory.join(format!("slide-{:04}.png", page + 1)))
                    .map_err(|e| e.to_string())?;
            file.write_all(&png).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(&doc.info).map_err(|e| e.to_string())?;
        std::fs::File::create_new(directory.join("inspect.json"))
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(|e| e.to_string())
    })();
    // Partial results remain for diagnosis; no existing directory or file is overwritten.
    result
}

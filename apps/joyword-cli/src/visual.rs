//! Deterministic visual exports from the same immutable PageScene used by the
//! app and PDF consumers. No system font lookup or alternate pagination occurs.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use joyoffice_opc::Budget;
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document,
};
use joyword_render::{Projection, RasterCancellation, RasterLimits, rasterize};
use serde_json::{Value, json};

const MAX_INPUT: u64 = 32 << 20;
const MAX_FONT: u64 = 128 << 20;
const MAX_PAGES: usize = 64;
const MAX_OUTPUT: usize = 256 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Render,
    Convert,
}

pub struct Options {
    command: Command,
    input: OsString,
    font: OsString,
    output: OsString,
    dpi: u16,
    page: Option<usize>,
}

pub fn options(
    command: Command,
    mut args: impl Iterator<Item = OsString>,
) -> Result<Option<Options>, &'static str> {
    let mut input = None;
    let mut font = None;
    let mut output = None;
    let mut to = None;
    let mut dpi = 144;
    let mut dpi_seen = false;
    let mut page = None;
    let mut positional_only = false;
    while let Some(arg) = args.next() {
        if !positional_only && arg == "--" {
            positional_only = true;
            continue;
        }
        if !positional_only && (arg == "--help" || arg == "-h") {
            return if input.is_none()
                && font.is_none()
                && output.is_none()
                && to.is_none()
                && !dpi_seen
                && page.is_none()
                && args.next().is_none()
            {
                Ok(None)
            } else {
                Err("unexpected arguments")
            };
        }
        if !positional_only && arg == "--font" {
            if font.is_some() {
                return Err("duplicate font");
            }
            font = Some(args.next().ok_or("missing font value")?);
            continue;
        }
        if !positional_only && arg == "--output" {
            if output.is_some() {
                return Err("duplicate output");
            }
            output = Some(args.next().ok_or("missing output value")?);
            continue;
        }
        if !positional_only && arg == "--to" {
            if to.is_some() {
                return Err("duplicate conversion target");
            }
            to = Some(args.next().ok_or("missing conversion target")?);
            continue;
        }
        if !positional_only && arg == "--dpi" {
            if dpi_seen {
                return Err("duplicate DPI");
            }
            dpi_seen = true;
            dpi = args
                .next()
                .and_then(|value| value.to_str().and_then(|text| text.parse().ok()))
                .filter(|value| (1..=1200).contains(value))
                .ok_or("invalid DPI (1..1200)")?;
            continue;
        }
        if !positional_only && arg == "--page" {
            if page.is_some() {
                return Err("duplicate page");
            }
            page = Some(
                args.next()
                    .and_then(|value| value.to_str().and_then(|text| text.parse::<usize>().ok()))
                    .filter(|value| *value >= 1)
                    .ok_or("invalid page (one-based)")?,
            );
            continue;
        }
        if !positional_only && arg.as_encoded_bytes().starts_with(b"-") {
            return Err("unknown option; use -- before a path starting with '-'");
        }
        if input.replace(arg).is_some() {
            return Err("unexpected arguments");
        }
    }
    let input = input.ok_or("missing DOCX path")?;
    let font = font.ok_or("missing --font PATH:FACE")?;
    let output = output.ok_or("missing --output")?;
    match command {
        Command::Render => {
            if to.is_some() {
                return Err("render does not accept --to");
            }
        }
        Command::Convert => {
            if page.is_some() {
                return Err("convert writes all pages and does not accept --page");
            }
            if to.as_deref() != Some(std::ffi::OsStr::new("pdf")) {
                return Err("only --to pdf is supported");
            }
        }
    }
    Ok(Some(Options {
        command,
        input,
        font,
        output,
        dpi,
        page,
    }))
}

fn import_docx(path: &OsString) -> Result<joyword_docx::ImportResult, &'static str> {
    if !fs::metadata(path)
        .map_err(|_| "cannot inspect input")?
        .is_file()
    {
        return Err("input must be a regular file");
    }
    let file = File::open(path).map_err(|_| "cannot open input")?;
    if !file
        .metadata()
        .map_err(|_| "cannot inspect input")?
        .is_file()
    {
        return Err("input must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read input")?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err("input exceeds 32 MiB budget");
    }
    let budget = Budget {
        max_entries: 1024,
        max_part_bytes: 16 << 20,
        max_total_bytes: 64 << 20,
        max_xml_depth: 128,
        max_xml_nodes: 1_000_000,
        max_relationships: 4096,
        ..Budget::default()
    };
    joyword_docx::import(&bytes, &budget)
        .map_err(|_| "DOCX refused: invalid, unsupported or over budget")
}

fn load_font(spec: &OsString) -> Result<LoadedFont, &'static str> {
    let spec = spec
        .to_str()
        .ok_or("font specification is not valid Unicode")?;
    let (path, face) = spec.rsplit_once(':').ok_or("font must be PATH:FACE")?;
    let face = face.parse().map_err(|_| "invalid font face")?;
    let file = File::open(path).map_err(|_| "cannot open font")?;
    let mut bytes = Vec::new();
    file.take(MAX_FONT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read font")?;
    if bytes.len() as u64 > MAX_FONT {
        return Err("font exceeds 128 MiB budget");
    }
    LoadedFont::from_bytes(bytes.into(), face).map_err(|_| "invalid or unsupported font face")
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn load_layout(options: &Options) -> Result<(LoadedFont, LayoutSnapshot), &'static str> {
    let font = load_font(&options.font)?;
    let imported = import_docx(&options.input)?;
    let profile = profile();
    let layout = layout_document(
        &imported.document.snapshot(),
        FaceSet::uniform(&font),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .map_err(|_| "document layout refused: unsupported content or budget")?;
    layout
        .ensure_current(&imported.document.snapshot(), &font, profile)
        .map_err(|_| "layout snapshot changed unexpectedly")?;
    if !layout.diagnostics().is_empty() || layout.omitted_diagnostics() != 0 {
        return Err("font cannot display the complete document");
    }
    if layout.pages().is_empty() || layout.pages().len() > MAX_PAGES {
        return Err("visual page budget exceeded");
    }
    Ok((font, layout))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    let name = path.file_name().ok_or("invalid output path")?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|_| "cannot inspect output directory")?;
    let target = parent.join(name);
    if fs::symlink_metadata(&target).is_ok() {
        return Err("output already exists");
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-cli-")
        .tempfile_in(&parent)
        .map_err(|_| "cannot create output temporary file")?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| "cannot write or sync output")?;
    let mut check = temporary
        .as_file()
        .try_clone()
        .map_err(|_| "cannot reopen output temporary file")?;
    std::io::Seek::seek(&mut check, std::io::SeekFrom::Start(0))
        .map_err(|_| "cannot rewind output temporary file")?;
    let mut actual = Vec::new();
    check
        .take(bytes.len() as u64 + 1)
        .read_to_end(&mut actual)
        .map_err(|_| "cannot read output temporary file")?;
    if actual != bytes {
        return Err("output temporary verification failed");
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err("output already exists");
    }
    fs::hard_link(temporary.path(), &target).map_err(|_| "cannot publish output")?;
    temporary
        .close()
        .map_err(|_| "output published but temporary cleanup failed")?;
    let published = fs::read(&target).map_err(|_| "output published but read-back failed")?;
    if published != bytes {
        return Err("output published but verification failed");
    }
    Ok(())
}

fn write_report(report: Value) -> Result<u8, &'static str> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &report).map_err(|_| "cannot write JSON")?;
    stdout.write_all(b"\n").map_err(|_| "cannot write stdout")?;
    stdout.flush().map_err(|_| "cannot flush stdout")?;
    Ok(0)
}

fn render(options: &Options, font: LoadedFont, layout: LayoutSnapshot) -> Result<u8, &'static str> {
    let output = PathBuf::from(&options.output);
    if output.exists() && !output.is_dir() {
        return Err("render output must be a directory");
    }
    fs::create_dir_all(&output).map_err(|_| "cannot create render output directory")?;
    let projection =
        Projection::new(f64::from(options.dpi), 1.0).map_err(|_| "invalid raster projection")?;
    let selected: Vec<usize> = match options.page {
        Some(page) if page <= layout.pages().len() => vec![page - 1],
        Some(_) => return Err("page is outside the document"),
        None => (0..layout.pages().len()).collect(),
    };
    let mut artifacts = Vec::new();
    let mut total = 0usize;
    for index in selected {
        let scene = layout
            .pages()
            .get(index)
            .ok_or("page is outside the document")?;
        let raster = rasterize(
            scene,
            std::slice::from_ref(&font),
            projection,
            RasterLimits::default(),
            &RasterCancellation::default(),
        )
        .map_err(|_| "page raster refused: invalid content or budget")?;
        let png = raster.png().map_err(|_| "cannot encode PNG")?;
        total = total
            .checked_add(png.len())
            .ok_or("visual output budget exceeded")?;
        if total > MAX_OUTPUT {
            return Err("visual output budget exceeded");
        }
        artifacts.push((index, png, raster.width(), raster.height()));
    }
    let mut files = Vec::new();
    for (index, png, width, height) in artifacts {
        let name = format!("page-{}.png", index + 1);
        write_new(&output.join(&name), &png)?;
        files.push(json!({
            "name": name,
            "bytes": png.len(),
            "width": width,
            "height": height,
        }));
    }
    write_report(json!({
        "schema_version": 1,
        "operation": "render",
        "dpi": options.dpi,
        "pages": files.len(),
        "omitted_blocks": layout.omitted_blocks(),
        "font_sha256": font.key().hash_hex(),
        "font_face": font.key().index(),
        "files": files,
    }))
}

fn convert(
    options: &Options,
    font: LoadedFont,
    layout: LayoutSnapshot,
) -> Result<u8, &'static str> {
    let output = Path::new(&options.output);
    if !output
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        return Err("PDF output path must end in .pdf");
    }
    let pdf = joyword_pdf::render(layout.pages(), std::slice::from_ref(&font))
        .map_err(|_| "PDF writer refused the page scene")?;
    if pdf.len() > MAX_OUTPUT {
        return Err("visual output budget exceeded");
    }
    write_new(Path::new(&options.output), &pdf)?;
    write_report(json!({
        "schema_version": 1,
        "operation": "convert",
        "target": "pdf",
        "pages": layout.pages().len(),
        "bytes": pdf.len(),
        "omitted_blocks": layout.omitted_blocks(),
        "font_sha256": font.key().hash_hex(),
        "font_face": font.key().index(),
    }))
}

pub fn execute(options: Options) -> Result<u8, &'static str> {
    let (font, layout) = load_layout(&options)?;
    match options.command {
        Command::Render => render(&options, font, layout),
        Command::Convert => convert(&options, font, layout),
    }
}

//! Legacy option syntax and schema 1 reports over R8 visual exports.

use joyword_shell::{
    reader::{publish_new, publish_rasters},
    visual::FixedVisual,
};
use serde_json::json;
use std::{ffi::OsString, path::Path};

const MAX_OUTPUT: usize = 256 << 20;
const RENDER: &str = "render FILE --font PATH:FACE --output DIR [--dpi N] [--page N]";
const CONVERT: &str =
    "convert FILE --to pdf|pdfa-2b --font PATH:FACE --output FILE.pdf [--archive-year YYYY]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Render,
    Convert,
}

struct Options {
    command: Command,
    input: OsString,
    font: OsString,
    output: OsString,
    dpi: u16,
    page: Option<usize>,
    archival: bool,
    archive_year: Option<u16>,
}

fn options(
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
    let mut archive_year = None;
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
                && archive_year.is_none()
                && args.next().is_none()
            {
                Ok(None)
            } else {
                Err("unexpected arguments")
            };
        }
        if !positional_only && arg == "--archive-year" {
            if archive_year.is_some() {
                return Err("duplicate archive year");
            }
            archive_year = Some(
                args.next()
                    .and_then(|v| v.to_str().and_then(|v| v.parse::<u16>().ok()))
                    .filter(|v| (1..=9999).contains(v))
                    .ok_or("invalid archive year")?,
            );
            continue;
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
            if !matches!(
                to.as_ref().and_then(|s| s.to_str()),
                Some("pdf" | "pdfa-2b")
            ) {
                return Err("only --to pdf or pdfa-2b is supported");
            }
        }
    }
    if to.as_ref().is_some_and(|s| s == "pdfa-2b") != archive_year.is_some() {
        return Err("--archive-year required only for pdfa-2b");
    }
    Ok(Some(Options {
        command,
        input,
        font,
        output,
        dpi,
        page,
        archive_year,
        archival: to.as_ref().is_some_and(|s| s == "pdfa-2b"),
    }))
}

pub fn is_legacy(args: &[OsString]) -> bool {
    args.len() < 2
        || args.iter().take_while(|arg| *arg != "--").any(|arg| {
            matches!(
                arg.to_str(),
                Some("--font" | "--output" | "--to" | "--archive-year" | "--dpi" | "--help" | "-h")
            )
        })
        || args.iter().any(|arg| arg == "--")
}

pub fn run(command: Command, args: &[OsString]) -> Result<u8, String> {
    let Some(options) = options(command, args.iter().cloned())? else {
        println!(
            "{}",
            if command == Command::Render {
                RENDER
            } else {
                CONVERT
            }
        );
        return Ok(0);
    };
    let output = Path::new(&options.output);
    if options.command == Command::Convert
        && !output
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
    {
        return Err("PDF output path must end in .pdf".into());
    }
    let document = FixedVisual::open(
        Path::new(&options.input),
        options
            .font
            .to_str()
            .ok_or("font specification is not valid Unicode")?,
    )?;
    let mut report = json!({
        "schema_version": 1,
        "font_sha256": document.font_hash(),
        "font_face": document.font_index(),
        "omitted_blocks": 0,
    });
    if options.command == Command::Render {
        let pages = match options.page {
            Some(page) if page <= document.page_count() => page - 1..page,
            Some(_) => return Err("page is outside the document".into()),
            None => 0..document.page_count(),
        };
        let mut artifacts = Vec::new();
        let mut files = Vec::new();
        let mut total = 0;
        for page in pages {
            let name = format!("page-{}.png", page + 1);
            let destination = output.join(&name);
            let raster = document.raster(page, options.dpi)?;
            let png = raster.to_png();
            if png.is_empty() {
                return Err("PNG encoding failed".into());
            }
            total += png.len();
            if total > MAX_OUTPUT {
                return Err("visual output budget exceeded".into());
            }
            files.push(json!({
                "name": name, "bytes": png.len(),
                "width": raster.width, "height": raster.height,
            }));
            artifacts.push((destination, png));
        }
        publish_rasters(output, &artifacts)?;
        report["operation"] = json!("render");
        report["dpi"] = json!(options.dpi);
        report["pages"] = json!(files.len());
        report["files"] = json!(files);
    } else {
        let pdf = document.pdf(options.archive_year)?;
        if pdf.len() > MAX_OUTPUT {
            return Err("visual output budget exceeded".into());
        }
        publish_new(output, &pdf)?;
        report["operation"] = json!("convert");
        report["target"] = json!(if options.archival { "pdfa-2b" } else { "pdf" });
        report["independent_conformance_verified"] = json!(false);
        report["pages"] = json!(document.page_count());
        report["bytes"] = json!(pdf.len());
    }
    super::report(report)?;
    Ok(0)
}

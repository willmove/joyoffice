//! PDF bytes from the page scene the screen already installed.
//!
//! Export and print slice that scene. They do not paginate again.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use joyword_layout::{LayoutSnapshot, LoadedFont};
use joyword_model::{Document, DocumentSnapshot};
use joyword_pdf::{PdfExport, core_title, outline_from_headings, render_export};
use joyword_shell::print::{self, PrintReceipt};

pub(crate) enum PdfKind {
    Export,
    Print { copies: u16, range: String },
}

pub(crate) struct PdfJob {
    pub destination: PathBuf,
    pub layout: Arc<LayoutSnapshot>,
    pub font: Arc<LoadedFont>,
    pub document: DocumentSnapshot,
    pub core: Option<Arc<[u8]>>,
    pub pages: Range<usize>,
    pub kind: PdfKind,
}

pub(crate) fn scene_pdf(
    layout: &LayoutSnapshot,
    font: &LoadedFont,
    document: &Document,
    core_xml: Option<&[u8]>,
    pages: Range<usize>,
) -> Result<Vec<u8>, String> {
    if pages.start >= pages.end || pages.end > layout.pages().len() {
        return Err("range".into());
    }
    let mut outline = outline_from_headings(document, layout);
    let start = pages.start;
    if pages != (0..layout.pages().len()) {
        outline.retain_mut(|entry| {
            if (start..pages.end).contains(&entry.page) {
                entry.page -= start;
                true
            } else {
                false
            }
        });
    }
    let export = PdfExport {
        title: core_xml.and_then(core_title),
        outline,
    };
    let selected = layout.pages()[pages].to_vec();
    render_export(&selected, std::slice::from_ref(font), &export).map_err(|error| match error {
        joyword_pdf::PdfError::MissingFont(_) => "missing font".into(),
        joyword_pdf::PdfError::Writer(_) => "pdf writer".into(),
        joyword_pdf::PdfError::InvalidPage => "invalid page".into(),
        joyword_pdf::PdfError::Image(_) => "image".into(),
    })
}

pub(crate) fn run_pdf(job: PdfJob) -> Result<PrintReceipt, String> {
    let bytes = scene_pdf(
        &job.layout,
        job.font.as_ref(),
        &job.document,
        job.core.as_deref(),
        job.pages,
    )?;
    match job.kind {
        PdfKind::Export => {
            print::publish_pdf(&job.destination, &bytes).map_err(|error| error.to_string())
        }
        PdfKind::Print { copies, range } => print::print_to_file(print::PrintToFile {
            destination: &job.destination,
            pdf: &bytes,
            copies,
            range: &range,
        })
        .map_err(|error| error.to_string()),
    }
}

//! PDF bytes from the page scene the screen already installed.
//!
//! Export and print slice that scene. They do not paginate again.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;

use joyword_layout::{LayoutSnapshot, LoadedFont, SingleFontProfile};
use joyword_model::{Document, DocumentSnapshot};
use joyword_pdf::{
    PdfExport, core_title, links_from_document, navigation_from_document, outline_from_headings,
    render_export_with_navigation,
};
use joyword_shell::print::{self, PrintReceipt};

pub(crate) enum PdfKind {
    Export,
    Print { copies: u16, range: String },
}

pub(crate) struct PdfJob {
    pub destination: PathBuf,
    pub layout: Arc<LayoutSnapshot>,
    pub font: Arc<LoadedFont>,
    pub profile: SingleFontProfile,
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
    let mut navigation =
        navigation_from_document(document, layout).map_err(|_| "navigation".to_string())?;
    let mut outline = outline_from_headings(document, layout);
    let mut links = links_from_document(document, layout);
    let start = pages.start;
    if pages != (0..layout.pages().len()) {
        navigation.bookmarks.retain_mut(|bookmark| {
            if pages.contains(&bookmark.destination.page) {
                bookmark.destination.page -= start;
                true
            } else {
                false
            }
        });
        navigation.links.retain_mut(|link| {
            if pages.contains(&link.page) && pages.contains(&link.destination.page) {
                link.page -= start;
                link.destination.page -= start;
                true
            } else {
                false
            }
        });
        outline.retain_mut(|entry| {
            if (start..pages.end).contains(&entry.page) {
                entry.page -= start;
                true
            } else {
                false
            }
        });
        links.retain_mut(|link| {
            if (start..pages.end).contains(&link.page) {
                link.page -= start;
                true
            } else {
                false
            }
        });
    }
    let export = PdfExport {
        title: core_xml.and_then(core_title),
        outline,
        links,
        ..Default::default()
    };
    let selected = layout.pages()[pages].to_vec();
    render_export_with_navigation(&selected, std::slice::from_ref(font), &export, &navigation)
        .map_err(|error| match error {
            joyword_pdf::PdfError::MissingFont(_) => "missing font".into(),
            joyword_pdf::PdfError::UnembeddableFont(_) => "unembeddable font".into(),
            joyword_pdf::PdfError::Writer(_) => "pdf writer".into(),
            joyword_pdf::PdfError::InvalidPage => "invalid page".into(),
            joyword_pdf::PdfError::Image(_) => "image".into(),
            joyword_pdf::PdfError::Navigation(_) => "navigation".into(),
        })
}

/// One publish for export and print-to-file. The layout must still be the
/// settled snapshot for the handed document: an unsettled or stale layout
/// (layout reports "布局未稳定" as [`joyword_layout::LayoutError::Unstable`])
/// refuses before any byte is written, so no partial PDF is ever published.
pub(crate) fn run_pdf(job: PdfJob) -> Result<PrintReceipt, String> {
    job.layout
        .ensure_current(&job.document, &job.font, job.profile)
        .map_err(|_| "layout unstable".to_string())?;
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

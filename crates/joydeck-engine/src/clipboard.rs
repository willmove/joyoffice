//! Document-local link references on data received from the system clipboard.
use crate::{
    Command, media_refs,
    transfer::{PASTE_OFFSET, Transfer},
};
use joydeck_model::{
    Presentation, Shape, ShapeKind, TextBody, Xfrm,
    text::{Action, Hyperlink, RunProps, safe_url},
};
use std::collections::BTreeSet;

/// Insert an external formatted fragment as a new canvas text box in one transaction.
pub fn text_box(mut body: TextBody) -> Command {
    for paragraph in &mut body.paragraphs {
        paragraph
            .def_props
            .font
            .get_or_insert_with(|| "Arial".into());
        paragraph.def_props.size.get_or_insert(24.);
    }
    Command::PasteObjects {
        transfer: Box::new(Transfer {
            joydeck_objects: 1,
            shapes: vec![Shape {
                name: "Text".into(),
                xfrm: Some(Xfrm::new(
                    80. - PASTE_OFFSET,
                    100. - PASTE_OFFSET,
                    400.,
                    80.,
                )),
                text: Some(body),
                text_box: true,
                ..Default::default()
            }],
            media: vec![],
        }),
    }
}

struct LinkScope<'a> {
    document: Option<&'a Presentation>,
    transfer_media: bool,
    adjusted: bool,
}

impl LinkScope<'_> {
    fn link(&mut self, slot: &mut Option<Hyperlink>) {
        let Some(link) = slot else { return };
        let valid = match &link.action {
            Action::Slide { slide } => self.document.is_some_and(|doc| doc.slide(*slide).is_some()),
            Action::CustomShow { name, .. } => self
                .document
                .is_some_and(|doc| doc.custom_shows.iter().any(|show| show.name == *name)),
            Action::PlayMedia => self.document.is_some(),
            Action::Url { url } => safe_url(url) && !url.chars().any(char::is_control),
            Action::Program { .. } => false,
            Action::NextSlide
            | Action::PreviousSlide
            | Action::FirstSlide
            | Action::LastSlide
            | Action::EndShow
            | Action::LastViewed => true,
        };
        if !valid {
            *slot = None;
            self.adjusted = true;
            return;
        }
        if let Some(slide) = link.zoom_return
            && !self.document.is_some_and(|doc| doc.slide(slide).is_some())
        {
            link.zoom_return = None;
            self.adjusted = true;
        }
        if !self.transfer_media
            && let Some(sound) = link.sound
            && !self.document.is_some_and(|doc| doc.media(sound).is_some())
        {
            link.sound = None;
            self.adjusted = true;
        }
    }

    fn run(&mut self, props: &mut RunProps) {
        self.link(&mut props.link);
        self.link(&mut props.hover);
    }

    fn text(&mut self, body: &mut TextBody) {
        for level in body.list_style.levels.iter_mut().flatten() {
            self.run(&mut level.run);
        }
        for paragraph in &mut body.paragraphs {
            self.run(&mut paragraph.def_props);
            self.run(&mut paragraph.end_props);
            for run in &mut paragraph.runs {
                self.run(&mut run.props);
            }
        }
    }
}

/// Scope a rich text fragment to a verified open document. The text clipboard
/// does not transport media bytes, so unresolved picture formatting cannot be pasted.
pub fn scope_text(body: &mut TextBody, document: Option<&Presentation>) -> Result<bool, String> {
    let mut scope = LinkScope {
        document,
        transfer_media: false,
        adjusted: false,
    };
    scope.text(body);
    let mut missing = false;
    media_refs::visit::text(body, &mut |id| {
        missing |= !document.is_some_and(|doc| doc.media(*id).is_some());
    });
    if missing {
        return Err("此富文字引用源文稿的图片。可在源文稿内粘贴，或复制整个对象以携带图片".into());
    }
    Ok(scope.adjusted)
}

/// Object transfers carry their own media and the engine remaps it at insertion.
pub fn scope_objects(transfer: &mut Transfer, document: Option<&Presentation>) -> bool {
    let mut scope = LinkScope {
        document,
        transfer_media: true,
        adjusted: false,
    };
    for shape in &mut transfer.shapes {
        scope.link(&mut shape.click);
        scope.link(&mut shape.hover);
        if let Some(body) = &mut shape.text {
            scope.text(body);
        }
        if let ShapeKind::Table(table) = &mut shape.kind {
            for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
                scope.text(&mut cell.text);
            }
        }
    }
    if scope.adjusted {
        let mut retained = BTreeSet::new();
        media_refs::visit::shapes(&transfer.shapes, &mut |id| {
            retained.insert(*id);
        });
        transfer.media.retain(|part| retained.contains(&part.id));
    }
    scope.adjusted
}

/// Reject incoming draft links whose meaning cannot be established without a
/// verified source document. Do not strip or silently retarget user content.
pub(crate) fn validate_foreign_text_links(body: &TextBody) -> Result<(), String> {
    let mut checked = body.clone();
    let mut scope = LinkScope {
        document: None,
        transfer_media: false,
        adjusted: false,
    };
    scope.text(&mut checked);
    if scope.adjusted {
        return Err(
            "incoming text has unsafe or document-local links; scope them before paste".into(),
        );
    }
    Ok(())
}

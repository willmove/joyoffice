//! Inline still pictures (ECMA-376 Part 1 §17.3.3.19 `w:drawing`, §17.3.3.20
//! `w:pict`, §20.4.2.7 `a:blip`, §20.4.2.15 `wp:extent`).
//!
//! A single internal raster blip becomes an image resource and an inline
//! object. The drawing element is not registered as a fragment, so an
//! unedited paragraph keeps its original XML on export. Remote targets are
//! not fetched, and OLE, SVG and other active payloads are not executed.
//! Floating `wp:anchor` drawings stay on the existing preserve path.

use std::sync::Arc;

use joyoffice_opc::Target;
use joyword_model::{
    ImageLimits, ImageRefuse, InlineKind, InlineObject, MAX_IMAGE_BYTES, Measure, RunProps,
    SourceRef, StyleId, accept_image, display_extent, extent_emu,
};

use crate::backing::PreservedReason;
use crate::body::inline;
use crate::import::{Importer, Scope};
use crate::ns::{A, R, REL_IMAGE, VML, W, WP};
use crate::result::{DiagnosticKind, ImageOutcome, Severity};
use crate::xml::Element;
use joyoffice_opc::PartName;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PictureImport {
    /// An inline image was added. The element is not preserved and the
    /// paragraph is not locked for that reason.
    Imported,
    /// The element was preserved and the paragraph must be locked.
    Kept,
    /// Not a picture this slice maps. The caller preserves it as before.
    Pass,
}

pub(crate) fn import_container(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &mut joyword_model::Paragraph,
    format: (Option<StyleId>, Arc<RunProps>),
) -> PictureImport {
    if element.is(W, "object") {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            "",
            ImageOutcome::Active,
        );
    }
    if element.is(W, "drawing") || element.is(W, "pict") {
        return import_picture(importer, part, element, scope, paragraph, format);
    }
    if element.is(crate::ns::MC, "AlternateContent") {
        for picture in picture_descendants(element) {
            match import_container(importer, part, picture, scope, paragraph, format.clone()) {
                PictureImport::Imported => return PictureImport::Imported,
                PictureImport::Kept => return PictureImport::Kept,
                PictureImport::Pass => {}
            }
        }
        return PictureImport::Pass;
    }
    PictureImport::Pass
}

fn import_picture(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &mut joyword_model::Paragraph,
    format: (Option<StyleId>, Arc<RunProps>),
) -> PictureImport {
    if element.is(W, "drawing")
        && find(element, WP, "anchor").is_some()
        && find(element, WP, "inline").is_none()
    {
        return PictureImport::Pass;
    }
    let blips = collect_blips(element);
    if blips.is_empty() {
        return PictureImport::Pass;
    }
    if blips.len() != 1 {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            "",
            ImageOutcome::Malformed,
        );
    }
    let blip = &blips[0];
    let Some(embed) = blip.embed else {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            safe_id(blip.reported),
            ImageOutcome::Remote,
        );
    };
    let package = importer.backing.package();
    let relationship = package
        .relationships(Some(part))
        .iter()
        .find(|item| item.id.as_ref() == embed);
    let Some(relationship) = relationship else {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            safe_id(embed),
            ImageOutcome::Malformed,
        );
    };
    let reported = safe_id(relationship.id.as_ref());
    if !is_image_relationship(relationship.kind.as_ref()) {
        let outcome = if is_active_relationship(relationship.kind.as_ref()) {
            ImageOutcome::Active
        } else {
            ImageOutcome::Malformed
        };
        return refuse(
            importer, part, element, scope, paragraph, format, reported, outcome,
        );
    }
    let Target::Internal(name) = &relationship.target else {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            reported,
            ImageOutcome::Remote,
        );
    };
    let Some(media) = package.part(name.as_str()) else {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            reported,
            ImageOutcome::Malformed,
        );
    };
    if content_type_is_active(media.content_type.as_deref()) {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            reported,
            ImageOutcome::Active,
        );
    }
    if media.data.len() as u64 > MAX_IMAGE_BYTES {
        return refuse(
            importer,
            part,
            element,
            scope,
            paragraph,
            format,
            reported,
            ImageOutcome::Oversize,
        );
    }
    let accepted = match accept_image(media.data.as_ref(), &ImageLimits::default()) {
        Ok(image) => image,
        Err(error) => {
            return refuse(
                importer,
                part,
                element,
                scope,
                paragraph,
                format,
                reported,
                outcome_of(error),
            );
        }
    };
    let (width, height) = match placed_extent(element, accepted.width_px, accepted.height_px) {
        Ok(extent) => extent,
        Err(outcome) => {
            return refuse(
                importer, part, element, scope, paragraph, format, reported, outcome,
            );
        }
    };
    let resource = importer.document.ids().resource();
    importer.document.resources_mut().insert(
        resource,
        accepted.media_type,
        media.data.clone(),
        Some(Arc::new(SourceRef {
            part: name.as_str().into(),
            locator: embed.into(),
        })),
    );
    paragraph.push_inline(
        InlineObject {
            id: importer.document.ids().node(),
            kind: InlineKind::Image {
                resource,
                width,
                height,
            },
            source: Some(Importer::source(part, element)),
        },
        format.0,
        format.1,
    );
    PictureImport::Imported
}

#[allow(clippy::too_many_arguments)]
fn refuse(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &mut joyword_model::Paragraph,
    format: (Option<StyleId>, Arc<RunProps>),
    relationship: impl Into<String>,
    outcome: ImageOutcome,
) -> PictureImport {
    importer.diagnose(
        Severity::Warning,
        Some(part),
        Some(element.span.clone()),
        DiagnosticKind::Image {
            relationship: relationship.into(),
            outcome,
        },
    );
    inline(
        importer,
        part,
        element,
        scope,
        PreservedReason::Unsupported,
        paragraph,
        format,
    );
    PictureImport::Kept
}

struct BlipRef<'a> {
    embed: Option<&'a str>,
    reported: &'a str,
}

fn collect_blips(element: &Element) -> Vec<BlipRef<'_>> {
    let mut found = Vec::new();
    collect_blips_into(element, &mut found);
    found
}

fn collect_blips_into<'a>(element: &'a Element, found: &mut Vec<BlipRef<'a>>) {
    if element.is(A, "blip") {
        let embed = element.attribute(Some(R), "embed");
        let link = element.attribute(Some(R), "link");
        found.push(BlipRef {
            embed,
            reported: embed.or(link).unwrap_or(""),
        });
    } else if element.is(VML, "imagedata") {
        let id = element
            .attribute(Some(R), "id")
            .or_else(|| element.attribute(Some(R), "pict"));
        found.push(BlipRef {
            embed: id,
            reported: id.unwrap_or(""),
        });
    }
    for child in element.elements() {
        collect_blips_into(child, found);
    }
}

fn placed_extent(
    element: &Element,
    width_px: u32,
    height_px: u32,
) -> Result<(Measure, Measure), ImageOutcome> {
    if let Some(inline) = find(element, WP, "inline")
        && let Some(extent) = inline.child(WP, "extent")
    {
        let cx = extent
            .attribute(None, "cx")
            .and_then(|value| value.parse().ok());
        let cy = extent
            .attribute(None, "cy")
            .and_then(|value| value.parse().ok());
        if let (Some(cx), Some(cy)) = (cx, cy) {
            return extent_emu(cx, cy).map_err(|error| match error {
                ImageRefuse::OversizeDimension { .. } => ImageOutcome::Oversize,
                _ => ImageOutcome::Malformed,
            });
        }
        return Err(ImageOutcome::Malformed);
    }
    if let Some(shape) = find(element, VML, "shape")
        && let Some(style) = shape.attribute(None, "style")
        && let Some(extent) = style_extent(style)
    {
        return Ok(extent);
    }
    Ok(display_extent(width_px, height_px))
}

fn style_extent(style: &str) -> Option<(Measure, Measure)> {
    let width = css_emu(style, "width")?;
    let height = css_emu(style, "height")?;
    extent_emu(width, height).ok()
}

fn css_emu(style: &str, name: &str) -> Option<i64> {
    for piece in style.split(';') {
        let (key, value) = piece.split_once(':')?;
        if !key.trim().eq_ignore_ascii_case(name) {
            continue;
        }
        return css_length_emu(value.trim());
    }
    None
}

fn css_length_emu(token: &str) -> Option<i64> {
    let split = token
        .find(|ch: char| ch.is_ascii_alphabetic())
        .unwrap_or(token.len());
    let (number, unit) = token.split_at(split);
    let value: f64 = number.trim().parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let per = match unit.trim().to_ascii_lowercase().as_str() {
        "pt" => 12_700.0,
        "px" => 9_525.0,
        "in" => 914_400.0,
        "cm" => 360_000.0,
        "mm" => 36_000.0,
        "emu" => 1.0,
        _ => return None,
    };
    let emu = (value * per).round();
    if !emu.is_finite() || emu <= 0.0 || emu > i64::MAX as f64 {
        return None;
    }
    Some(emu as i64)
}

fn outcome_of(error: ImageRefuse) -> ImageOutcome {
    match error {
        ImageRefuse::Remote => ImageOutcome::Remote,
        ImageRefuse::Active => ImageOutcome::Active,
        ImageRefuse::OversizeBytes { .. }
        | ImageRefuse::OversizePixels { .. }
        | ImageRefuse::OversizeDimension { .. } => ImageOutcome::Oversize,
        ImageRefuse::Empty | ImageRefuse::Malformed => ImageOutcome::Malformed,
    }
}

fn is_image_relationship(kind: &str) -> bool {
    kind == REL_IMAGE || kind.ends_with("/image")
}

fn is_active_relationship(kind: &str) -> bool {
    let lower = kind.to_ascii_lowercase();
    lower.contains("oleobject") || lower.contains("activex") || lower.contains("package")
}

fn content_type_is_active(content_type: Option<&str>) -> bool {
    let Some(content_type) = content_type else {
        return false;
    };
    let lower = content_type.to_ascii_lowercase();
    lower.contains("svg")
        || lower.contains("html")
        || lower.contains("javascript")
        || (lower.contains("xml") && !lower.starts_with("image/"))
}

fn safe_id(id: &str) -> String {
    if id.is_empty()
        || id.len() > 64
        || id.contains("://")
        || id.contains('\\')
        || id.contains("//")
        || id.contains(':')
    {
        return String::new();
    }
    id.to_owned()
}

fn picture_descendants(element: &Element) -> Vec<&Element> {
    let mut found = Vec::new();
    for child in element.elements() {
        collect_pictures(child, &mut found);
    }
    found
}

fn collect_pictures<'a>(element: &'a Element, found: &mut Vec<&'a Element>) {
    if element.is(W, "drawing") || element.is(W, "pict") || element.is(W, "object") {
        found.push(element);
        return;
    }
    for child in element.elements() {
        collect_pictures(child, found);
    }
}

fn find<'a>(element: &'a Element, namespace: &str, local: &str) -> Option<&'a Element> {
    if element.is(namespace, local) {
        return Some(element);
    }
    for child in element.elements() {
        if let Some(found) = find(child, namespace, local) {
            return Some(found);
        }
    }
    None
}

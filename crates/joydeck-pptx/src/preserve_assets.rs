//! Import known media relationships for drawings produced by the generator.

use super::{El, PartEdits, PptxError, content, error, links, opc};

pub(super) struct Assets<'a> {
    pictures: content::Pictures<'a>,
}

impl<'a> Assets<'a> {
    pub fn new(original: &'a opc::Package) -> Self {
        Self {
            pictures: content::Pictures::new(original),
        }
    }

    pub fn picture(
        &mut self,
        generated: &opc::Package,
        path: &str,
        parts: &mut PartEdits<'_>,
        types: &mut El,
    ) -> Result<String, PptxError> {
        self.pictures.target(generated, path, parts, types)
    }

    pub fn rebind(
        &mut self,
        node: &mut El,
        generated: &opc::Package,
        relationships: &mut links::Relationships<'_>,
        parts: &mut PartEdits<'_>,
        types: &mut El,
    ) -> Result<(), PptxError> {
        if node.name == "a:blip" && node.attr("r:link").is_some_and(|id| !id.is_empty()) {
            return Err(error("edited linked resources need .joydeck Save As"));
        }
        if let Some((attribute, kind)) = role(node)
            && let Some(id) = node.attr(attribute)
        {
            let edge = relationships
                .generated
                .get(id)
                .filter(|edge| edge.kind == kind)
                .ok_or_else(|| error("missing generated drawing resource relationship"))?;
            let target = if edge.external {
                if !matches!(kind, "audio" | "video") {
                    return Err(error("edited linked resources need .joydeck Save As"));
                }
                edge.target.clone()
            } else {
                self.picture(generated, &edge.target, parts, types)?
            };
            node.set_attr(
                attribute,
                &relationships.add_typed(&edge.ty, &target, edge.external),
            );
        }
        for child in node.elements_mut() {
            self.rebind(child, generated, relationships, parts, types)?;
        }
        Ok(())
    }
}

fn role(node: &El) -> Option<(&'static str, &'static str)> {
    match node.name.as_str() {
        "a:blip" => Some(("r:embed", "image")),
        "a:videoFile" => Some(("r:link", "video")),
        "a:audioFile" => Some(("r:link", "audio")),
        "p14:media" => Some(("r:embed", "media")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use joydeck_model::{ShapeId, ShapeKind};

    #[test]
    fn copied_video_reuses_source_payload_for_both_relationship_roles()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = include_bytes!("../../../tests/fixtures/joydeck/r6-r7/delivery-media.pptx");
        let base = crate::import(source)?;
        let (page, source_shape) = base
            .slides
            .iter()
            .enumerate()
            .find_map(|(page, slide)| {
                slide
                    .shapes
                    .iter()
                    .find(|shape| matches!(shape.kind, ShapeKind::Media(_)))
                    .map(|shape| (page, shape))
            })
            .ok_or("fixture media missing")?;
        let mut current = base.clone();
        let mut copy = source_shape.clone();
        copy.id = ShapeId(current.alloc_id().ok_or("shape id exhausted")?);
        let copied_id = copy.id;
        Arc::make_mut(&mut current.slides[page]).shapes.push(copy);
        let bindings: Vec<_> = base
            .slides
            .iter()
            .map(|slide| crate::SlideBinding {
                slide: slide.id,
                source: slide.id,
                shapes: slide
                    .shapes
                    .iter()
                    .map(|shape| (shape.id, shape.id))
                    .collect(),
            })
            .collect();
        let output = crate::export_preserving(source, &base, &current, &bindings)?;
        let imported = crate::import(&output)?;
        let original = imported.slides[page]
            .shapes
            .iter()
            .find(|shape| shape.id == source_shape.id)
            .ok_or("first video missing")?;
        let duplicate = imported.slides[page]
            .shapes
            .iter()
            .find(|shape| shape.id == copied_id)
            .ok_or("copied video missing")?;
        let (ShapeKind::Media(original), ShapeKind::Media(duplicate)) =
            (&original.kind, &duplicate.kind)
        else {
            return Err("preserved drawing is no longer media".into());
        };
        assert_eq!(duplicate.media, original.media);
        assert_eq!(duplicate.poster, original.poster);
        assert_eq!(duplicate.crop, original.crop);
        let archive = crate::opc::Package::open(&output)?;
        let part = format!("ppt/slides/slide{}.xml", page + 1);
        let slide = archive.xml(&part).ok_or("modified slide missing")?;
        let mut videos = Vec::new();
        slide.root.find_all("videoFile", &mut videos);
        let mut embedded = Vec::new();
        slide.root.find_all("media", &mut embedded);
        let rels = archive.rels(&part);
        let legacy = videos
            .last()
            .and_then(|node| node.attr("r:link"))
            .and_then(|id| rels.get(id))
            .ok_or("legacy media relationship missing")?;
        let modern = embedded
            .last()
            .and_then(|node| node.attr("r:embed"))
            .and_then(|id| rels.get(id))
            .ok_or("modern media relationship missing")?;
        assert_eq!(legacy.kind, "video");
        assert_eq!(modern.ty, crate::opc::RT_MEDIA);
        assert_eq!(legacy.target, modern.target);
        assert!(
            archive
                .get(&legacy.target)
                .is_some_and(|data| !data.is_empty())
        );
        Ok(())
    }
}

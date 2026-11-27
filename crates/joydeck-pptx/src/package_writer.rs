//! Content type registration and serialization of a generated OPC package.

use std::collections::{BTreeMap, HashMap};

use crate::opc::{Package, norm, rels_path};
use crate::relationships::RelsOut;
use crate::xml::{A, W};
use crate::{PptxError, archive};

const MANIFEST: &str = "[Content_Types].xml";

struct Member {
    name: String,
    data: Vec<u8>,
    content: Content,
}

enum Content {
    Xml(String),
    Binary(String),
    Relationships,
    Carried,
}

#[derive(Default)]
pub struct PackageOut {
    entries: Vec<Member>,
    index: HashMap<String, usize>,
}

impl PackageOut {
    fn insert(&mut self, name: &str, data: Vec<u8>, content: Content) {
        let key = norm(name);
        if key == norm(MANIFEST) || self.index.contains_key(&key) {
            log::warn!("pptx: part {name} written twice; keeping the first");
            return;
        }
        self.index.insert(key, self.entries.len());
        self.entries.push(Member {
            name: name.into(),
            data,
            content,
        });
    }

    pub fn xml_part(&mut self, name: &str, content_type: &str, data: Vec<u8>) {
        self.insert(name, data, Content::Xml(content_type.into()));
    }

    pub fn binary(&mut self, name: &str, content_type: &str, data: Vec<u8>) {
        self.insert(name, data, Content::Binary(content_type.into()));
    }

    pub fn raw(&mut self, name: &str, data: Vec<u8>) {
        self.insert(name, data, Content::Carried);
    }

    pub fn rels(&mut self, part: &str, relations: &RelsOut) {
        if !relations.is_empty() {
            let name = rels_path(part);
            let bytes = relations.to_xml();
            if let Some(member) = self
                .index
                .get(&norm(&name))
                .and_then(|&index| self.entries.get_mut(index))
            {
                member.data = bytes;
                member.content = Content::Relationships;
            } else {
                self.insert(&name, bytes, Content::Relationships);
            }
        }
    }

    fn manifest(&self) -> Result<Vec<u8>, PptxError> {
        let mut defaults = vec![
            (
                "rels".into(),
                "application/vnd.openxmlformats-package.relationships+xml",
            ),
            ("xml".into(), "application/xml"),
        ];
        let mut overrides = Vec::new();
        for member in &self.entries {
            let mut segments = member.name.split('/');
            if member.name.contains(['\\', ':', '\0'])
                || segments.clone().any(|part| matches!(part, "" | "." | ".."))
            {
                return Err(PptxError::Write(format!(
                    "invalid generated package member: {}",
                    member.name
                )));
            }
            match &member.content {
                Content::Xml(mime) => overrides.push((&member.name, mime.as_str())),
                Content::Binary(mime) => {
                    let ext = segments
                        .next_back()
                        .and_then(|file| file.rsplit_once('.'))
                        .map(|(_, ext)| ext.to_ascii_lowercase());
                    let Some(ext) = ext else {
                        overrides.push((&member.name, mime.as_str()));
                        continue;
                    };
                    match defaults.iter().find(|(known, _)| *known == ext) {
                        Some((_, known_type)) if *known_type == mime => {}
                        Some(_) => overrides.push((&member.name, mime.as_str())),
                        None => defaults.push((ext, mime.as_str())),
                    }
                }
                Content::Relationships | Content::Carried => {}
            }
        }
        let mut xml = W::new();
        xml.open(
            "Types",
            A::new().a("xmlns", joyoffice_opc::CONTENT_TYPES_NAMESPACE),
        );
        for (ext, mime) in defaults {
            xml.empty(
                "Default",
                A::new().a("Extension", ext).a("ContentType", mime),
            );
        }
        for (name, mime) in overrides {
            xml.empty(
                "Override",
                A::new()
                    .a("PartName", format!("/{name}"))
                    .a("ContentType", mime),
            );
        }
        xml.close("Types");
        Ok(xml.finish())
    }

    pub(super) fn editable_parts(self) -> Result<Package, PptxError> {
        let manifest = self.manifest()?;
        let mut files: BTreeMap<_, _> = self
            .entries
            .into_iter()
            .map(|part| (part.name, part.data))
            .collect();
        files.insert(MANIFEST.into(), manifest);
        Package::from_parts(files)
    }

    pub fn finish(self) -> Result<Vec<u8>, PptxError> {
        let manifest = self.manifest()?;
        archive::write(
            std::iter::once((MANIFEST, manifest.as_slice(), true)).chain(self.entries.iter().map(
                |member| {
                    (
                        member.name.as_str(),
                        member.data.as_slice(),
                        !matches!(member.content, Content::Binary(_)),
                    )
                },
            )),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_types_follow_the_effective_member_inventory() -> Result<(), PptxError> {
        let mut package = PackageOut::default();
        package.binary("ppt/media/first.bin", "video/example", vec![1, 2, 3]);
        package.binary("PPT/media/first.bin", "audio/discarded", vec![4]);
        package.binary("ppt/media/second.bin", "audio/example", vec![5, 6]);
        let mut relationships = RelsOut::default();
        let first = relationships.add("video", "../media/first.bin", false);
        package.rels("ppt/slides/slide1.xml", &relationships);
        let second = relationships.add("audio", "../media/second.bin", false);
        package.rels("ppt/slides/slide1.xml", &relationships);
        let file = Package::open(&package.finish()?)?;
        assert_eq!(file.get("ppt/media/first.bin"), Some([1, 2, 3].as_slice()));
        assert_eq!(
            file.content_type("ppt/media/first.bin").as_deref(),
            Some("video/example")
        );
        assert_eq!(
            file.content_type("ppt/media/second.bin").as_deref(),
            Some("audio/example")
        );
        let relationships = file.rels("ppt/slides/slide1.xml");
        assert_eq!(relationships.target(&first), Some("ppt/media/first.bin"));
        assert_eq!(relationships.target(&second), Some("ppt/media/second.bin"));
        Ok(())
    }
}

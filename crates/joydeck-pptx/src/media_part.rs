//! Map presentation media IDs to package parts and relationships.

use std::collections::HashMap;

use joydeck_model::{MediaId, MediaItem, Presentation};

use crate::opc::{self, PackageOut};
use crate::read::Imp;
use crate::write::Out;

impl Imp<'_> {
    pub fn media_for(&mut self, path: &str) -> Option<MediaId> {
        let key = opc::norm(path);
        if let Some(&id) = self.media_by_part.get(&key) {
            return Some(id);
        }
        let package = self.pkg;
        let bytes = package.shared_member(path)?;
        let file = opc::file_of(package.original_name(path).unwrap_or(path));
        let kind = package.content_type(path).filter(|value| !value.is_empty());
        let id = MediaId(self.alloc());
        self.media.push(MediaItem {
            id,
            name: file.into(),
            content_type: kind.unwrap_or_else(|| crate::mime_for(file).into()),
            data: bytes,
            link: None,
        });
        self.media_by_part.insert(key, id);
        Some(id)
    }

    pub fn media_link(&mut self, uri: &str) -> MediaId {
        if let Some(&id) = self.media_by_uri.get(uri) {
            return id;
        }
        let id = MediaId(self.alloc());
        let name = opc::file_of(uri);
        self.media.push(MediaItem {
            id,
            name: name.into(),
            content_type: crate::mime_for(name).into(),
            data: Default::default(),
            link: Some(uri.into()),
        });
        self.media_by_uri.insert(uri.into(), id);
        id
    }
}

#[derive(Default)]
pub(super) struct Writer {
    embedded: HashMap<MediaId, String>,
    pictures: u32,
    other: u32,
    missing_picture: Option<String>,
}

impl Writer {
    pub(super) fn relationship(
        &mut self,
        model: &Presentation,
        package: &mut PackageOut,
        owner: &mut Out,
        id: MediaId,
        kind: &str,
        allow_external: bool,
    ) -> Option<String> {
        let media = model.media(id)?;
        if media.data.is_empty() {
            let uri = media.link.as_deref().filter(|_| allow_external)?;
            return Some(owner.rels.add(kind, uri, true));
        }
        let path = if let Some(path) = self.embedded.get(&id) {
            path.clone()
        } else {
            let ext = extension(media);
            let content_type = if media.content_type.contains('/') {
                media.content_type.clone()
            } else {
                crate::mime_for(&format!("media.{ext}")).into()
            };
            let path = self.next_path(content_type.starts_with("image/"), &ext);
            package.binary(&path, &content_type, media.data.as_ref().clone());
            self.embedded.insert(id, path.clone());
            path
        };
        Some(
            owner
                .rels
                .add(kind, &opc::relative(&owner.name, &path), false),
        )
    }

    pub(super) fn missing(&mut self, package: &mut PackageOut, owner: &mut Out) -> String {
        if self.missing_picture.is_none() {
            let path = self.next_path(true, "png");
            package.binary(&path, "image/png", crate::png::transparent_1x1());
            self.missing_picture = Some(path);
        }
        owner.rels.add(
            "image",
            &opc::relative(
                &owner.name,
                self.missing_picture.as_deref().unwrap_or_default(),
            ),
            false,
        )
    }

    fn next_path(&mut self, image: bool, extension: &str) -> String {
        let (stem, counter) = if image {
            ("image", &mut self.pictures)
        } else {
            ("media", &mut self.other)
        };
        *counter += 1;
        format!("ppt/media/{stem}{counter}.{extension}")
    }
}

fn extension(media: &MediaItem) -> String {
    let extension = match media.content_type.as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/svg+xml" => "svg",
        "image/x-emf" | "image/emf" => "emf",
        "image/x-wmf" | "image/wmf" => "wmf",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/x-ms-wmv" => "wmv",
        "video/x-msvideo" => "avi",
        "video/webm" => "webm",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/x-ms-wma" => "wma",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        "model/gltf-binary" => "glb",
        _ => {
            for (magic, extension) in [
                (b"\x89PNG".as_slice(), "png"),
                (&[0xff, 0xd8], "jpeg"),
                (b"GIF8", "gif"),
            ] {
                if media.data.starts_with(magic) {
                    return extension.into();
                }
            }
            return media
                .name
                .rsplit_once('.')
                .map(|(_, suffix)| suffix)
                .filter(|suffix| {
                    (1..=5).contains(&suffix.len())
                        && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
                })
                .map(str::to_ascii_lowercase)
                .unwrap_or_else(|| "bin".into());
        }
    };
    extension.into()
}

//! Map presentation media IDs to package parts and relationships.

use std::collections::HashMap;

use joydeck_model::{MediaClip, MediaId, MediaItem, Presentation, style::PictureFill};

use crate::opc::{self, PackageOut};
use crate::read::{Imp, Part, dml};
use crate::write::{Exp, Out};
use crate::xml::{A, El, W};

pub(super) fn clip(
    imp: &mut Imp,
    part: &Part,
    source: &El,
    poster: Option<&PictureFill>,
) -> Option<MediaClip> {
    let properties = source.path(&["nvPicPr", "nvPr"])?;
    let video = properties.child("videoFile");
    let audio = properties.child("audioFile");
    let extension = properties
        .child("extLst")
        .and_then(|list| list.find("media"));
    let media = extension
        .and_then(|node| dml::media_ref(imp, part, node))
        .or_else(|| {
            video
                .or(audio)
                .and_then(|node| dml::media_ref(imp, part, node))
        })?;
    let time = |group, attr| {
        extension
            .and_then(|media| media.child(group))
            .map_or(0, |node| milliseconds(node.attr(attr)))
    };
    let bookmarks = extension
        .and_then(|media| media.child("bmkLst"))
        .into_iter()
        .flat_map(|list| list.children_named("bmk"))
        .take(1000)
        .map(|mark| {
            (
                mark.attr("name").unwrap_or_default().into(),
                milliseconds(mark.attr("time")),
            )
        })
        .collect();
    Some(MediaClip {
        media,
        video: video.is_some() || audio.is_none(),
        poster: poster.map(|poster| poster.media),
        crop: poster.map(|poster| poster.crop).unwrap_or_default(),
        trim_start_ms: time("trim", "st"),
        trim_end_ms: time("trim", "end"),
        fade_in_ms: time("fade", "in"),
        fade_out_ms: time("fade", "out"),
        bookmarks,
        volume: 1.0,
        ..Default::default()
    })
}

fn milliseconds(value: Option<&str>) -> u32 {
    value
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|number| number.is_finite())
        .map_or(0, |number| number.clamp(0.0, f64::from(u32::MAX)) as u32)
}

pub(super) struct ClipRefs {
    legacy: Option<String>,
    embedded: Option<String>,
}

pub(super) fn clip_refs(export: &mut Exp<'_>, owner: &mut Out, clip: &MediaClip) -> ClipRefs {
    ClipRefs {
        legacy: export.media_rel(
            owner,
            clip.media,
            if clip.video { "video" } else { "audio" },
        ),
        embedded: export.media_rel_new(owner, clip.media, opc::RT_MEDIA),
    }
}

/// Write legacy audio/video and Office's embedded-media relationship from the same asset.
pub(super) fn write_clip(xml: &mut W, clip: &MediaClip, refs: ClipRefs) {
    let tag = if clip.video {
        "a:videoFile"
    } else {
        "a:audioFile"
    };
    if let Some(id) = refs.legacy {
        xml.empty(tag, A::new().a("r:link", id));
    }
    let Some(id) = refs.embedded else { return };
    xml.open0("p:extLst");
    xml.open(
        "p:ext",
        A::new().a("uri", "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}"),
    );
    xml.open(
        "p14:media",
        A::new().a("xmlns:p14", opc::NS_P14).a("r:embed", id),
    );
    for (tag, attrs) in [
        (
            "p14:trim",
            [("st", clip.trim_start_ms), ("end", clip.trim_end_ms)],
        ),
        (
            "p14:fade",
            [("in", clip.fade_in_ms), ("out", clip.fade_out_ms)],
        ),
    ] {
        if attrs.iter().any(|(_, value)| *value != 0) {
            xml.empty(
                tag,
                attrs
                    .into_iter()
                    .fold(A::new(), |attributes, (key, value)| {
                        attributes.o(key, (value != 0).then_some(value))
                    }),
            );
        }
    }
    if !clip.bookmarks.is_empty() {
        xml.open0("p14:bmkLst");
        for (name, time) in clip.bookmarks.iter().take(1000) {
            xml.empty("p14:bmk", A::new().a("name", name).a("time", time));
        }
        xml.close("p14:bmkLst");
    }
    xml.close("p14:media");
    xml.close("p:ext");
    xml.close("p:extLst");
}

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

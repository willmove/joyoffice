//! Read the format-1 language packs used by JoyWord. Archive names are
//! interpreted as portable tar paths before any filesystem path is built.
use super::storage::{
    Error, IndexError, MAX_PACK_ARCHIVE_BYTES, MAX_PACK_ENTRIES, MAX_PACK_FILE_BYTES,
    MAX_PACK_INDEX_BYTES, MAX_PACK_UNPACKED_BYTES, MAX_PACKS, PackEntry, PackIndex, is_format,
    is_locale, is_sha256, local_source, plain_text, windows_reserved,
};
use ruzstd::decoding::{FrameDecoder, StreamingDecoder};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_PACK_FRAMES: usize = 4096;
const STREAM_LIMIT: u64 = MAX_PACK_UNPACKED_BYTES + (MAX_PACK_ENTRIES as u64 + 2) * 1024 + 20 * 512;

type Decoder<'a> = StreamingDecoder<&'a [u8], FrameDecoder>;

struct PackStream<'a> {
    current: Option<Decoder<'a>>,
    remaining_frames: usize,
}

impl<'a> PackStream<'a> {
    fn start(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() as u64 > MAX_PACK_ARCHIVE_BYTES {
            return Err("compressed dictionary exceeds the archive limit".into());
        }
        let decoder = StreamingDecoder::new(bytes).map_err(|e| format!("not zstd: {e}"))?;
        Ok(Self {
            current: Some(decoder),
            remaining_frames: MAX_PACK_FRAMES - 1,
        })
    }
}

impl Read for PackStream<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while let Some(mut decoder) = self.current.take() {
            let read = decoder.read(buf)?;
            if read != 0 {
                self.current = Some(decoder);
                return Ok(read);
            }
            let (next, workspace) = decoder.into_parts();
            if !next.is_empty() {
                self.remaining_frames = self.remaining_frames.checked_sub(1).ok_or_else(|| {
                    io::Error::other(format!("more than {MAX_PACK_FRAMES} zstd frames"))
                })?;
                self.current = Some(
                    StreamingDecoder::new_with_decoder(next, workspace)
                        .map_err(io::Error::other)?,
                );
            }
        }
        Ok(0)
    }
}

#[derive(Default)]
struct Members {
    written: BTreeSet<PathBuf>,
    payload_bytes: u64,
}

fn member(raw: &[u8], directory: bool) -> Result<PathBuf, String> {
    let name = std::str::from_utf8(raw).map_err(|_| "non-UTF-8 dictionary member")?;
    if name.starts_with('/') || name.contains(['\\', ':']) {
        return Err("dictionary member must be relative".into());
    }
    let mut parts = name
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".");
    let components = (parts.next(), parts.next(), parts.next());
    match (directory, components) {
        (true, (None, None, None)) => Ok(PathBuf::new()),
        (true, (Some("LICENCES"), None, None)) => Ok("LICENCES".into()),
        (
            false,
            (
                Some(root @ ("pack.toml" | "spelling.dic" | "spelling.aff" | "hyphenation.pat")),
                None,
                None,
            ),
        ) => Ok(root.into()),
        (false, (Some("LICENCES"), Some(file), None))
            if file.len() <= 100
                && !file.starts_with('.')
                && !file.ends_with('.')
                && !windows_reserved(file)
                && file
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) =>
        {
            Ok(Path::new("LICENCES").join(file))
        }
        _ => Err(format!(
            "unsupported dictionary member {:?}",
            name.chars().take(160).collect::<String>()
        )),
    }
}

impl Members {
    fn extract<R: Read>(
        &mut self,
        file: &mut tar::Entry<'_, R>,
        root: &Path,
    ) -> Result<(), String> {
        let kind = file.header().entry_type();
        if !(kind.is_dir() || kind.is_file()) {
            return Err("only dictionary files and folders can be installed".into());
        }
        let relative = member(&file.path_bytes(), kind.is_dir())?;
        let path = root.join(&relative);
        if kind.is_dir() {
            return fs::create_dir_all(path).map_err(|e| e.to_string());
        }
        let size = file.size();
        if size > MAX_PACK_FILE_BYTES {
            return Err(format!(
                "{}: larger than {MAX_PACK_FILE_BYTES} bytes",
                relative.display()
            ));
        }
        self.payload_bytes = self
            .payload_bytes
            .checked_add(size)
            .filter(|&n| n <= MAX_PACK_UNPACKED_BYTES)
            .ok_or("dictionary contents exceed total size limit")?;
        if !self.written.insert(relative) {
            return Err("duplicate dictionary member".into());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = File::create_new(path).map_err(|e| e.to_string())?;
        let count = io::copy(file, &mut out).map_err(|e| e.to_string())?;
        if count != size {
            return Err("incomplete dictionary member".into());
        }
        Ok(())
    }

    fn verify(&self, root: &Path, pack: &PackEntry) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Manifest {
            locale: String,
        }

        let text =
            fs::read_to_string(root.join("pack.toml")).map_err(|e| format!("pack.toml: {e}"))?;
        let manifest: Manifest = toml::from_str(&text).map_err(|e| format!("pack.toml: {e}"))?;
        if manifest.locale != pack.locale {
            return Err(format!("pack.toml does not name {}", pack.locale));
        }
        let has = |name: &str| self.written.contains(Path::new(name));
        if pack.spelling && (!has("spelling.aff") || !has("spelling.dic")) {
            return Err("the index promises a spelling dictionary the archive doesn't have".into());
        }
        if pack.hyphenation && !has("hyphenation.pat") {
            return Err("the index promises hyphenation patterns the archive doesn't have".into());
        }
        Ok(())
    }
}

pub(super) fn unpack(bytes: &[u8], root: &Path, pack: &PackEntry) -> Result<(), String> {
    let input = PackStream::start(bytes)?.take(STREAM_LIMIT + 1);
    let mut archive = tar::Archive::new(input);
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut members = Members::default();
    for (number, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        if number >= MAX_PACK_ENTRIES {
            return Err("too many dictionary members".into());
        }
        members.extract(&mut entry.map_err(|e| e.to_string())?, root)?;
    }
    let mut remaining = archive.into_inner();
    let mut padding = [0; 8192];
    loop {
        let n = remaining.read(&mut padding).map_err(|e| e.to_string())?;
        if remaining.limit() == 0 {
            return Err("dictionary tar stream exceeds its byte budget".into());
        }
        if n == 0 {
            break;
        }
        if padding[..n].iter().any(|&v| v != 0) {
            return Err("data after the end of the archive".into());
        }
    }
    members.verify(root, pack)
}

pub(super) fn parse_index(bytes: &[u8], allow_local: bool) -> Result<PackIndex, Error> {
    fn invalid(reason: impl std::fmt::Display) -> Error {
        Error::Index(IndexError::Invalid(format!(
            "Language pack index: {reason}"
        )))
    }
    if bytes.len() as u64 > MAX_PACK_INDEX_BYTES {
        return Err(Error::Index(IndexError::TooLarge {
            max: MAX_PACK_INDEX_BYTES as usize,
        }));
    }
    let mut index: PackIndex = serde_json::from_slice(bytes)
        .map_err(|e| Error::Index(IndexError::Json(format!("Language pack index: {e}"))))?;
    if index.format != 1 {
        return Err(invalid(format!("unsupported format {}", index.format)));
    }
    if index.packs.len() > MAX_PACKS {
        return Err(invalid("too many packs"));
    }
    let mut locales = BTreeSet::new();
    index.release = plain_text(&index.release);
    for pack in &mut index.packs {
        if !is_locale(&pack.locale) || !locales.insert(pack.locale.as_str()) {
            return Err(invalid(format!(
                "invalid or repeated language tag {:?}",
                pack.locale.chars().take(40).collect::<String>()
            )));
        }
        let for_pack = |reason: &str| invalid(format!("{}: {reason}", pack.locale));
        if pack.size > MAX_PACK_ARCHIVE_BYTES || !is_sha256(&pack.sha256) {
            return Err(for_pack("invalid size or sha256"));
        }
        let archive_allowed = if pack.url.starts_with("https://") {
            https_url(&pack.url)
        } else {
            allow_local && !pack.url.is_empty() && local_source(&pack.url).is_some()
        };
        if !archive_allowed {
            return Err(for_pack("invalid dictionary archive location"));
        }
        if let Some(source) = &pack.source
            && (source.len() > 2048 || !https_url(source))
        {
            return Err(for_pack("invalid dictionary source URL"));
        }
        for field in [
            &mut pack.name,
            &mut pack.licence,
            &mut pack.class,
            &mut pack.version,
        ] {
            *field = plain_text(field);
        }
    }
    Ok(index)
}

fn https_url(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !text
        .chars()
        .any(|c| c.is_control() || c == ' ' || is_format(c))
        && authority
            .parse::<ureq::http::uri::Authority>()
            .is_ok_and(|host| !host.host().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_preserves_order_and_rejects_oversized_or_duplicate_entries()
    -> Result<(), Box<dyn std::error::Error>> {
        let entry = PackEntry {
            locale: String::new(),
            name: " Sprache ".into(),
            version: String::new(),
            url: "https://example.org/archive".into(),
            sha256: "a".repeat(64),
            size: 1,
            licence: "MIT".into(),
            class: String::new(),
            spelling: false,
            hyphenation: false,
            source: Some("https://example.org/词典".into()),
        };
        let mut spec = PackIndex {
            format: 1,
            release: "test".into(),
            packs: (0..MAX_PACKS)
                .rev()
                .map(|i| PackEntry {
                    locale: format!("x-{i}"),
                    ..entry.clone()
                })
                .collect(),
        };
        let observed = PackIndex::from_json(&serde_json::to_vec(&spec)?)?;
        assert!(
            observed
                .packs
                .iter()
                .zip(&spec.packs)
                .all(|(a, b)| a.locale == b.locale && a.name == "Sprache")
        );
        spec.packs.push(PackEntry {
            locale: "extra".into(),
            ..entry
        });
        assert!(PackIndex::from_json(&serde_json::to_vec(&spec)?).is_err());
        spec.packs.truncate(MAX_PACKS);
        let repeated = spec.packs[0].clone();
        spec.packs[MAX_PACKS - 1] = repeated;
        assert!(PackIndex::from_json(&serde_json::to_vec(&spec)?).is_err());
        let generous_padding = vec![b' '; MAX_PACK_INDEX_BYTES as usize + 1];
        assert!(matches!(
            PackIndex::from_json(&generous_padding),
            Err(Error::Index(IndexError::TooLarge { .. }))
        ));
        Ok(())
    }

    #[test]
    fn only_portable_payload_names_are_allowed() {
        for name in ["LICENCES/LICENSE-1.0", "./LICENCES//NOTICE", "spelling.dic"] {
            assert!(member(name.as_bytes(), false).is_ok());
        }
        for name in [
            "README",
            "LICENCES/new/NOTICE",
            "LICENCES/.",
            "LICENCES/NUL.txt",
        ] {
            assert!(member(name.as_bytes(), false).is_err());
        }
        assert_eq!(member(b"./", true), Ok(PathBuf::new()));
        assert_eq!(member(b"./LICENCES/", true), Ok("LICENCES".into()));
    }
}

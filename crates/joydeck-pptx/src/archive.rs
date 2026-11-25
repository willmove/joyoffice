//! ZIP inventory and transaction output for PresentationML packages.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write};

use crate::{
    PptxError,
    opc::{Package, norm},
};

/// Read every source member before attempting preserving writeback.
pub(super) fn read_for_writeback(bytes: &[u8]) -> Result<Package, PptxError> {
    let budget = joyoffice_opc::Budget {
        max_entries: 4096,
        max_part_bytes: 500 << 20,
        max_total_bytes: 1000 << 20,
        ..Default::default()
    };
    let error = |error| PptxError::Write(format!("invalid original package: {error:?}"));
    let archive = joyoffice_opc::ZipArchive::new(bytes, &budget).map_err(error)?;
    let mut names = BTreeSet::new();
    let mut parts = BTreeMap::new();
    for (index, entry) in archive.entries().iter().enumerate() {
        let name = &entry.name;
        if entry.is_symlink()
            || name.contains(['\\', ':', '\0'])
            || name
                .trim_end_matches('/')
                .split('/')
                .any(|segment| matches!(segment, "" | "." | ".."))
            || !names.insert(name.to_lowercase())
        {
            return Err(PptxError::Write(format!(
                "unsafe or duplicate original package path: {name}"
            )));
        }
        let part = archive.read(index).map_err(error)?;
        if !name.ends_with('/') {
            parts.insert(name.clone(), part);
        }
    }
    Package::from_parts(parts)
}

/// Keep an immutable source package and store only changed or new members.
pub(super) struct PartEdits<'a> {
    source: &'a Package,
    changed: BTreeMap<String, (String, Vec<u8>)>,
}

impl<'a> PartEdits<'a> {
    pub(super) fn new(source: &'a Package) -> Self {
        Self {
            source,
            changed: BTreeMap::new(),
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<&[u8]> {
        self.changed
            .get(&norm(name))
            .map(|(_, bytes)| bytes.as_slice())
            .or_else(|| self.source.get(name))
    }

    pub(super) fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub(super) fn insert(&mut self, name: String, bytes: Vec<u8>) {
        let key = norm(&name);
        if let Some((_, current)) = self.changed.get_mut(&key) {
            *current = bytes;
        } else {
            let name = self
                .source
                .original_name(&name)
                .map(str::to_owned)
                .unwrap_or(name);
            self.changed.insert(key, (name, bytes));
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> + '_ {
        self.source
            .members()
            .filter(|(key, ..)| !self.changed.contains_key(*key))
            .map(|(_, name, bytes)| (name, bytes))
            .chain(
                self.changed
                    .values()
                    .map(|(name, bytes)| (name.as_str(), bytes.as_slice())),
            )
    }

    pub(super) fn finish(&self) -> Result<Vec<u8>, PptxError> {
        let mut entries: Vec<_> = self.iter().collect();
        entries.sort_unstable_by_key(|(name, _)| *name);
        write(entries.into_iter().map(|(name, bytes)| (name, bytes, true)))
    }
}

pub(super) fn write<'a>(
    files: impl IntoIterator<Item = (&'a str, &'a [u8], bool)>,
) -> Result<Vec<u8>, PptxError> {
    let error = |error: &dyn std::fmt::Display| PptxError::Write(error.to_string());
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes, compress) in files {
        let method = if compress {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .large_file(bytes.len() as u64 >= u32::MAX as u64 / 2);
        archive.start_file(name, options).map_err(|e| error(&e))?;
        archive.write_all(bytes).map_err(|e| error(&e))?;
    }
    Ok(archive.finish().map_err(|e| error(&e))?.into_inner())
}

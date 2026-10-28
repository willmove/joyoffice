//! Format recognition and stream preservation for MS-DOC and WPS files (T602, T605).

use joyoffice_cfb::CompoundFile;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Doc,
    Dot,
    Wps,
    Wpt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservedStream {
    pub name: String,
    pub size: usize,
    pub sha256: [u8; 32],
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocMetadata {
    pub format: DocumentFormat,
    pub is_template: bool,
    pub preserved_streams: BTreeMap<String, PreservedStream>,
}

pub fn detect_format_and_preserve_streams(cfb: &CompoundFile, is_template: bool) -> DocMetadata {
    let mut is_wps = false;
    let mut preserved_streams = BTreeMap::new();

    for (name, _) in cfb.list_streams() {
        if name.starts_with("Wps")
            || name.starts_with("wps")
            || name.starts_with("KSO")
            || name.starts_with("kso")
            || name == "KSOTree"
            || name == "KsoProductInfo"
        {
            is_wps = true;
        }

        // Preserve all non-DOC core streams or private streams
        if name != "WordDocument"
            && name != "0Table"
            && name != "1Table"
            && name != "Data"
            && let Ok(data) = cfb.read_stream(&name)
        {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let sha256 = hasher.finalize().into();
            preserved_streams.insert(
                name.clone(),
                PreservedStream {
                    name: name.clone(),
                    size: data.len(),
                    sha256,
                    data,
                },
            );
        }
    }

    let format = match (is_wps, is_template) {
        (true, true) => DocumentFormat::Wpt,
        (true, false) => DocumentFormat::Wps,
        (false, true) => DocumentFormat::Dot,
        (false, false) => DocumentFormat::Doc,
    };

    DocMetadata {
        format,
        is_template,
        preserved_streams,
    }
}

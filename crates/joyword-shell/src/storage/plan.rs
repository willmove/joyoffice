//! Narrow adapter into JoyOffice's verified, version-bound storage pipelines.
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentVersion(pub(crate) u64);
impl DocumentVersion {
    pub fn get(self) -> u64 {
        self.0
    }
}
impl std::fmt::Display for DocumentVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportTarget {
    Docx,
    Dotx,
    Docm,
    Dotm,
}
pub struct ExportPlan {
    bytes: Vec<u8>,
    origin: Option<Arc<[u8]>>,
    version: DocumentVersion,
}
impl ExportPlan {
    pub fn build(
        doc: &joyword_engine::doc::Document,
        version: u64,
        origin: Option<Arc<[u8]>>,
    ) -> Result<Self, String> {
        let bytes = joyword_docx::write(doc).map_err(|e| e.to_string())?;
        joyoffice_opc::Package::read(&bytes, &Default::default()).map_err(|e| e.to_string())?;
        let back = joyword_docx::read(&bytes).map_err(|e| e.to_string())?;
        if back.plain_text(joyword_engine::doc::StoryRef::Body)
            != doc.plain_text(joyword_engine::doc::StoryRef::Body)
        {
            return Err("DOCX export failed text read-back verification".into());
        }
        Ok(Self {
            bytes,
            origin,
            version: DocumentVersion(version),
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn origin_bytes(&self) -> Option<&[u8]> {
        self.origin.as_deref()
    }
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    pub fn target(&self) -> ExportTarget {
        ExportTarget::Docx
    }
}

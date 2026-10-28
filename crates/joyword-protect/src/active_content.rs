//! Active content security audit, external links policy, macro preservation, and spoofing detection (QUALITY-009).

use joyoffice_opc::{ExternalKind, Package, Relationship, Target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveContentKind {
    VbaMacro,
    ActiveXControl,
    OleObject,
    ExternalTemplate(String),
    ExternalField(String),
    SuspiciousExternalLink(String),
    UpdateFieldsEnabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatSpoofRisk {
    ExecutableFile(String),
    MismatchedContainer {
        extension: String,
        detected_format: &'static str,
    },
    Valid,
}

pub struct ActiveContentAuditor;

impl ActiveContentAuditor {
    /// Detect format spoofing by inspecting magic bytes vs file extension.
    pub fn check_extension_spoofing(extension: &str, bytes: &[u8]) -> FormatSpoofRisk {
        if bytes.len() >= 2 && bytes[0] == b'M' && bytes[1] == b'Z' {
            return FormatSpoofRisk::ExecutableFile(
                "PE executable disguised as document".to_string(),
            );
        }
        if bytes.len() >= 4 && bytes[0..4] == [0x7f, b'E', b'L', b'F'] {
            return FormatSpoofRisk::ExecutableFile(
                "ELF executable disguised as document".to_string(),
            );
        }

        let is_zip = bytes.len() >= 4 && bytes[0..4] == [0x50, 0x4B, 0x03, 0x04];
        let is_cfb =
            bytes.len() >= 8 && bytes[0..8] == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

        let ext = extension.trim_start_matches('.').to_lowercase();
        match ext.as_str() {
            "docx" | "docm" | "dotx" | "dotm" if is_cfb => {
                return FormatSpoofRisk::MismatchedContainer {
                    extension: ext,
                    detected_format: "Compound File Binary (legacy DOC/WPS)",
                };
            }
            "doc" | "wps" | "wpt" if is_zip => {
                return FormatSpoofRisk::MismatchedContainer {
                    extension: ext,
                    detected_format: "ZIP/OOXML (DOCX)",
                };
            }
            _ => {}
        }

        FormatSpoofRisk::Valid
    }

    /// Audit all parts and relationships in an OPC package for active content and external risks.
    pub fn audit_opc_package(package: &Package) -> Vec<ActiveContentKind> {
        let mut findings = Vec::new();

        // 1. Check part names for macros and embedded active objects
        for part in package.parts() {
            let path = part.name.as_str().to_lowercase();
            if path.contains("vbaproject.bin") || path.contains("vbadirection") {
                findings.push(ActiveContentKind::VbaMacro);
            } else if path.contains("activex") {
                findings.push(ActiveContentKind::ActiveXControl);
            } else if path.contains("oleobject") {
                findings.push(ActiveContentKind::OleObject);
            }
        }

        // 2. Audit all relationships for external targets
        for rel in package.relationships(None) {
            Self::audit_relationship(rel, &mut findings);
        }

        for part in package.parts() {
            for rel in package.relationships(Some(&part.name)) {
                Self::audit_relationship(rel, &mut findings);
            }
        }

        findings
    }

    fn audit_relationship(rel: &Relationship, findings: &mut Vec<ActiveContentKind>) {
        if let Target::External { ref raw, ref kind } = rel.target {
            let uri_str = raw.as_ref();
            if *kind == ExternalKind::NetworkShare {
                findings.push(ActiveContentKind::SuspiciousExternalLink(format!(
                    "network share/UNC blocked: {uri_str}"
                )));
            } else if *kind == ExternalKind::File {
                findings.push(ActiveContentKind::SuspiciousExternalLink(format!(
                    "local file blocked: {uri_str}"
                )));
            }

            if rel.kind.contains("attachedTemplate") {
                findings.push(ActiveContentKind::ExternalTemplate(uri_str.to_string()));
            }
        }
    }
}

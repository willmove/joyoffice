//! Reading an OPC package: parts with raw bytes, content types and
//! relationships (ECMA-376 Part 2). Fatal problems — unsafe containers,
//! invalid or duplicate part names, unreadable package XML — reject the
//! package. Recoverable ones are diagnostics so the format layer can decide
//! between editing, read-only opening and refusal.
//!
//! External relationship targets are recorded and classified, never resolved
//! or accessed: opening a document must not touch the network or file
//! shares (QUALITY-005; ARCHITECTURE §5).

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::budget::{Budget, BudgetError};
use crate::part::{PartName, PartNameError, resolve};
use crate::xml::{XmlError, attribute, flat_children};
use crate::zip::{Method, ZipArchive, ZipError};

pub const CONTENT_TYPES_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/content-types";
pub const RELATIONSHIPS_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";
const CONTENT_TYPES_ITEM: &str = "[Content_Types].xml";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpcError {
    Zip(ZipError),
    PartName {
        name: String,
        error: PartNameError,
    },
    /// Two parts are equivalent under ASCII case folding.
    DuplicatePart {
        name: String,
    },
    /// A part name is the directory of another part.
    PrefixPart {
        name: String,
    },
    MissingContentTypes,
    Xml {
        part: String,
        error: XmlError,
    },
    InvalidContentTypes {
        reason: String,
    },
    DuplicateRelationshipId {
        part: String,
        id: String,
    },
    InvalidRelationship {
        part: String,
        reason: String,
    },
}

impl From<ZipError> for OpcError {
    fn from(error: ZipError) -> Self {
        OpcError::Zip(error)
    }
}

impl From<BudgetError> for OpcError {
    fn from(error: BudgetError) -> Self {
        OpcError::Zip(ZipError::Budget(error))
    }
}

impl fmt::Display for OpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpcError::Zip(ZipError::Budget(error)) => write!(f, "resource budget: {error}"),
            OpcError::Zip(error) => write!(f, "archive: {error:?}"),
            OpcError::PartName { name, error } => write!(f, "part name {name:?}: {error}"),
            OpcError::DuplicatePart { name } => write!(f, "duplicate part {name}"),
            OpcError::PrefixPart { name } => write!(f, "part {name} is also a directory"),
            OpcError::MissingContentTypes => f.write_str("[Content_Types].xml is missing"),
            OpcError::Xml { part, error } => write!(f, "{part}: {error:?}"),
            OpcError::InvalidContentTypes { reason } => write!(f, "content types: {reason}"),
            OpcError::DuplicateRelationshipId { part, id } => {
                write!(f, "{part}: relationship id {id} used twice")
            }
            OpcError::InvalidRelationship { part, reason } => write!(f, "{part}: {reason}"),
        }
    }
}

impl std::error::Error for OpcError {}

/// Recoverable findings, reported with the package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    SkippedDirectoryItem {
        name: String,
    },
    MissingContentType {
        part: String,
    },
    /// Relationships part whose source part does not exist.
    OrphanRelationships {
        part: String,
    },
    /// Internal target that is not a part of this package.
    DanglingTarget {
        source: String,
        id: String,
        target: String,
    },
    /// Internal target that is not a valid part name or leaves the package.
    InvalidTarget {
        source: String,
        id: String,
        target: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub name: PartName,
    pub content_type: Option<Arc<str>>,
    /// Exact uncompressed bytes, kept for preservation on save.
    pub data: Arc<[u8]>,
    pub method: Method,
}

/// How an external target would be reached; nothing here reaches it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalKind {
    Web,
    /// `file:` URI or a relative or absolute local path.
    File,
    /// UNC or SMB path; merely resolving one on Windows can leak credentials.
    NetworkShare,
    Mail,
    Other,
}

impl ExternalKind {
    pub fn classify(target: &str) -> Self {
        let lower = target.trim().to_ascii_lowercase();
        if lower.starts_with("\\\\") || lower.starts_with("//") || lower.starts_with("smb:") {
            ExternalKind::NetworkShare
        } else if lower.starts_with("file:") {
            // file://host/share is a network share in disguise.
            let rest = lower.trim_start_matches("file:");
            if rest.starts_with("////") || (rest.starts_with("//") && !rest.starts_with("///")) {
                ExternalKind::NetworkShare
            } else {
                ExternalKind::File
            }
        } else if lower.starts_with("http://") || lower.starts_with("https://") {
            ExternalKind::Web
        } else if lower.starts_with("mailto:") {
            ExternalKind::Mail
        } else if lower.contains(':') && !lower.chars().nth(1).is_some_and(|c| c == ':') {
            ExternalKind::Other
        } else {
            ExternalKind::File
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Internal(PartName),
    External {
        raw: Arc<str>,
        kind: ExternalKind,
    },
    /// Internal target that could not be resolved; see the diagnostics.
    Unresolved(Arc<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relationship {
    pub id: Arc<str>,
    pub kind: Arc<str>,
    pub target: Target,
}

#[derive(Clone, Debug)]
pub struct Package {
    parts: BTreeMap<String, Part>,
    /// Keyed by source part key; `None` is the package itself.
    relationships: BTreeMap<Option<String>, Vec<Relationship>>,
    content_types: Arc<[u8]>,
    diagnostics: Vec<Diagnostic>,
}

struct ContentTypes {
    defaults: BTreeMap<String, Arc<str>>,
    overrides: BTreeMap<String, Arc<str>>,
}

fn content_types(bytes: &[u8], budget: &Budget) -> Result<ContentTypes, OpcError> {
    let children = flat_children(
        CONTENT_TYPES_ITEM,
        bytes,
        budget,
        CONTENT_TYPES_NAMESPACE,
        "Types",
    )
    .map_err(|error| OpcError::Xml {
        part: CONTENT_TYPES_ITEM.into(),
        error,
    })?;
    let invalid = |reason: &str| OpcError::InvalidContentTypes {
        reason: reason.into(),
    };
    let mut types = ContentTypes {
        defaults: BTreeMap::new(),
        overrides: BTreeMap::new(),
    };
    for child in &children {
        let content_type: Arc<str> = attribute(child, "ContentType")
            .ok_or_else(|| invalid("missing ContentType"))?
            .into();
        match child.local.as_str() {
            "Default" => {
                let extension = attribute(child, "Extension")
                    .ok_or_else(|| invalid("Default without Extension"))?
                    .to_ascii_lowercase();
                if types.defaults.insert(extension, content_type).is_some() {
                    return Err(invalid("duplicate Default extension"));
                }
            }
            "Override" => {
                let name = attribute(child, "PartName")
                    .ok_or_else(|| invalid("Override without PartName"))?;
                let name = PartName::new(name).map_err(|_| invalid("bad Override PartName"))?;
                if types.overrides.insert(name.key(), content_type).is_some() {
                    return Err(invalid("duplicate Override"));
                }
            }
            _ => {}
        }
    }
    Ok(types)
}

fn relationships(
    part: &PartName,
    source: Option<&PartName>,
    bytes: &[u8],
    budget: &Budget,
    keys: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<Relationship>, OpcError> {
    let children = flat_children(
        part.as_str(),
        bytes,
        budget,
        RELATIONSHIPS_NAMESPACE,
        "Relationships",
    )
    .map_err(|error| OpcError::Xml {
        part: part.to_string(),
        error,
    })?;
    if children.len() > budget.max_relationships {
        return Err(BudgetError::Relationships {
            part: part.to_string(),
            limit: budget.max_relationships,
        }
        .into());
    }
    let invalid = |reason: &str| OpcError::InvalidRelationship {
        part: part.to_string(),
        reason: reason.into(),
    };
    let base = source.map_or("/", PartName::directory);
    let source_name = source.map_or_else(|| "/".to_owned(), ToString::to_string);
    let mut ids = HashSet::new();
    let mut out = Vec::new();
    for child in children
        .iter()
        .filter(|child| child.local == "Relationship")
    {
        let id = attribute(child, "Id")
            .filter(|id| !id.is_empty())
            .ok_or_else(|| invalid("missing Id"))?;
        let kind = attribute(child, "Type").ok_or_else(|| invalid("missing Type"))?;
        let raw = attribute(child, "Target").ok_or_else(|| invalid("missing Target"))?;
        if !ids.insert(id.to_owned()) {
            return Err(OpcError::DuplicateRelationshipId {
                part: part.to_string(),
                id: id.to_owned(),
            });
        }
        let target = match attribute(child, "TargetMode").unwrap_or("Internal") {
            "External" => Target::External {
                raw: raw.into(),
                kind: ExternalKind::classify(raw),
            },
            "Internal" => match resolve(base, raw).map(|path| PartName::new(&path)) {
                Some(Ok(name)) => {
                    if !keys.contains(&name.key()) {
                        diagnostics.push(Diagnostic::DanglingTarget {
                            source: source_name.clone(),
                            id: id.to_owned(),
                            target: raw.to_owned(),
                        });
                    }
                    Target::Internal(name)
                }
                _ => {
                    diagnostics.push(Diagnostic::InvalidTarget {
                        source: source_name.clone(),
                        id: id.to_owned(),
                        target: raw.to_owned(),
                    });
                    Target::Unresolved(raw.into())
                }
            },
            _ => return Err(invalid("unknown TargetMode")),
        };
        out.push(Relationship {
            id: id.into(),
            kind: kind.into(),
            target,
        });
    }
    Ok(out)
}

impl Package {
    /// Reads and validates a package; see the module documentation.
    pub fn read(bytes: &[u8], budget: &Budget) -> Result<Package, OpcError> {
        let archive = ZipArchive::new(bytes, budget)?;
        let mut diagnostics = Vec::new();
        let mut names: Vec<(usize, PartName)> = Vec::new();
        let mut content_types_item = None;
        let mut keys = HashSet::new();
        for (index, entry) in archive.entries().iter().enumerate() {
            if entry.name.eq_ignore_ascii_case(CONTENT_TYPES_ITEM) {
                if content_types_item.replace(index).is_some() {
                    return Err(OpcError::DuplicatePart {
                        name: entry.name.clone(),
                    });
                }
                continue;
            }
            if entry.name.ends_with('/') && entry.uncompressed_size == 0 {
                diagnostics.push(Diagnostic::SkippedDirectoryItem {
                    name: entry.name.clone(),
                });
                continue;
            }
            let name =
                PartName::from_item_name(&entry.name).map_err(|error| OpcError::PartName {
                    name: entry.name.clone(),
                    error,
                })?;
            if !keys.insert(name.key()) {
                return Err(OpcError::DuplicatePart {
                    name: entry.name.clone(),
                });
            }
            names.push((index, name));
        }
        for (_, name) in &names {
            if let Some(prefix) = name.ancestor_keys().find(|key| keys.contains(key)) {
                return Err(OpcError::PrefixPart { name: prefix });
            }
        }

        let content_types_bytes: Arc<[u8]> = archive
            .read(content_types_item.ok_or(OpcError::MissingContentTypes)?)?
            .into();
        let types = content_types(&content_types_bytes, budget)?;

        let mut parts = BTreeMap::new();
        for (index, name) in names {
            let data: Arc<[u8]> = archive.read(index)?.into();
            let content_type = types.overrides.get(&name.key()).cloned().or_else(|| {
                name.extension()
                    .and_then(|extension| types.defaults.get(&extension).cloned())
            });
            if content_type.is_none() {
                diagnostics.push(Diagnostic::MissingContentType {
                    part: name.to_string(),
                });
            }
            parts.insert(
                name.key(),
                Part {
                    name,
                    content_type,
                    data,
                    method: archive.entries()[index].method,
                },
            );
        }

        let mut relationships_map = BTreeMap::new();
        for part in parts.values() {
            let Some(source) = part.name.relationships_source() else {
                continue;
            };
            if let Some(source) = &source
                && !keys.contains(&source.key())
            {
                diagnostics.push(Diagnostic::OrphanRelationships {
                    part: part.name.to_string(),
                });
                continue;
            }
            let list = relationships(
                &part.name,
                source.as_ref(),
                &part.data,
                budget,
                &keys,
                &mut diagnostics,
            )?;
            relationships_map.insert(source.map(|source| source.key()), list);
        }

        Ok(Package {
            parts,
            relationships: relationships_map,
            content_types: content_types_bytes,
            diagnostics,
        })
    }

    /// Part by name, compared under ASCII case folding.
    pub fn part(&self, name: &str) -> Option<&Part> {
        self.parts.get(&name.to_ascii_lowercase())
    }

    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        self.parts.values()
    }

    /// Relationships of a part, or of the package when `source` is `None`.
    pub fn relationships(&self, source: Option<&PartName>) -> &[Relationship] {
        self.relationships
            .get(&source.map(PartName::key))
            .map_or(&[], Vec::as_slice)
    }

    pub fn relationships_of_type<'a>(
        &'a self,
        source: Option<&PartName>,
        kind: &'a str,
    ) -> impl Iterator<Item = &'a Relationship> + 'a {
        self.relationships(source)
            .iter()
            .filter(move |relationship| &*relationship.kind == kind)
    }

    /// Exact bytes of `[Content_Types].xml`, kept for preservation.
    pub fn content_types_xml(&self) -> &[u8] {
        &self.content_types
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

//! Open Packaging Conventions layer shared by JoyOffice formats (ADR-011;
//! first used by JoyWord's T103 DOCX reader).
//!
//! Reads ZIP-based OPC packages from untrusted bytes under explicit
//! resource budgets (QUALITY-005): a strict ZIP reader, part-name validation,
//! content types and relationships. Every part's exact bytes are kept so the
//! format layer can preserve what it does not interpret. No Word-specific
//! knowledge lives here.

mod budget;
mod package;
mod part;
mod xml;
mod zip;

pub use budget::{Budget, BudgetError};
pub use package::{
    CONTENT_TYPES_NAMESPACE, Diagnostic, ExternalKind, OpcError, Package, Part,
    RELATIONSHIPS_NAMESPACE, Relationship, Target,
};
pub use part::{PartName, PartNameError};
pub use xml::XmlError;
pub use zip::{Method, ZipArchive, ZipEntry, ZipError, write_archive};

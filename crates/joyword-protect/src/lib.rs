//! Document protection, active content safety, digital signatures, and OOXML encryption (T604).

pub mod active_content;
pub mod crypto;
pub mod encrypted_package;
pub mod protection;
pub mod signature;

pub use active_content::{ActiveContentAuditor, ActiveContentKind, FormatSpoofRisk};
pub use encrypted_package::{EncryptedPackageReader, EncryptionError};
pub use protection::{DocumentProtection, ProtectionMode};
pub use signature::{SignatureStatus, SignatureVerifier};

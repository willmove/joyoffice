//! Per-file metadata observation for original replacement (T110, SPEC §5).
//!
//! Extended attributes and provenance marks (macOS quarantine, Windows
//! Zone.Identifier) are read and, when small enough, copied with the `xattr`
//! crate. Access control lists are observed and not rewritten: an empty ACL
//! is not a loss; a present ACL is listed as unpreservable. A syscall failure
//! is an observation failure and refuses replacement. A platform with no API
//! reports the item as unobservable instead of guessing that it is absent.
//!
//! Linux `security.*` (SELinux, Smack, KYSEC `security.ksip` /
//! `security.boxadm`) is applied by the kernel to the new inode. Those names
//! are recorded and are not part of the preservable set: user space often
//! cannot write them back, and treating that `EPERM` as
//! `UnpreservableMetadata(ExtendedAttributes)` made every ordinary file
//! unreplacable. User-namespace attributes stay in the restore set.
//!
//! Windows observes creation time, link count and `Zone.Identifier`; the
//! publish path delegates their preservation to `ReplaceFileW` and verifies
//! the observable values after replacement. Ownership, ACLs and other named
//! streams remain in the explicit confirmation list. The provenance stream is
//! read through a length cap so a huge stream cannot exhaust memory.
//!
//! Precise ACL and alternate-data-stream observation (T110 Windows gap):
//! `winsafe` 0.0.19 with the `kernel` feature does not surface
//! `GetFileSecurityW` / `GetSecurityInfo` /
//! `ConvertSecurityDescriptorToStringSecurityDescriptorW`, so the raw SDDL and
//! its DACL flags (`D:` vs `D:AI` vs `D:PAI`) cannot be read with an approved
//! dependency, and it does not surface `FindFirstStreamW` /
//! `GetFileInformationByHandleEx(FileStreamInfo)`, so streams other than the
//! one we name explicitly cannot be enumerated. Adding a dependency or calling
//! those through `unsafe` FFI is out of scope for this slice
//! (`unsafe_code = forbid`). Empirical evidence on the reference Windows
//! machine is that `ReplaceFileW` preserves the security descriptor
//! byte-for-byte (identical `Get-Acl .Sddl` before and after, for both an
//! inherited `D:AI` and a protected `D:PAI` DACL) and preserves every named
//! stream (`:$DATA`, a custom stream and `Zone.Identifier` all survive). We
//! therefore do NOT claim a `D:`→`D:AI` regression is inherent to
//! `ReplaceFileW`; but because the code cannot observe the descriptor or the
//! stream list, silent loss could not be detected in-process. Per the FORMAT-004
//! contract both items stay `Unobservable`, so they are listed before
//! replacement and only an explicit confirmation proceeds. The `Zone.Identifier`
//! preservation is still verified after publish; other streams are not.
//!
//! Volume capability (T110 Windows gap): `ReplaceFileW` atomic-replace
//! semantics, the hard-link backup and the named provenance stream all assume
//! an NTFS/ReFS local volume. `GetVolumeInformationW` and `GetDriveTypeW` are
//! exposed by `winsafe` 0.0.19, so [`observe_volume`] classifies the source's
//! volume and [`classify_volume`] refuses replacement on FAT/exFAT/other
//! filesystems, network shares and volumes lacking persistent ACLs, named
//! streams or hard links. A refusal surfaces as an observation failure, which
//! `save_original` rejects even under an explicit confirmation; the default
//! copy path is unaffected.

use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::Path;

use super::error::{MetadataItem, ReplacementBlocker};

#[cfg(any(unix, windows))]
const MAX_ATTR: usize = 1 << 20;
#[cfg(unix)]
const MAX_TOTAL: usize = 4 << 20;

#[derive(Clone, Debug)]
pub(crate) struct ObservedMetadata {
    pub losses: Vec<ReplacementBlocker>,
    /// Attributes the publish path must copy onto the replacement.
    pub xattrs: Vec<(OsString, Vec<u8>)>,
    /// Linux `security.*` names. Observed, not restored, and not a blocker.
    pub kernel_xattrs: Vec<OsString>,
}

pub(crate) fn observe(path: &Path) -> Result<ObservedMetadata, io::Error> {
    observe_platform(path)
}

#[cfg(unix)]
fn observe_platform(path: &Path) -> Result<ObservedMetadata, io::Error> {
    observe_unix(path)
}

/// Windows observes the source's creation time and named provenance stream.
/// `ReplaceFileW` preserves both; the stream is still bounded and retained so
/// the publish path can verify that contract instead of merely assuming it.
/// Ownership and unnamed streams remain explicitly unobservable.
///
/// The source's volume is classified first: on a filesystem, drive type or
/// capability set that `ReplaceFileW` cannot honour, observation fails so the
/// replacement is refused before any write (see [`classify_volume`]).
#[cfg(windows)]
fn observe_platform(path: &Path) -> Result<ObservedMetadata, io::Error> {
    use std::io::{ErrorKind, Read};

    observe_volume(path).and_then(|facts| classify_volume(&facts).map_err(volume_rejected))?;
    std::fs::metadata(path)?.created()?;
    let mut losses = vec![
        ReplacementBlocker::UnobservableMetadata(MetadataItem::Ownership),
        ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl),
        ReplacementBlocker::UnobservableMetadata(MetadataItem::ExtendedAttributes),
    ];
    let mut xattrs = Vec::new();
    let ads_string = format!("{}:Zone.Identifier", path.display());
    let ads = Path::new(&ads_string);
    match std::fs::File::open(ads) {
        Ok(file) => {
            let mut capped = file.take(MAX_ATTR as u64 + 1);
            let mut value = Vec::new();
            capped.read_to_end(&mut value)?;
            if value.len() > MAX_ATTR {
                push_loss(
                    &mut losses,
                    ReplacementBlocker::UnpreservableMetadata(MetadataItem::ProvenanceMarks),
                );
            } else {
                xattrs.push((OsString::from("Zone.Identifier"), value));
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(ObservedMetadata {
        losses,
        xattrs,
        kernel_xattrs: Vec::new(),
    })
}

/// Raw volume facts observed for a source path, kept as primitives so the
/// classification below is testable with synthetic inputs and does not depend
/// on `winsafe` types.
#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VolumeFacts {
    /// Filesystem name reported by `GetVolumeInformationW` (e.g. `NTFS`).
    pub file_system: String,
    /// Raw `GetDriveTypeW` return value.
    pub drive_type: u32,
    /// Raw `GetVolumeInformationW` filesystem-flags bitmask (`FILE_VOL`).
    pub flags: u32,
}

/// Why a volume cannot carry the original-replacement contract. Each variant
/// maps to one observable volume fact so the classification is auditable.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VolumeRejection {
    /// `GetDriveTypeW` reported `DRIVE_REMOTE`: a network share whose atomic
    /// replace, hard-link backup and stream semantics are not verified here.
    Remote,
    /// `GetDriveTypeW` could not classify the volume (`DRIVE_UNKNOWN` /
    /// `DRIVE_NO_ROOT_DIR`).
    UnknownDriveType,
    /// The filesystem is not one `ReplaceFileW` atomic replace is verified on
    /// (NTFS or ReFS). FAT/exFAT lack ACLs, named streams or hard links.
    UnsupportedFileSystem,
    /// The volume does not persist ACLs (`FILE_PERSISTENT_ACLS` clear).
    NoPersistentAcls,
    /// The volume has no named streams (`FILE_NAMED_STREAMS` clear), so the
    /// `Zone.Identifier` provenance stream could not be carried.
    NoNamedStreams,
    /// The volume does not support hard links (`FILE_SUPPORTS_HARD_LINKS`
    /// clear), so the pre-replace backup link could not be created.
    NoHardLinks,
}

#[cfg(windows)]
impl VolumeRejection {
    fn message(self) -> &'static str {
        match self {
            Self::Remote => {
                "source is on a network volume; original replacement is not verified there"
            }
            Self::UnknownDriveType => "the source volume type could not be determined",
            Self::UnsupportedFileSystem => {
                "the source filesystem is not NTFS/ReFS; original replacement is not verified there"
            }
            Self::NoPersistentAcls => "the source volume does not persist access control lists",
            Self::NoNamedStreams => "the source volume does not support named streams",
            Self::NoHardLinks => "the source volume does not support hard links",
        }
    }
}

// `GetDriveTypeW` return values (winsafe `co::DRIVE`).
#[cfg(windows)]
const DRIVE_UNKNOWN: u32 = 0;
#[cfg(windows)]
const DRIVE_NO_ROOT_DIR: u32 = 1;
#[cfg(windows)]
const DRIVE_REMOTE: u32 = 4;
// `GetVolumeInformationW` filesystem flags (winsafe `co::FILE_VOL`).
#[cfg(windows)]
const FILE_VOL_PERSISTENT_ACLS: u32 = 0x0000_0008;
#[cfg(windows)]
const FILE_VOL_NAMED_STREAMS: u32 = 0x0004_0000;
#[cfg(windows)]
const FILE_VOL_SUPPORTS_HARD_LINKS: u32 = 0x0040_0000;

/// Decide whether `facts` describe a volume the original-replacement contract
/// can honour. Pure and dependency-free so every branch is unit-testable with
/// synthetic inputs; a network share and a non-NTFS/ReFS filesystem are refused
/// first, then each required capability flag.
#[cfg(windows)]
pub(crate) fn classify_volume(facts: &VolumeFacts) -> Result<(), VolumeRejection> {
    match facts.drive_type {
        DRIVE_REMOTE => return Err(VolumeRejection::Remote),
        DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR => return Err(VolumeRejection::UnknownDriveType),
        _ => {}
    }
    let fs = facts.file_system.to_ascii_lowercase();
    if fs != "ntfs" && fs != "refs" {
        return Err(VolumeRejection::UnsupportedFileSystem);
    }
    if facts.flags & FILE_VOL_PERSISTENT_ACLS == 0 {
        return Err(VolumeRejection::NoPersistentAcls);
    }
    if facts.flags & FILE_VOL_NAMED_STREAMS == 0 {
        return Err(VolumeRejection::NoNamedStreams);
    }
    if facts.flags & FILE_VOL_SUPPORTS_HARD_LINKS == 0 {
        return Err(VolumeRejection::NoHardLinks);
    }
    Ok(())
}

/// Turn a volume refusal into the observation failure `save_original` reports;
/// the message names the concrete volume fact so the diagnostic is locatable.
#[cfg(windows)]
fn volume_rejected(rejection: VolumeRejection) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, rejection.message())
}

/// Observe the volume a source path lives on through `GetVolumePathNameW`,
/// `GetVolumeInformationW` and `GetDriveTypeW` (all in `winsafe` 0.0.19).
#[cfg(windows)]
pub(crate) fn observe_volume(path: &Path) -> io::Result<VolumeFacts> {
    let name = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Win32 path is not valid Unicode",
        )
    })?;
    let root = winsafe::GetVolumePathName(name)
        .map_err(|e| io::Error::from_raw_os_error(e.raw() as i32))?;
    let mut file_system = String::new();
    let mut flags = winsafe::co::FILE_VOL::default();
    winsafe::GetVolumeInformation(
        Some(&root),
        None,
        None,
        None,
        Some(&mut flags),
        Some(&mut file_system),
    )
    .map_err(|e| io::Error::from_raw_os_error(e.raw() as i32))?;
    let drive_type = winsafe::GetDriveType(Some(&root)).raw();
    Ok(VolumeFacts {
        file_system,
        drive_type,
        flags: flags.raw(),
    })
}

/// Other targets have no reviewed observation API: prove the source exists
/// and report every item as unobservable instead of guessing it is absent.
#[cfg(all(not(unix), not(windows)))]
fn observe_platform(path: &Path) -> Result<ObservedMetadata, io::Error> {
    std::fs::metadata(path)?;
    Ok(ObservedMetadata {
        losses: vec![
            ReplacementBlocker::UnobservableMetadata(MetadataItem::Ownership),
            ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl),
            ReplacementBlocker::UnobservableMetadata(MetadataItem::ExtendedAttributes),
            ReplacementBlocker::UnobservableMetadata(MetadataItem::CreationTime),
            ReplacementBlocker::UnobservableMetadata(MetadataItem::Links),
            ReplacementBlocker::UnobservableMetadata(MetadataItem::ProvenanceMarks),
        ],
        xattrs: Vec::new(),
        kernel_xattrs: Vec::new(),
    })
}

#[cfg(unix)]
fn observe_unix(path: &Path) -> Result<ObservedMetadata, io::Error> {
    let mut losses = Vec::new();
    let mut xattrs = Vec::new();
    let mut kernel_xattrs = Vec::new();
    let mut total = 0usize;
    let mut xattr_loss = false;
    let mut acl_from_xattr = false;
    for name in xattr::list(path)? {
        if is_acl_name(&name) {
            acl_from_xattr = true;
            continue;
        }
        if is_kernel_security_xattr(&name) {
            // Do not read or copy. `get`/`set` of KYSEC and SELinux labels
            // fails with EPERM for ordinary processes, which would refuse
            // replacement of every file on those systems.
            kernel_xattrs.push(name);
            continue;
        }
        let provenance = is_provenance(&name);
        match xattr::get(path, &name) {
            Ok(Some(value))
                if value.len() <= MAX_ATTR && total.saturating_add(value.len()) <= MAX_TOTAL =>
            {
                total += value.len();
                xattrs.push((name, value));
            }
            Ok(_) => {
                if provenance {
                    push_loss(
                        &mut losses,
                        ReplacementBlocker::UnpreservableMetadata(MetadataItem::ProvenanceMarks),
                    );
                } else {
                    xattr_loss = true;
                }
            }
            Err(error) if missing(&error) => {}
            Err(error) => return Err(error),
        }
    }
    if xattr_loss {
        push_loss(
            &mut losses,
            ReplacementBlocker::UnpreservableMetadata(MetadataItem::ExtendedAttributes),
        );
    }
    if acl_from_xattr || matches!(acl_present(path)?, AclSeen::Present) {
        push_loss(
            &mut losses,
            ReplacementBlocker::UnpreservableMetadata(MetadataItem::Acl),
        );
    }
    #[cfg(not(target_os = "macos"))]
    match std::fs::metadata(path).and_then(|metadata| metadata.created()) {
        Ok(_) => push_loss(
            &mut losses,
            ReplacementBlocker::UnpreservableMetadata(MetadataItem::CreationTime),
        ),
        Err(error) if missing(&error) || error.kind() == io::ErrorKind::Unsupported => push_loss(
            &mut losses,
            ReplacementBlocker::UnobservableMetadata(MetadataItem::CreationTime),
        ),
        Err(error) => return Err(error),
    }
    Ok(ObservedMetadata {
        losses,
        xattrs,
        kernel_xattrs,
    })
}

/// `names` contains only kernel-managed security attributes, so dropping
/// them from the restore set cannot hide a user attribute.
pub(crate) fn kernel_xattrs_are_security_namespace(names: &[OsString]) -> bool {
    names.iter().all(|name| is_kernel_security_xattr(name))
}

#[cfg(unix)]
pub(crate) fn restore_xattrs(
    path: &Path,
    xattrs: &[(OsString, Vec<u8>)],
) -> Result<(), MetadataItem> {
    for (name, value) in xattrs {
        if is_kernel_security_xattr(name) {
            continue;
        }
        let item = if is_provenance(name) {
            MetadataItem::ProvenanceMarks
        } else {
            MetadataItem::ExtendedAttributes
        };
        xattr::set(path, name, value).map_err(|_| item)?;
        match xattr::get(path, name) {
            Ok(Some(got)) if got == *value => {}
            _ => return Err(item),
        }
    }
    Ok(())
}

#[cfg(any(unix, windows))]
fn push_loss(losses: &mut Vec<ReplacementBlocker>, blocker: ReplacementBlocker) {
    if !losses.contains(&blocker) {
        losses.push(blocker);
    }
}

#[cfg(unix)]
fn is_provenance(name: &OsStr) -> bool {
    name == OsStr::new("com.apple.quarantine") || name == OsStr::new("Zone.Identifier")
}

#[cfg(unix)]
fn is_acl_name(name: &OsStr) -> bool {
    name == OsStr::new("system.posix_acl_access") || name == OsStr::new("system.posix_acl_default")
}

/// Linux kernel xattr namespace. The prefix is the namespace, so
/// `user.security.note` stays preservable. Other Unix platforms have no
/// such kernel namespace; a literal `security.*` name there is user data.
pub(crate) fn is_kernel_security_xattr(name: &OsStr) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        name.as_bytes().starts_with(b"security.")
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = name;
        false
    }
}

#[cfg(unix)]
fn missing(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum AclSeen {
    Absent,
    Present,
}

#[cfg(unix)]
fn acl_present(path: &Path) -> Result<AclSeen, io::Error> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/bin/ls")
            .args(["-led"])
            .arg(path)
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "acl observation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| io::Error::other("acl observation returned non-utf8"))?;
        let present = text.lines().any(|line| {
            let trimmed = line.trim_start();
            let index = trimmed.split(':').next().unwrap_or("");
            !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
        });
        Ok(if present {
            AclSeen::Present
        } else {
            AclSeen::Absent
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        match xattr::get(path, "system.posix_acl_access") {
            Ok(Some(value)) if !value.is_empty() => Ok(AclSeen::Present),
            Ok(_) => Ok(AclSeen::Absent),
            Err(error) if missing(&error) || error.kind() == io::ErrorKind::Unsupported => {
                Ok(AclSeen::Absent)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_path_is_an_observation_failure() {
        let missing = std::env::temp_dir().join("joyword-missing-metadata.docx");
        let _ = std::fs::remove_file(&missing);
        assert!(observe(&missing).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_observation_reports_only_unobserved_metadata_as_losses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, b"0123456789").unwrap();
        let observed = observe(&path).unwrap();
        for item in [
            MetadataItem::Ownership,
            MetadataItem::Acl,
            MetadataItem::ExtendedAttributes,
        ] {
            assert!(
                observed
                    .losses
                    .contains(&ReplacementBlocker::UnobservableMetadata(item)),
                "{item:?}"
            );
        }
        assert!(!observed.losses.iter().any(|loss| matches!(
            loss,
            ReplacementBlocker::UnobservableMetadata(MetadataItem::CreationTime)
                | ReplacementBlocker::UnpreservableMetadata(MetadataItem::CreationTime)
                | ReplacementBlocker::UnobservableMetadata(MetadataItem::Links)
                | ReplacementBlocker::UnpreservableMetadata(MetadataItem::Links)
        )));
        assert!(observed.xattrs.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn windows_zone_identifier_is_retained_for_publish_verification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, b"0123456789").unwrap();
        let stream = format!("{}:Zone.Identifier", path.display());
        let value = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        std::fs::write(&stream, value).unwrap();
        let observed = observe(&path).unwrap();
        assert!(!observed.losses.iter().any(|loss| matches!(
            loss,
            ReplacementBlocker::UnpreservableMetadata(MetadataItem::ProvenanceMarks)
        )));
        assert_eq!(observed.xattrs.len(), 1);
        assert_eq!(observed.xattrs[0].0, "Zone.Identifier");
        assert_eq!(observed.xattrs[0].1, value);
        std::fs::remove_file(&stream).unwrap();
        let observed = observe(&path).unwrap();
        assert!(observed.xattrs.is_empty());
        assert!(observed.kernel_xattrs.is_empty());
    }

    #[cfg(windows)]
    fn facts(fs: &str, drive_type: u32, flags: u32) -> VolumeFacts {
        VolumeFacts {
            file_system: fs.to_string(),
            drive_type,
            flags,
        }
    }

    #[cfg(windows)]
    const DRIVE_FIXED: u32 = 3;
    #[cfg(windows)]
    const DRIVE_REMOVABLE: u32 = 2;
    #[cfg(windows)]
    const FULL_NTFS: u32 =
        FILE_VOL_PERSISTENT_ACLS | FILE_VOL_NAMED_STREAMS | FILE_VOL_SUPPORTS_HARD_LINKS;

    #[cfg(windows)]
    #[test]
    fn classify_volume_accepts_a_local_ntfs_volume_with_every_required_capability() {
        assert_eq!(
            classify_volume(&facts("NTFS", DRIVE_FIXED, FULL_NTFS)),
            Ok(())
        );
        assert_eq!(
            classify_volume(&facts("ReFS", DRIVE_FIXED, FULL_NTFS)),
            Ok(())
        );
        // A removable local NTFS volume (no network) is still acceptable.
        assert_eq!(
            classify_volume(&facts("ntfs", DRIVE_REMOVABLE, FULL_NTFS)),
            Ok(())
        );
        // Filesystem name matching is case-insensitive; unrelated extra flags
        // (compression, encryption, USN journal) never cause a refusal.
        assert_eq!(
            classify_volume(&facts(
                "NTFS",
                DRIVE_FIXED,
                FULL_NTFS | 0x0000_0010 | 0x0002_0000
            )),
            Ok(())
        );
    }

    #[cfg(windows)]
    #[test]
    fn classify_volume_refuses_network_and_unrecognized_drive_types() {
        // A network share is refused even when it reports full NTFS flags.
        assert_eq!(
            classify_volume(&facts("NTFS", DRIVE_REMOTE, FULL_NTFS)),
            Err(VolumeRejection::Remote)
        );
        assert_eq!(
            classify_volume(&facts("NTFS", DRIVE_UNKNOWN, FULL_NTFS)),
            Err(VolumeRejection::UnknownDriveType)
        );
        assert_eq!(
            classify_volume(&facts("NTFS", DRIVE_NO_ROOT_DIR, FULL_NTFS)),
            Err(VolumeRejection::UnknownDriveType)
        );
    }

    #[cfg(windows)]
    #[test]
    fn classify_volume_refuses_non_ntfs_filesystems() {
        for fs in ["FAT", "FAT32", "exFAT", "", "UDF", "CDFS"] {
            assert_eq!(
                classify_volume(&facts(fs, DRIVE_FIXED, FULL_NTFS)),
                Err(VolumeRejection::UnsupportedFileSystem),
                "{fs}"
            );
        }
        // Drive type is checked before the filesystem, so a remote FAT share is
        // reported as remote, not as an unsupported filesystem.
        assert_eq!(
            classify_volume(&facts("FAT32", DRIVE_REMOTE, FULL_NTFS)),
            Err(VolumeRejection::Remote)
        );
    }

    #[cfg(windows)]
    #[test]
    fn classify_volume_refuses_each_missing_capability_flag() {
        assert_eq!(
            classify_volume(&facts(
                "NTFS",
                DRIVE_FIXED,
                FILE_VOL_NAMED_STREAMS | FILE_VOL_SUPPORTS_HARD_LINKS
            )),
            Err(VolumeRejection::NoPersistentAcls)
        );
        assert_eq!(
            classify_volume(&facts(
                "NTFS",
                DRIVE_FIXED,
                FILE_VOL_PERSISTENT_ACLS | FILE_VOL_SUPPORTS_HARD_LINKS
            )),
            Err(VolumeRejection::NoNamedStreams)
        );
        assert_eq!(
            classify_volume(&facts(
                "NTFS",
                DRIVE_FIXED,
                FILE_VOL_PERSISTENT_ACLS | FILE_VOL_NAMED_STREAMS
            )),
            Err(VolumeRejection::NoHardLinks)
        );
        assert_eq!(
            classify_volume(&facts("NTFS", DRIVE_FIXED, 0)),
            Err(VolumeRejection::NoPersistentAcls)
        );
    }

    /// Real observation of the volume the test's temporary directory lives on.
    /// It must be a supported local volume, so classification succeeds and the
    /// reported filesystem is one `ReplaceFileW` is verified on.
    #[cfg(windows)]
    #[test]
    fn observe_volume_reports_a_supported_volume_for_the_temp_directory() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.docx");
        std::fs::write(&path, b"0123456789")?;
        let observed = observe_volume(&path)?;
        assert_eq!(classify_volume(&observed), Ok(()));
        let fs = observed.file_system.to_ascii_lowercase();
        assert!(fs == "ntfs" || fs == "refs", "{fs}");
        assert_ne!(observed.drive_type, DRIVE_REMOTE);
        assert_ne!(observed.drive_type, DRIVE_UNKNOWN);
        assert_ne!(observed.drive_type, DRIVE_NO_ROOT_DIR);
        // The full observation path (used by save_original) must succeed here.
        let metadata = observe(&path)?;
        assert!(
            metadata
                .losses
                .contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl))
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn volume_rejection_messages_are_distinct_and_non_empty() {
        use std::collections::HashSet;
        let messages: HashSet<&str> = [
            VolumeRejection::Remote,
            VolumeRejection::UnknownDriveType,
            VolumeRejection::UnsupportedFileSystem,
            VolumeRejection::NoPersistentAcls,
            VolumeRejection::NoNamedStreams,
            VolumeRejection::NoHardLinks,
        ]
        .into_iter()
        .map(|rejection| {
            let message = rejection.message();
            assert!(!message.is_empty());
            // The io::Error carries the same locatable text.
            assert_eq!(volume_rejected(rejection).to_string(), message);
            message
        })
        .collect();
        assert_eq!(messages.len(), 6);
    }

    #[test]
    fn security_namespace_is_kernel_managed_only_on_linux() {
        assert_eq!(
            is_kernel_security_xattr(OsStr::new("security.selinux")),
            cfg!(target_os = "linux")
        );
        assert_eq!(
            is_kernel_security_xattr(OsStr::new("security.ksip")),
            cfg!(target_os = "linux")
        );
        assert_eq!(
            is_kernel_security_xattr(OsStr::new("security.boxadm")),
            cfg!(target_os = "linux")
        );
        assert_eq!(
            is_kernel_security_xattr(OsStr::new("security.capability")),
            cfg!(target_os = "linux")
        );
        assert!(!is_kernel_security_xattr(OsStr::new("user.joyword-m1")));
        assert!(!is_kernel_security_xattr(OsStr::new("user.security.note")));
        assert!(!is_kernel_security_xattr(OsStr::new("security")));
        assert!(!is_kernel_security_xattr(OsStr::new(
            "com.apple.quarantine"
        )));
        assert!(kernel_xattrs_are_security_namespace(&[]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_security_prefix_is_kernel_managed() {
        use std::os::unix::ffi::OsStrExt;
        let name = OsStr::from_bytes(b"security.ksip\xff");
        assert!(is_kernel_security_xattr(name));
        assert!(!is_kernel_security_xattr(OsStr::from_bytes(
            b"user.security\xff"
        )));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_security_xattrs_are_observed_and_not_queued_for_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, b"0123456789").unwrap();
        xattr::set(&path, "user.joyword-m1", b"kept").unwrap();
        xattr::set(&path, "user.security.note", b"also").unwrap();
        let marker = b"v";
        let planted = xattr::set(&path, "security.selinux", marker).is_ok();
        let observed = observe(&path).unwrap();
        assert!(
            observed
                .xattrs
                .iter()
                .any(|(name, value)| name == "user.joyword-m1" && value == b"kept")
        );
        assert!(
            observed
                .xattrs
                .iter()
                .any(|(name, value)| name == "user.security.note" && value == b"also")
        );
        assert!(
            observed
                .xattrs
                .iter()
                .all(|(name, _)| !is_kernel_security_xattr(name))
        );
        assert!(kernel_xattrs_are_security_namespace(
            &observed.kernel_xattrs
        ));
        assert!(!observed.losses.iter().any(|loss| matches!(
            loss,
            ReplacementBlocker::UnpreservableMetadata(MetadataItem::ExtendedAttributes)
        )));
        if planted {
            assert!(
                observed
                    .kernel_xattrs
                    .iter()
                    .any(|name| name == "security.selinux")
            );
        }
        if let Ok(listed) = xattr::list(&path) {
            for name in listed {
                if is_kernel_security_xattr(&name) {
                    assert!(
                        observed.kernel_xattrs.iter().any(|got| got == &name),
                        "{name:?}"
                    );
                }
            }
        }
    }
}

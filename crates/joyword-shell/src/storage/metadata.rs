//! Per-file metadata observation for original replacement (T110, SPEC §5).
//!
//! Extended attributes and provenance marks (macOS quarantine, Windows
//! Zone.Identifier) are read and, when small enough, copied with the `xattr`
//! crate. Access control lists are observed and not rewritten: an empty ACL
//! is not a loss; a present ACL is listed as unpreservable. A syscall failure
//! is an observation failure and refuses replacement. A platform with no API
//! reports the item as unobservable instead of guessing that it is absent.
//!
//! Windows observes creation time, link count and `Zone.Identifier`; the
//! publish path delegates their preservation to `ReplaceFileW` and verifies
//! the observable values after replacement. Ownership, ACLs and other named
//! streams remain in the explicit confirmation list. The provenance stream is
//! read through a length cap so a huge stream cannot exhaust memory.

#[cfg(unix)]
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
    pub xattrs: Vec<(OsString, Vec<u8>)>,
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
#[cfg(windows)]
fn observe_platform(path: &Path) -> Result<ObservedMetadata, io::Error> {
    use std::io::{ErrorKind, Read};

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
    Ok(ObservedMetadata { losses, xattrs })
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
    })
}

#[cfg(unix)]
fn observe_unix(path: &Path) -> Result<ObservedMetadata, io::Error> {
    let mut losses = Vec::new();
    let mut xattrs = Vec::new();
    let mut total = 0usize;
    let mut xattr_loss = false;
    let mut acl_from_xattr = false;
    for name in xattr::list(path)? {
        if is_acl_name(&name) {
            acl_from_xattr = true;
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
    Ok(ObservedMetadata { losses, xattrs })
}

#[cfg(unix)]
pub(crate) fn restore_xattrs(
    path: &Path,
    xattrs: &[(OsString, Vec<u8>)],
) -> Result<(), MetadataItem> {
    for (name, value) in xattrs {
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
    }
}

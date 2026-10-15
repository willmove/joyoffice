//! macOS extended metadata that `std` cannot observe: extended attribute
//! names and access control lists.
//!
//! Without new dependencies and with `unsafe` forbidden there is no
//! listxattr or acl call, so this reads the system `ls` on the sealed system
//! volume: `/bin/ls -ld -@ -e` prints a `@` (extended attributes) or `+`
//! (ACL) marker after the mode string, one tab-indented line per attribute
//! name and one ` N: …` line per ACL entry (ls(1), macOS). Anything that does
//! not parse exactly is an error, which callers treat as "unobservable"
//! and refuse. Values are never compared: an item that is present is a
//! reason to refuse, not something this module claims to preserve.
//!
//! The one tolerated attribute is `com.apple.provenance`. The kernel manages
//! it and attaches the writing process's provenance to every file a tracked
//! process creates, including the replacement; user processes cannot carry
//! another writer's value over, so it is reported but not counted.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use super::error::MetadataItem;

/// Kernel-managed provenance attribute; see the module documentation.
const KERNEL_PROVENANCE: &str = "com.apple.provenance";

/// Extended metadata observed on one regular file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Extended {
    /// Extended attribute names, in `ls` order.
    pub xattrs: Vec<String>,
    /// Whether at least one ACL entry is present.
    pub acl: bool,
}

impl Extended {
    /// Metadata items present on the file that a replacement would lose or
    /// change, each listed once.
    pub(crate) fn present_items(&self) -> Vec<MetadataItem> {
        let mut items = Vec::new();
        let mut push = |item| {
            if !items.contains(&item) {
                items.push(item);
            }
        };
        if self.acl {
            push(MetadataItem::Acl);
        }
        for name in &self.xattrs {
            match name.as_str() {
                KERNEL_PROVENANCE => {}
                // Gatekeeper quarantine and the download origin record.
                "com.apple.quarantine" | "com.apple.metadata:kMDItemWhereFroms" => {
                    push(MetadataItem::ProvenanceMarks)
                }
                _ => push(MetadataItem::ExtendedAttributes),
            }
        }
        items
    }
}

/// Observe the extended metadata of the regular file at `path`, which must
/// be absolute and free of control characters (so the listing cannot be
/// split or confused by the file name).
pub(crate) fn observe(path: &Path) -> io::Result<Extended> {
    let unsupported = |what| Err(io::Error::new(io::ErrorKind::Unsupported, what));
    let Some(text) = path.to_str() else {
        return unsupported("path is not valid UTF-8");
    };
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return unsupported("path cannot be listed unambiguously");
    }
    let output = Command::new("/bin/ls")
        .args(["-ld", "-@", "-e", "--"])
        .arg(path)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err(io::Error::other(format!(
            "/bin/ls failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    match String::from_utf8(output.stdout) {
        Ok(listing) => parse(&listing),
        Err(_) => unsupported("listing is not valid UTF-8"),
    }
}

fn parse(listing: &str) -> io::Result<Extended> {
    let invalid = |what| Err(io::Error::new(io::ErrorKind::InvalidData, what));
    let mut lines = listing.lines();
    let Some(first) = lines.next() else {
        return invalid("empty listing");
    };
    let mode = first.as_bytes();
    if mode.first() != Some(&b'-') || mode.len() < 11 {
        return invalid("listing does not describe a regular file");
    }
    let marker = mode[10];
    let mut extended = Extended::default();
    for line in lines {
        if let Some(rest) = line.strip_prefix('\t') {
            let Some((name, _size)) = rest.split_once('\t') else {
                return invalid("malformed attribute line");
            };
            if name.is_empty() || extended.acl {
                return invalid("malformed attribute line");
            }
            extended.xattrs.push(name.to_owned());
        } else if is_acl_entry(line) {
            extended.acl = true;
        } else {
            return invalid("unexpected listing line");
        }
    }
    // The marker and the detail lines must tell the same story.
    let consistent = match marker {
        b'@' => !extended.xattrs.is_empty(),
        b'+' => extended.xattrs.is_empty() && extended.acl,
        b' ' => extended.xattrs.is_empty() && !extended.acl,
        _ => false,
    };
    if !consistent {
        return invalid("listing marker disagrees with its detail lines");
    }
    Ok(extended)
}

/// ` 0: group:everyone deny delete`
fn is_acl_entry(line: &str) -> bool {
    let Some(rest) = line.strip_prefix(' ') else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && rest[digits..].starts_with(": ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markers_attributes_and_acl_entries() {
        let plain = parse("-rw-r--r--  1 u  staff  6 Oct 15 23:22 /a.docx\n").unwrap();
        assert_eq!(plain, Extended::default());
        let both = parse(
            "-rw-r--r--@ 1 u  staff  6 Oct 15 23:22 /a.docx\n\tcom.apple.provenance\t11 \n\tcom.apple.quarantine\t21 \n\tcom.example.k\t1 \n 0: group:everyone deny delete\n",
        )
        .unwrap();
        assert_eq!(
            both.xattrs,
            [
                "com.apple.provenance",
                "com.apple.quarantine",
                "com.example.k"
            ]
        );
        assert!(both.acl);
        assert_eq!(
            both.present_items(),
            [
                MetadataItem::Acl,
                MetadataItem::ProvenanceMarks,
                MetadataItem::ExtendedAttributes
            ]
        );
        let acl = parse("-rw-r--r--+ 1 u  staff  6 Oct 15 23:22 /a.docx\n 0: user:u allow read\n")
            .unwrap();
        assert!(acl.acl && acl.xattrs.is_empty());
    }

    #[test]
    fn kernel_provenance_alone_is_not_a_present_item() {
        let extended =
            parse("-rw-r--r--@ 1 u  staff  6 Oct 15 23:22 /a.docx\n\tcom.apple.provenance\t11 \n")
                .unwrap();
        assert!(extended.present_items().is_empty());
    }

    #[test]
    fn inconsistent_or_unknown_listings_are_errors() {
        for listing in [
            "",
            "drwxr-xr-x  2 u  staff  64 Oct 15 23:22 /d\n",
            "-rw-r--r--@ 1 u  staff  6 Oct 15 23:22 /a.docx\n",
            "-rw-r--r--  1 u  staff  6 Oct 15 23:22 /a.docx\n\tcom.example.k\t1 \n",
            "-rw-r--r--+ 1 u  staff  6 Oct 15 23:22 /a.docx\n",
            "-rw-r--r--  1 u  staff  6 Oct 15 23:22 /a.docx\nsurprise\n",
            "-rw-r--r--@ 1 u  staff  6 Oct 15 23:22 /a.docx\n\tnosize\n",
            "-rw-r--r--x 1 u  staff  6 Oct 15 23:22 /a.docx\n",
        ] {
            assert!(parse(listing).is_err(), "{listing:?}");
        }
    }

    #[test]
    fn real_files_are_observed_with_their_attributes() {
        let dir = tempfile::tempdir().unwrap();
        let path = std::fs::canonicalize(dir.path()).unwrap().join("a.docx");
        std::fs::write(&path, b"x").unwrap();
        assert!(observe(&path).unwrap().present_items().is_empty());
        let status = Command::new("/usr/bin/xattr")
            .args(["-w", "com.example.joyword", "1"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            observe(&path).unwrap().present_items(),
            [MetadataItem::ExtendedAttributes]
        );
        assert!(observe(Path::new("relative.docx")).is_err());
        assert!(observe(&dir.path().join("line\nbreak.docx")).is_err());
    }
}

//! Native font discovery shared by Word and Deck. Product fallback order and shaping remain local.
use sha2::{Digest, Sha256};
use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Identity of a collection face before choosing a variable-font instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileFace {
    pub sha256: [u8; 32],
    pub index: u32,
}

pub fn file_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

impl fmt::LowerHex for FileFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.sha256 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for FileFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:x}/{}", self.index)
    }
}

/// Ordered system and user roots common to both font catalogs. Product extensions follow these.
pub fn system_roots(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(target_arch = "wasm32") {
        return dirs;
    }
    if cfg!(target_os = "macos") {
        dirs.extend(["/System/Library/Fonts", "/Library/Fonts"].map(PathBuf::from));
        dirs.extend(home.map(|h| h.join("Library/Fonts")));
    } else if cfg!(windows) {
        let root = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into());
        dirs.push(root.join("Fonts"));
        dirs.extend(
            std::env::var_os("LOCALAPPDATA")
                .map(|local| PathBuf::from(local).join("Microsoft\\Windows\\Fonts")),
        );
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
        if let Some(h) = home {
            dirs.extend([h.join(".fonts"), h.join(".local/share/fonts")]);
        }
    }
    dirs
}

pub fn macos_asset_roots(include_legacy: bool) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for parent in [
        "/System/Library/AssetsV2",
        "/System/Library/AssetsV2/PreinstalledAssetsV2/InstallWithOs",
    ]
    .into_iter()
    .chain(include_legacy.then_some("/System/Library/Assets"))
    {
        if let Ok(entries) = std::fs::read_dir(parent) {
            dirs.extend(
                entries
                    .flatten()
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with("com_apple_MobileAsset_Font")
                    })
                    .map(|e| e.path()),
            );
        }
    }
    dirs
}

pub fn flatpak_roots(include_legacy: bool) -> impl Iterator<Item = PathBuf> {
    [
        "/run/host/fonts",
        "/run/host/local-fonts",
        "/run/host/user-fonts",
    ]
    .into_iter()
    .chain(include_legacy.then_some("/run/host/user-fonts-legacy"))
    .map(PathBuf::from)
}

/// Filesystem revision used for catalog discovery. Rendering still uses a digest of the
/// actual font bytes and the collection face. On Unix this also detects same-length rewrites
/// that restore mtime and atomic replacements with a different inode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStamp {
    length: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}

impl From<&std::fs::Metadata> for FileStamp {
    fn from(meta: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            length: meta.len(),
            modified: meta.modified().ok(),
            created: meta.created().ok(),
            #[cfg(unix)]
            identity: (meta.dev(), meta.ino(), meta.ctime(), meta.ctime_nsec()),
        }
    }
}

/// Enumerate regular OpenType and TrueType files in the supplied root order (last root first).
/// Directory symlinks are followed at most once. Return false if the caller's entry budget
/// stops this scan. The owner retains its file-size, face and cache policies.
pub fn visit_font_files(
    roots: &[PathBuf],
    max_depth: usize,
    max_entries: usize,
    mut visit: impl FnMut(PathBuf, std::fs::Metadata),
) -> bool {
    let mut pending: Vec<_> = roots.iter().cloned().map(|root| (root, 0usize)).collect();
    let mut directories = std::collections::HashSet::new();
    let mut remaining = max_entries;
    while let Some((directory, depth)) = pending.pop() {
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            let Ok(meta) = std::fs::metadata(&directory) else {
                continue;
            };
            (meta.dev(), meta.ino())
        };
        #[cfg(not(unix))]
        let identity = std::fs::canonicalize(&directory).unwrap_or_else(|_| directory.clone());
        if !directories.insert(identity) {
            continue;
        }
        let Ok(children) = std::fs::read_dir(directory) else {
            continue;
        };
        for child in children.flatten() {
            let Some(budget) = remaining.checked_sub(1) else {
                return false;
            };
            remaining = budget;
            let path = child.path();
            let has_font_extension = path.extension().is_some_and(|ext| {
                ["ttf", "otf", "ttc", "otc"]
                    .iter()
                    .any(|suffix| ext.eq_ignore_ascii_case(suffix))
            });
            // Directory entries normally carry their type. Stat only matching files or links,
            // which may point to directories even if their names have no font extension.
            if let Ok(kind) = child.file_type() {
                if kind.is_dir() {
                    if depth < max_depth {
                        pending.push((path, depth + 1));
                    }
                    continue;
                }
                if !kind.is_symlink() && !has_font_extension {
                    continue;
                }
            }
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if meta.is_dir() && depth < max_depth {
                pending.push((path, depth + 1));
            } else if meta.is_file() && has_font_extension {
                visit(path, meta);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn visits_real_font_files_once_through_symlink_cycles_and_honors_budgets() -> std::io::Result<()>
    {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!("joyoffice-font-walk-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let run = || -> std::io::Result<()> {
            let nested = root.join("nested");
            std::fs::create_dir(&nested)?;
            let face = nested.join("Family.TtC");
            std::fs::write(&face, b"one")?;
            std::fs::write(nested.join("readme.txt"), b"text")?;
            symlink(&root, nested.join("cycle"))?;
            let mut paths = Vec::new();
            assert!(visit_font_files(
                std::slice::from_ref(&root),
                12,
                20,
                |path, meta| paths.push((path, FileStamp::from(&meta)))
            ));
            assert_eq!(paths.len(), 1);
            assert_eq!(paths[0].0, face);
            let replacement = nested.join("new");
            std::fs::write(&replacement, b"two")?;
            std::fs::rename(replacement, &face)?;
            assert_ne!(paths[0].1, FileStamp::from(&std::fs::metadata(&face)?));
            assert!(!visit_font_files(
                std::slice::from_ref(&root),
                12,
                0,
                |_, _| {}
            ));
            let mut count = 0;
            assert!(visit_font_files(
                std::slice::from_ref(&root),
                0,
                20,
                |_, _| count += 1
            ));
            assert_eq!(count, 0);
            Ok(())
        };
        let result = run();
        std::fs::remove_dir_all(root)?;
        result
    }

    #[test]
    fn identity_distinguishes_file_content_and_collection_index() {
        let face = FileFace {
            sha256: file_digest(b"abc"),
            index: 0,
        };
        assert_eq!(
            format!("{face:x}"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(face, FileFace { index: 1, ..face });
        assert_ne!(
            face,
            FileFace {
                sha256: file_digest(b"abd"),
                ..face
            }
        );
        assert!(face.to_string().ends_with("/0"));
    }
}

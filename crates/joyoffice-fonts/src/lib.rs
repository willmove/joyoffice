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

#[cfg(test)]
mod tests {
    use super::*;

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

//! Production recovery log (T104). A checkpoint stores the original package
//! bytes and the text of the last committed transaction. Encrypted packages
//! are not written out as plaintext.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use joyoffice_opc::{Budget, ZipArchive, ZipError};

const MAGIC: &[u8; 8] = b"JWREC001";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovered {
    pub baseline: Vec<u8>,
    pub text: String,
}

pub fn package_is_encrypted(bytes: &[u8]) -> bool {
    matches!(
        ZipArchive::new(bytes, &Budget::default()),
        Err(ZipError::Unsupported(message)) if message.contains("encrypted")
    )
}

/// Append one committed transaction. `baseline` is the original package.
/// Returns an error and writes nothing when the package is encrypted.
pub fn commit(directory: &Path, baseline: &[u8], text: &str) -> io::Result<PathBuf> {
    if package_is_encrypted(baseline) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "encrypted documents are not written as plaintext recovery copies",
        ));
    }
    fs::create_dir_all(directory)?;
    let path = directory.join("recovery.jwr");
    let mut payload = Vec::new();
    payload.extend_from_slice(MAGIC);
    write_blob(&mut payload, baseline);
    write_blob(&mut payload, text.as_bytes());
    let crc = crc32fast::hash(&payload);
    payload.extend_from_slice(&crc.to_le_bytes());
    let temporary = directory.join("recovery.jwr.tmp");
    {
        let mut file = File::create(&temporary)?;
        file.write_all(&payload)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, &path)?;
    Ok(path)
}

/// Remove the journal for one document. Missing files are not an error.
/// Choosing Don't Save calls this so the next launch does not restore the
/// edits the user discarded.
pub fn clear(directory: &Path) -> io::Result<()> {
    for name in ["recovery.jwr", "recovery.jwr.tmp"] {
        match fs::remove_file(directory.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub fn recover(directory: &Path) -> io::Result<Recovered> {
    let path = directory.join("recovery.jwr");
    let mut file = File::open(&path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "recovery header",
        ));
    }
    let (crc_bytes, body) = bytes.split_at(bytes.len() - 4);
    let expected = u32::from_le_bytes(
        body.try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "recovery checksum"))?,
    );
    if crc32fast::hash(crc_bytes) != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "recovery checksum",
        ));
    }
    let mut cursor = &crc_bytes[8..];
    let baseline = read_blob(&mut cursor)?;
    let text = read_blob(&mut cursor)?;
    let text = String::from_utf8(text)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "recovery text"))?;
    Ok(Recovered { baseline, text })
}

/// Where a running editor journals a commit. An explicit directory wins.
/// Otherwise the journal lives under the per-user configuration directory.
pub fn directory_from(explicit: Option<&Path>, config_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit.filter(|path| !path.as_os_str().is_empty()) {
        return Some(path.to_path_buf());
    }
    config_dir.map(|dir| dir.join("recovery"))
}

/// `JOYWORD_RECOVERY_DIR` when it is set, otherwise [`directory_from`] with
/// the active profile's configuration directory. A test profile does not
/// fall back to the user preferences directory.
pub fn directory() -> Option<PathBuf> {
    let explicit = std::env::var_os("JOYWORD_RECOVERY_DIR").map(PathBuf::from);
    let config = crate::diag::resolve().ok().map(|paths| paths.config_dir);
    directory_from(explicit.as_deref(), config.as_deref())
}

fn write_blob(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn read_blob(input: &mut &[u8]) -> io::Result<Vec<u8>> {
    if input.len() < 8 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "blob"));
    }
    let mut len_bytes = [0; 8];
    len_bytes.copy_from_slice(&input[..8]);
    *input = &input[8..];
    let len = u64::from_le_bytes(len_bytes) as usize;
    if input.len() < len {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "blob"));
    }
    let blob = input[..len].to_vec();
    *input = &input[len..];
    Ok(blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_directory_wins_over_the_config_dir() {
        let chosen = directory_from(
            Some(Path::new("/var/joyword-recovery")),
            Some(Path::new("/cfg")),
        );
        assert_eq!(chosen, Some(PathBuf::from("/var/joyword-recovery")));
    }

    #[test]
    fn the_config_dir_is_used_when_no_directory_is_explicit() {
        let chosen = directory_from(Some(Path::new("")), Some(Path::new("/cfg")));
        assert_eq!(chosen, Some(PathBuf::from("/cfg/recovery")));
        assert_eq!(directory_from(None, None), None);
    }

    #[test]
    fn clearing_removes_a_committed_journal() {
        let dir = std::env::temp_dir().join(format!(
            "joyword-recovery-clear-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        commit(&dir, b"PK", "edited").expect("commit");
        assert_eq!(recover(&dir).expect("recover").text, "edited");
        clear(&dir).expect("clear");
        assert!(recover(&dir).is_err(), "discarded journal must not restore");
        clear(&dir).expect("clearing a missing journal is fine");
        let _ = fs::remove_dir_all(&dir);
    }
}

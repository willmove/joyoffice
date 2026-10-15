//! Two self-contained generations; keep the other generation and source log
//! throughout replacement. Full history is retained so engine undo survives.
use crate::{
    Error, io,
    journal::{self, Limits},
    replay::{self, Recovery},
};
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Written,
    Verified,
    Published,
}
#[derive(Debug)]
pub struct Receipt {
    pub path: PathBuf,
    pub records: usize,
    pub directory_synced: bool,
}
fn regular(path: &Path) -> Result<(), Error> {
    let m = fs::symlink_metadata(path).map_err(|e| io("checkpoint metadata", e))?;
    if !m.is_file() || m.file_type().is_symlink() {
        return Err(Error::Invalid("checkpoint is not a regular file"));
    }
    Ok(())
}
pub fn load(
    log: &Path,
    expected_session: [u8; 16],
    expected_source: [u8; 32],
) -> Result<(Vec<u8>, Recovery), Error> {
    regular(log)?;
    let mut bytes = journal::read(log, Limits::default())?;
    let scanned = journal::scan(&bytes, Limits::default())?;
    let recovered = replay::recover(
        &scanned.records,
        expected_session,
        expected_source,
        &AtomicBool::new(false),
    )?;
    bytes.truncate(scanned.valid_bytes);
    Ok((bytes, recovered))
}
/// New independent recovery artifact, never an overwrite or DOCX export.
pub fn copy_prefix(
    log: &Path,
    new_path: &Path,
    session: [u8; 16],
    source: [u8; 32],
) -> Result<Receipt, Error> {
    let (bytes, _) = load(log, session, source)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(new_path)
        .map_err(|e| io("create recovery copy", e))?;
    file.write_all(&bytes)
        .map_err(|e| io("write recovery copy", e))?;
    file.flush()
        .and_then(|()| file.sync_all())
        .map_err(|e| io("sync recovery copy", e))?;
    if journal::read(new_path, Limits::default())? != bytes {
        return Err(Error::Invalid("recovery copy readback"));
    }
    Ok(Receipt {
        path: new_path.into(),
        records: journal::scan(&bytes, Limits::default())?.records.len(),
        directory_synced: false,
    })
}
pub fn rotate(
    log: &Path,
    directory: &Path,
    session: [u8; 16],
    source: [u8; 32],
) -> Result<Receipt, Error> {
    rotate_with(log, directory, session, source, |_| Ok(()))
}
pub fn rotate_with(
    log: &Path,
    directory: &Path,
    session: [u8; 16],
    source: [u8; 32],
    mut hook: impl FnMut(Phase) -> Result<(), Error>,
) -> Result<Receipt, Error> {
    let (bytes, _) = load(log, session, source)?;
    if !directory.exists() {
        fs::create_dir(directory).map_err(|e| io("create checkpoint directory", e))?;
    }
    let m = fs::symlink_metadata(directory).map_err(|e| io("checkpoint directory", e))?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(Error::Invalid("checkpoint directory alias"));
    }
    let paths = [
        directory.join("checkpoint-a.jwj"),
        directory.join("checkpoint-b.jwj"),
    ];
    let mut lengths = [0, 0];
    for (i, path) in paths.iter().enumerate() {
        if path.try_exists().map_err(|e| io("checkpoint exists", e))? {
            let (old, _) = load(path, session, source)?;
            if !bytes.starts_with(&old) {
                return Err(Error::Invalid("checkpoint history diverges or regresses"));
            }
            lengths[i] = old.len();
        }
    }
    let slot = usize::from(lengths[0] > lengths[1]);
    let target = &paths[slot];
    if target.exists()
        && fs::canonicalize(target).map_err(|e| io("resolve checkpoint", e))?
            == fs::canonicalize(log).map_err(|e| io("resolve source log", e))?
    {
        return Err(Error::Invalid("checkpoint would replace source log"));
    }
    let pending = directory.join("checkpoint-pending.jwj");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|e| {
            io(
                "create pending checkpoint; preserve interrupted artifact",
                e,
            )
        })?;
    file.write_all(&bytes)
        .map_err(|e| io("write checkpoint", e))?;
    file.flush()
        .and_then(|()| file.sync_all())
        .map_err(|e| io("sync checkpoint", e))?;
    hook(Phase::Written)?;
    if journal::read(&pending, Limits::default())? != bytes {
        return Err(Error::Invalid("checkpoint readback differs"));
    }
    let _ = load(&pending, session, source)?;
    let count = journal::scan(&bytes, Limits::default())?.records.len();
    hook(Phase::Verified)?;
    // Closing our own handle also permits Windows rename on ordinary files;
    // failure with other open handles leaves both earlier generations intact.
    drop(file);
    fs::rename(&pending, target).map_err(|e| io("publish checkpoint", e))?;
    if let Err(error) = hook(Phase::Published) {
        return Err(Error::Published {
            path: target.clone(),
            cause: error.to_string(),
        });
    }
    #[cfg(unix)]
    let directory_synced = {
        File::open(directory)
            .and_then(|f| f.sync_all())
            .map_err(|e| Error::Published {
                path: target.clone(),
                cause: format!("directory sync: {e}"),
            })?;
        true
    };
    #[cfg(not(unix))]
    let directory_synced = false;
    Ok(Receipt {
        path: target.clone(),
        records: count,
        directory_synced,
    })
}

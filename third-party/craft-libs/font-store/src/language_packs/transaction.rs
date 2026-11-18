//! Publish the language folder and its inventory as one recoverable change.
//! Until the journal is renamed back into staging, the previous version wins
//! on restart. Staging and the journal are on the same filesystem as the store.
use super::{FontStore, Installed, InstalledLanguagePack, is_locale};
use crate::http::Cancel;
use crate::store::{Error, Stage, io};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

const JOURNAL: &str = ".language-pack-change";
const INTENT: &str = "previous.json";

#[derive(Serialize, Deserialize)]
struct Previous {
    locale: String,
    had_folder: bool,
    entry: Option<InstalledLanguagePack>,
}

fn exists(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io("checking the language pack folder")(e)),
    }
}

fn rename(from: &Path, to: &Path) -> Result<(), Error> {
    fs::rename(from, to).map_err(io("moving a language pack during publication"))
}

fn invalid(detail: &str) -> Error {
    io("recovering a language pack change")(std::io::Error::new(ErrorKind::InvalidData, detail))
}

fn update(inventory: &mut Installed, locale: &str, desired: Option<InstalledLanguagePack>) {
    inventory.packs.retain(|p| p.locale != locale);
    if let Some(pack) = desired {
        inventory.packs.push(pack);
        inventory.packs.sort_by(|a, b| a.locale.cmp(&b.locale));
    }
}

impl Previous {
    fn record(&self, store: &FontStore, stage: &Stage) -> Result<PathBuf, Error> {
        let prepared = stage.dir.join(".prepared-change");
        fs::create_dir(&prepared).map_err(io("preparing the language pack change"))?;
        let mut file = fs::File::create_new(prepared.join(INTENT))
            .map_err(io("creating the recovery record"))?;
        let data = serde_json::to_vec(self).map_err(|e| invalid(&e.to_string()))?;
        file.write_all(&data)
            .and_then(|()| file.sync_all())
            .map_err(io("saving the recovery record"))?;
        drop(file);
        let journal = store.dir().join(JOURNAL);
        rename(&prepared, &journal)?;
        Ok(journal)
    }

    fn restore(&self, store: &FontStore, journal: &Path, stage: &Stage) -> Result<(), Error> {
        let dest = store.language_packs_dir().join(&self.locale);
        let old = journal.join("previous");
        let backup = exists(&old)?;
        if self.had_folder && !backup && !exists(&dest)? {
            return Err(invalid("the previous language folder is missing"));
        }
        // An absent backup for an existing locale means it was never moved, or
        // a prior recovery already put it back. Leave the visible folder alone.
        if backup || !self.had_folder {
            if exists(&dest)? {
                rename(&dest, &stage.dir.join(".unpublished-pack"))?;
            }
            if backup {
                rename(&old, &dest)?;
            }
        }
        let mut inventory = store.read_installed()?;
        if inventory.packs.iter().find(|p| p.locale == self.locale) != self.entry.as_ref() {
            update(&mut inventory, &self.locale, self.entry.clone());
            store.write_installed(&inventory)?;
        }
        rename(journal, &stage.dir.join(".recovered-change"))
    }
}

/// Normal opens do not lock or write when the previous writer completed.
/// Mutation callers already hold the same interprocess lock as font installs.
pub(super) fn recover_on_open(store: &FontStore) -> Result<(), Error> {
    if exists(&store.dir().join(JOURNAL))? {
        let _writer = store.lock()?;
        recover_locked(store)?;
    }
    Ok(())
}

fn recover_locked(store: &FontStore) -> Result<(), Error> {
    let journal = store.dir().join(JOURNAL);
    if !exists(&journal)? {
        return Ok(());
    }
    let record = journal.join(INTENT);
    if !fs::symlink_metadata(&journal)
        .map_err(io("checking the journal"))?
        .is_dir()
        || !fs::symlink_metadata(&record)
            .map_err(io("checking the recovery record"))?
            .is_file()
    {
        return Err(invalid(
            "the pending record is not a regular file in the journal",
        ));
    }
    let mut data = Vec::new();
    fs::File::open(&record)
        .and_then(|f| {
            f.take(super::MAX_PACK_INDEX_BYTES + 1)
                .read_to_end(&mut data)
        })
        .map_err(io("reading the recovery record"))?;
    if data.len() as u64 > super::MAX_PACK_INDEX_BYTES {
        return Err(invalid("the pending record is too large"));
    }
    let prior: Previous = serde_json::from_slice(&data).map_err(|e| invalid(&e.to_string()))?;
    if !is_locale(&prior.locale)
        || prior
            .entry
            .as_ref()
            .is_some_and(|p| p.locale != prior.locale)
    {
        return Err(invalid("the previous locale is invalid"));
    }
    prior.restore(store, &journal, &Stage::new(store.dir())?)
}

/// `candidate` is already verified and fully unpacked. Publication, removal
/// and recovery all use the same inventory lock, including across processes.
pub(super) fn publish(
    store: &FontStore,
    stage: &Stage,
    locale: &str,
    candidate: Option<(&Path, &InstalledLanguagePack)>,
    cancel: Option<&Cancel>,
) -> Result<(), Error> {
    let _writer = store.lock()?;
    recover_locked(store)?;
    if cancel.is_some_and(Cancel::is_cancelled) {
        return Err(Error::Cancelled);
    }
    let mut inventory = store.read_installed()?;
    let dest = store.language_packs_dir().join(locale);
    let previous = Previous {
        locale: locale.to_owned(),
        had_folder: exists(&dest)?,
        entry: inventory.packs.iter().find(|p| p.locale == locale).cloned(),
    };
    if candidate.is_none() && !previous.had_folder && previous.entry.is_none() {
        return Err(Error::NotInstalled(locale.to_owned()));
    }
    if candidate.is_some() {
        fs::create_dir_all(store.language_packs_dir())
            .map_err(io("creating the language packs folder"))?;
    }
    let journal = previous.record(store, stage)?;
    let mut apply = || {
        if previous.had_folder {
            rename(&dest, &journal.join("previous"))?;
        }
        if let Some((incoming, _)) = candidate {
            rename(incoming, &dest)?;
        }
        if cancel.is_some_and(Cancel::is_cancelled) {
            return Err(Error::Cancelled);
        }
        update(
            &mut inventory,
            locale,
            candidate.map(|(_, record)| record.clone()),
        );
        store.write_installed(&inventory)?;
        // The rename is the commit point. The old files can now be discarded
        // by Stage without leaving an incomplete recovery marker.
        rename(&journal, &stage.dir.join(".committed-change"))
    };
    if let Err(error) = apply() {
        previous.restore(store, &journal, stage)?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Caller;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn record(tag: &str, version: &str) -> InstalledLanguagePack {
        InstalledLanguagePack {
            locale: tag.into(),
            name: tag.into(),
            release: "test".into(),
            version: version.into(),
            licence: "CC0-1.0".into(),
            sha256: "a".repeat(64),
            size: 20,
        }
    }

    fn directory() -> std::io::Result<PathBuf> {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let name = format!(
                "joyoffice-pack-transaction-{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::env::temp_dir().join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(path),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    // Simulate losing the publishing process at each filesystem boundary. Restore
    // through a fresh FontStore and use a second open to check idempotence.
    #[test]
    fn existing_new_and_removed_dictionaries_recover_at_each_publication_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        for (existing, installing) in [(true, true), (false, true), (true, false)] {
            for point in 0..5 {
                let root = directory()?;
                let store = FontStore::new(root.clone(), Caller::User);
                let packs = store.language_packs_dir();
                fs::create_dir_all(&packs)?;
                let dir = packs.join("nl");
                let old = existing.then(|| record("nl", "original"));
                let new = installing.then(|| record("nl", "candidate"));
                let other = record("sv", "untouched");
                fs::create_dir(packs.join("sv"))?;
                if existing {
                    fs::create_dir(&dir)?;
                    fs::write(dir.join("dictionary"), "original")?;
                }
                let mut inventory = Installed {
                    version: 1,
                    packs: vec![other.clone()],
                };
                update(&mut inventory, "nl", old.clone());
                store.write_installed(&inventory)?;
                let stage = Stage::new(&root)?;
                let before = Previous {
                    locale: "nl".into(),
                    had_folder: existing,
                    entry: old.clone(),
                };
                let journal = before.record(&store, &stage)?;
                if point >= 1 && existing {
                    rename(&dir, &journal.join("previous"))?;
                }
                if point >= 2 && installing {
                    fs::create_dir(&dir)?;
                    fs::write(dir.join("dictionary"), "candidate")?;
                }
                if point >= 3 {
                    update(&mut inventory, "nl", new.clone());
                    store.write_installed(&inventory)?;
                }
                if point == 4 {
                    rename(&journal, &stage.dir.join(".committed-change"))?;
                }
                drop(stage);
                drop(store);
                let reopened = FontStore::new(root.clone(), Caller::Agent);
                for _ in 0..2 {
                    assert_eq!(reopened.open_language_packs(), packs);
                    let expected = if point == 4 {
                        new.as_ref()
                    } else {
                        old.as_ref()
                    };
                    assert_eq!(dir.exists(), expected.is_some());
                    if let Some(record) = expected {
                        assert_eq!(fs::read_to_string(dir.join("dictionary"))?, record.version);
                    }
                    let records = reopened.installed_language_packs();
                    assert_eq!(records.iter().find(|p| p.locale == "nl"), expected);
                    assert!(records.contains(&other));
                    assert!(!journal.exists());
                }
                fs::remove_dir_all(root)?;
            }
        }
        Ok(())
    }
}

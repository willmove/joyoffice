//! Publish the language folder and its inventory as one recoverable change.
//! Until the journal is renamed back into staging, the previous version wins
//! on restart. Staging and the journal are on the same filesystem as the store.
use super::Cancel;
use super::storage::{
    Error, Installed, InstalledLanguagePack, MAX_PACK_INDEX_BYTES, PackStore, Stage, io, is_locale,
};
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
    fn record(&self, store: &PackStore, stage: &Stage) -> Result<PathBuf, Error> {
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

    fn restore(&self, store: &PackStore, journal: &Path, stage: &Stage) -> Result<(), Error> {
        let dest = store.language_packs_dir().join(&self.locale);
        let old = journal.join("previous");
        let backup = exists(&old)?;
        if self.had_folder && !backup && !exists(&dest)? {
            return Err(invalid("the previous language folder is missing"));
        }
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

pub(super) fn recover_on_open(store: &PackStore) -> Result<(), Error> {
    if exists(&store.dir().join(JOURNAL))? {
        let _writer = store.lock()?;
        recover_locked(store)?;
    }
    Ok(())
}

fn recover_locked(store: &PackStore) -> Result<(), Error> {
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
        .and_then(|f| f.take(MAX_PACK_INDEX_BYTES + 1).read_to_end(&mut data))
        .map_err(io("reading the recovery record"))?;
    if data.len() as u64 > MAX_PACK_INDEX_BYTES {
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

pub(super) fn publish(
    store: &PackStore,
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
    use super::super::storage::{Caller, Network};
    use super::*;

    fn sample_pack(locale: &str, version: &str) -> InstalledLanguagePack {
        InstalledLanguagePack {
            locale: locale.into(),
            name: "Nederlands".into(),
            release: "language-packs-2".into(),
            version: version.into(),
            licence: "BSD-3-Clause".into(),
            sha256: "a".repeat(64),
            size: 42,
        }
    }

    #[test]
    fn publish_recover_and_remove_language_pack_atomically()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("joyword-pack-tx-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = PackStore::new(dir.clone(), Caller::User);
        assert_eq!(store.network(), Network::Off);
        store.enable_network()?;
        assert_eq!(store.network(), Network::On);

        let stage = Stage::new(store.dir())?;
        let candidate = stage.dir.join("nl");
        fs::create_dir_all(&candidate)?;
        fs::write(candidate.join("pack.toml"), "locale = \"nl\"\n")?;
        let v1 = sample_pack("nl", "v1");
        publish(&store, &stage, "nl", Some((&candidate, &v1)), None)?;
        assert!(store.language_packs_dir().join("nl/pack.toml").is_file());
        assert_eq!(store.read_installed()?.packs, vec![v1]);

        // Cancel during update leaves v1 intact
        let cancel = Cancel::new();
        cancel.cancel();
        let stage2 = Stage::new(store.dir())?;
        let candidate2 = stage2.dir.join("nl");
        fs::create_dir_all(&candidate2)?;
        fs::write(candidate2.join("pack.toml"), "locale = \"nl\"\n")?;
        let v2 = sample_pack("nl", "v2");
        assert!(matches!(
            publish(
                &store,
                &stage2,
                "nl",
                Some((&candidate2, &v2)),
                Some(&cancel)
            ),
            Err(Error::Cancelled)
        ));
        assert_eq!(store.read_installed()?.packs[0].version, "v1");

        // Remove cleans folder and inventory
        store.remove_language_pack("nl")?;
        assert!(!store.language_packs_dir().join("nl").exists());
        assert!(store.read_installed()?.packs.is_empty());

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}

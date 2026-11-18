//! The font store as the packs [`Backend`](super::Backend): its folder, its consent setting, the
//! pinned pack index and verified installs (craft-libs `craft-font-store`, kind `language-pack`).

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use craft_font_store::{FontStore, Network, PackEntry, PackIndex, PackIndexPin};

use super::{Cancel, Pack};

/// The pack index this build trusts: craft-i18n's second Language pack release,
/// language-packs-2 (75 packs, built from craft-i18n `20467b63`), pinned by the sha256 of its `index.json`.
/// `CRAFT_LANGUAGE_PACK_INDEX` (read by the font store itself) points WordCraft at a local or test
/// index instead, for trying packs before they are released.
pub const PACK_INDEX: PackIndexPin = PackIndexPin {
    url: "https://github.com/storytold/craft-i18n/releases/download/language-packs-2/index.json",
    sha256: "b35c7aef0591177f18233b551da16035dcd615549ff874e169ffd19f16fea74b",
};

pub(super) struct FontStoreBackend {
    store: FontStore,
    /// The index last read or fetched; installs take their entry (url, size, sha256) from it.
    index: Mutex<Option<PackIndex>>,
}

/// Keep existing installs, consent and cached indexes readable at their original location.
fn store_folder(get: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    let path = |key| get(key).filter(|p| !p.as_os_str().is_empty());
    if let Some(override_dir) = path("CRAFT_FONT_STORE") {
        return Some(override_dir);
    }
    if cfg!(any(
        target_os = "ios",
        target_os = "android",
        target_arch = "wasm32"
    )) {
        return None;
    }
    if cfg!(windows) {
        return Some(path("LOCALAPPDATA")?.join("Craft Fonts"));
    }
    if cfg!(target_os = "macos") {
        return Some(path("HOME")?.join("Library/Application Support/Craft Fonts"));
    }
    let data = path("XDG_DATA_HOME")
        .filter(|p| p.is_absolute())
        .or_else(|| path("HOME").map(|p| p.join(".local/share")))?;
    Some(data.join("craft-fonts"))
}

impl FontStoreBackend {
    pub(super) fn new(user: bool) -> Option<Self> {
        let path = store_folder(|key| std::env::var_os(key).map(PathBuf::from))?;
        Some(Self::in_dir(path, user))
    }

    /// A store in `dir`.
    pub(super) fn in_dir(dir: PathBuf, user: bool) -> Self {
        let caller = if user {
            craft_font_store::Caller::User
        } else {
            craft_font_store::Caller::Agent
        };
        Self {
            store: FontStore::new(dir, caller),
            index: Mutex::new(None),
        }
    }

    fn keep(&self, index: PackIndex) -> Vec<Pack> {
        let packs = index.packs.iter().map(pack).collect();
        *self.index.lock().unwrap_or_else(PoisonError::into_inner) = Some(index);
        packs
    }
}

fn pack(e: &PackEntry) -> Pack {
    Pack {
        locale: e.locale.clone(),
        name: e.name.clone(),
        version: e.version.clone(),
        url: e.url.clone(),
        source: e.source.clone(),
        size: e.size,
        licence: e.licence.clone(),
        class: e.class.clone(),
        spelling: e.spelling,
        hyphenation: e.hyphenation,
    }
}

impl super::Backend for FontStoreBackend {
    fn index(&self) -> Result<Vec<Pack>, String> {
        Ok(self
            .store
            .cached_language_pack_index(&PACK_INDEX)
            .map(|i| self.keep(i))
            .unwrap_or_default())
    }

    fn fetch_index(&self, cancel: &Cancel) -> Result<Vec<Pack>, String> {
        let index = self
            .store
            .fetch_language_pack_index(&PACK_INDEX, &cancel.0)
            .map_err(|e| e.to_string())?;
        Ok(self.keep(index))
    }

    fn network_on(&self) -> bool {
        self.store.network() == Network::On
    }

    fn enable_network(&self) -> Result<(), String> {
        self.store.enable_network().map_err(|e| e.to_string())
    }

    fn install(
        &self,
        pack: &Pack,
        cancel: &Cancel,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<(), String> {
        let index = {
            let held = self
                .index
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            match held {
                Some(i) => i,
                None => self
                    .store
                    .cached_language_pack_index(&PACK_INDEX)
                    .ok_or("the list of Language packs hasn't been fetched")?,
            }
        };
        let entry = index
            .pack(&pack.locale)
            .ok_or_else(|| format!("there is no language pack for {}", pack.locale))?
            .clone();
        self.store
            .install_language_pack(&index, &entry, &cancel.0, &mut |p| {
                progress(p.done, p.total)
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn remove(&self, locale: &str) -> Result<(), String> {
        self.store
            .remove_language_pack(locale)
            .map_err(|e| e.to_string())
    }

    fn index_override(&self) -> Option<String> {
        craft_font_store::language_pack_index_overridden()
    }

    fn packs_dir(&self) -> Option<PathBuf> {
        Some(self.store.language_packs_dir())
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;
    #[test]
    fn display_uses_the_verified_dictionary_source_or_the_archive_on_old_indexes() {
        let archive = "https://downloads.example.test/packs/nl.tar.zst";
        let origin = "https://data.example.test/dictionaries/nl";
        let mut index = serde_json::json!({"format":1,"release":"language-packs-2","packs":[{
            "locale":"nl","name":"Nederlands","url":archive,"sha256":"a".repeat(64),
            "size":123,"licence":"BSD-3-Clause","source":origin
        }]});
        for expected in [origin, archive] {
            let parsed = PackIndex::from_json(index.to_string().as_bytes()).ok();
            let displayed = parsed.as_ref().and_then(|i| i.pack("nl")).map(pack);
            assert_eq!(displayed.as_ref().map(Pack::source_link), Some(expected));
            assert_eq!(
                displayed.as_ref().map(|p| p.licence.as_str()),
                Some("BSD-3-Clause")
            );
            if let Some(fields) = index["packs"][0].as_object_mut() {
                fields.remove("source");
            }
        }
    }

    #[test]
    fn old_language_assets_and_consent_use_the_same_folder() {
        let loc = |var: &str| match var {
            "LOCALAPPDATA" => Some(PathBuf::from("Local")),
            "HOME" => Some(PathBuf::from("/profile")),
            "XDG_DATA_HOME" => Some(PathBuf::from("relative-must-be-ignored")),
            _ => None,
        };
        let expected = if cfg!(windows) {
            "Local/Craft Fonts"
        } else if cfg!(target_os = "macos") {
            "/profile/Library/Application Support/Craft Fonts"
        } else if cfg!(any(target_os = "android", target_os = "ios")) {
            assert_eq!(store_folder(loc), None);
            return;
        } else {
            "/profile/.local/share/craft-fonts"
        };
        assert_eq!(store_folder(loc), Some(PathBuf::from(expected)));
        assert_eq!(
            store_folder(|k| (k == "CRAFT_FONT_STORE").then(|| PathBuf::from("existing"))),
            Some(PathBuf::from("existing"))
        );
        assert_eq!(store_folder(|_| None), None);
    }
}

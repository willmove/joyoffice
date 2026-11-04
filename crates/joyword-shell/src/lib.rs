//! Window-independent preview state. No second editable document model.
pub mod advanced;
pub mod authoring;
pub mod editing;
pub mod formatting;
pub mod l10n;
mod legacy_recovery;
pub mod prefs;
pub mod reader;
pub mod storage;

pub use legacy_recovery::validate_legacy_recovery_record;

use l10n::Locale;
use prefs::{Loaded, PrefsStore};

pub struct CommandSpec {
    pub id: &'static str,
    pub enabled: bool,
}
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "file.new",
        enabled: true,
    },
    CommandSpec {
        id: "view.zoom_in",
        enabled: true,
    },
    CommandSpec {
        id: "view.zoom_out",
        enabled: true,
    },
    CommandSpec {
        id: "view.zoom_reset",
        enabled: true,
    },
    CommandSpec {
        id: "view.theme",
        enabled: true,
    },
    CommandSpec {
        id: "view.language",
        enabled: true,
    },
    CommandSpec {
        id: "file.open",
        enabled: false,
    },
    CommandSpec {
        id: "file.save",
        enabled: false,
    },
    CommandSpec {
        id: "edit.undo",
        enabled: false,
    },
    CommandSpec {
        id: "edit.redo",
        enabled: false,
    },
];

pub struct Shell {
    pub locale: Locale,
    pub dark: bool,
    pub zoom: u16,
    pub document_number: u32,
    pub notice: Option<String>,
    store: Option<PrefsStore>,
    preferences: Option<Loaded>,
}

impl Default for Shell {
    fn default() -> Self {
        let (store, preferences) = prefs::default_config_dir().map_or((None, None), |root| {
            let store = PrefsStore::new(root.join("migration-preview"));
            let loaded = store.load_with_legacy(&root, Locale::ZhCn);
            (Some(store), Some(loaded))
        });
        let mut shell = Self {
            locale: preferences
                .as_ref()
                .map_or(Locale::ZhCn, |p| p.prefs.ui_locale),
            dark: false,
            zoom: 100,
            document_number: 1,
            notice: None,
            store,
            preferences,
        };
        if let Some(loaded) = &shell.preferences {
            shell.dark = loaded
                .prefs
                .unknown
                .iter()
                .any(|(k, v)| k == "preview.theme" && v == "dark");
        }
        shell
    }
}

impl Shell {
    pub fn tr(&self, zh: &'static str, en: &'static str) -> &'static str {
        if self.locale == Locale::ZhCn { zh } else { en }
    }
    pub fn title(&self) -> String {
        format!(
            "{} {} — JoyWord",
            self.tr("未命名", "Untitled"),
            self.document_number
        )
    }
    pub fn execute(&mut self, id: &str) -> Result<(), String> {
        let spec = COMMANDS
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| format!("Unknown command: {id}"))?;
        if !spec.enabled {
            return Err(self
                .tr(
                    "此功能尚未接入，当前桌面为 DOCX 只读预览。",
                    "This feature is not connected. This desktop provides read-only DOCX preview.",
                )
                .into());
        }
        match id {
            "file.new" => {
                self.document_number = self
                    .document_number
                    .checked_add(1)
                    .ok_or("Document counter exhausted")?
            }
            "view.zoom_in" => self.zoom = self.zoom.saturating_add(10).min(200),
            "view.zoom_out" => self.zoom = self.zoom.saturating_sub(10).max(50),
            "view.zoom_reset" => self.zoom = 100,
            "view.theme" => self.dark = !self.dark,
            "view.language" => {
                self.locale = if self.locale == Locale::ZhCn {
                    Locale::En
                } else {
                    Locale::ZhCn
                }
            }
            _ => return Err(format!("Command is not implemented: {id}")),
        }
        if matches!(id, "view.theme" | "view.language") {
            self.persist()?;
        }
        Ok(())
    }
    fn persist(&mut self) -> Result<(), String> {
        if let (Some(store), Some(loaded)) = (&self.store, &mut self.preferences) {
            loaded.prefs.ui_locale = self.locale;
            loaded.prefs.unknown.retain(|(k, _)| k != "preview.theme");
            loaded.prefs.unknown.push((
                "preview.theme".into(),
                if self.dark { "dark" } else { "light" }.into(),
            ));
            store
                .save(loaded)
                .map_err(|e| format!("Preferences: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn isolated() -> Shell {
        Shell {
            locale: Locale::ZhCn,
            dark: false,
            zoom: 100,
            document_number: 1,
            notice: None,
            store: None,
            preferences: None,
        }
    }
    #[test]
    fn unavailable_commands_do_not_mutate_preview() {
        let mut shell = isolated();
        assert!(shell.execute("file.save").is_err());
        assert!(shell.execute("format.bold").is_err());
        assert_eq!(shell.document_number, 1);
        assert_eq!(shell.zoom, 100);
    }
    #[test]
    fn zoom_is_bounded_and_new_does_not_create_editable_state() -> Result<(), String> {
        let mut shell = isolated();
        for _ in 0..100 {
            shell.execute("view.zoom_out")?;
        }
        assert_eq!(shell.zoom, 50);
        for _ in 0..100 {
            shell.execute("view.zoom_in")?;
        }
        assert_eq!(shell.zoom, 200);
        shell.execute("file.new")?;
        assert_eq!(shell.document_number, 2);
        Ok(())
    }
}

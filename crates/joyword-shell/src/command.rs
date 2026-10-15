//! Command registry (PLATFORM-002, ARCHITECTURE §3). Menus, keyboard
//! shortcuts, the command line and later the ribbon all resolve to the same
//! entries and run through [`Registry::invoke`], which checks availability
//! before calling the host. Only implemented commands are registered, so an
//! unimplemented feature never shows up as a dead menu item.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::l10n::Catalog;

/// Stable command identifier, e.g. `view.zoom_in`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandId(pub &'static str);

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// Top-level menus in display order. Quit stays on the application menu.
/// File has Open and Save As only; there is no original-file Save command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MenuKind {
    File,
    App,
    Edit,
    View,
}

impl MenuKind {
    pub fn label_key(self) -> &'static str {
        match self {
            MenuKind::File => "menu.file",
            MenuKind::App => "menu.app",
            MenuKind::Edit => "menu.edit",
            MenuKind::View => "menu.view",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuPlacement {
    pub menu: MenuKind,
    /// Items with different groups are separated.
    pub group: u8,
}

/// Keystrokes in GPUI notation (`cmd-q`, `ctrl-=`), per platform family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub mac: Option<&'static str>,
    pub other: Option<&'static str>,
}

impl Shortcut {
    pub const NONE: Shortcut = Shortcut {
        mac: None,
        other: None,
    };

    const fn primary(mac: &'static str, other: &'static str) -> Self {
        Shortcut {
            mac: Some(mac),
            other: Some(other),
        }
    }

    pub fn for_platform(self, platform: Platform) -> Option<&'static str> {
        match platform {
            Platform::MacOs => self.mac,
            Platform::Windows | Platform::Linux => self.other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: CommandId,
    /// Localization key of the menu and command-search label.
    pub label: &'static str,
    pub menu: Option<MenuPlacement>,
    pub shortcut: Shortcut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Enabled,
    /// Shown but disabled, with a localization key explaining why.
    Disabled(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvokeError {
    Unknown(String),
    Disabled(&'static str),
    Failed(String),
}

/// The application side of a command: availability and execution.
pub trait CommandHost {
    fn availability(&self, id: CommandId) -> Availability;
    fn execute(&mut self, id: CommandId) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Command(CommandId),
    Separator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuModel {
    pub kind: MenuKind,
    pub entries: Vec<MenuEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryIssue {
    DuplicateId(CommandId),
    ShortcutConflict {
        platform: Platform,
        keys: &'static str,
        commands: (CommandId, CommandId),
    },
    MissingLabel {
        command: CommandId,
        key: &'static str,
        locale: &'static str,
    },
}

pub const QUIT: CommandId = CommandId("app.quit");
pub const FILE_OPEN: CommandId = CommandId("file.open");
pub const FILE_SAVE_COPY: CommandId = CommandId("file.save_copy");
pub const ZOOM_IN: CommandId = CommandId("view.zoom_in");
pub const ZOOM_OUT: CommandId = CommandId("view.zoom_out");
pub const ZOOM_RESET: CommandId = CommandId("view.zoom_reset");
pub const SWITCH_LANGUAGE: CommandId = CommandId("view.switch_language");

pub const EDIT_UNDO: CommandId = CommandId("edit.undo");
pub const EDIT_REDO: CommandId = CommandId("edit.redo");
pub const EDIT_SELECT_ALL: CommandId = CommandId("edit.select_all");
pub const EDIT_BACKSPACE: CommandId = CommandId("edit.backspace");
pub const EDIT_DELETE: CommandId = CommandId("edit.delete");
pub const EDIT_ENTER: CommandId = CommandId("edit.new_paragraph");
pub const EDIT_LEFT: CommandId = CommandId("edit.left");
pub const EDIT_RIGHT: CommandId = CommandId("edit.right");
pub const EDIT_EXTEND_LEFT: CommandId = CommandId("edit.extend_left");
pub const EDIT_EXTEND_RIGHT: CommandId = CommandId("edit.extend_right");
pub const EDIT_UP: CommandId = CommandId("edit.up");
pub const EDIT_DOWN: CommandId = CommandId("edit.down");
pub const EDIT_EXTEND_UP: CommandId = CommandId("edit.extend_up");
pub const EDIT_EXTEND_DOWN: CommandId = CommandId("edit.extend_down");
pub const EDIT_HOME: CommandId = CommandId("edit.line_start");
pub const EDIT_END: CommandId = CommandId("edit.line_end");
pub const EDIT_EXTEND_HOME: CommandId = CommandId("edit.extend_line_start");
pub const EDIT_EXTEND_END: CommandId = CommandId("edit.extend_line_end");
pub const EDIT_CANCEL: CommandId = CommandId("edit.cancel_composition");

#[derive(Clone, Debug)]
pub struct Registry {
    commands: Vec<CommandSpec>,
}

impl Registry {
    pub fn new(commands: Vec<CommandSpec>) -> Self {
        Self { commands }
    }

    /// Commands JoyWord implements today. New entries arrive with the
    /// feature that makes them work (file commands with T103, editing with
    /// T102), never ahead of it.
    pub fn builtin() -> Self {
        let view = |group| {
            Some(MenuPlacement {
                menu: MenuKind::View,
                group,
            })
        };
        Self::new(vec![
            CommandSpec {
                id: FILE_OPEN,
                label: "command.file.open",
                menu: Some(MenuPlacement {
                    menu: MenuKind::File,
                    group: 0,
                }),
                shortcut: Shortcut::primary("cmd-o", "ctrl-o"),
            },
            CommandSpec {
                id: FILE_SAVE_COPY,
                label: "command.file.save_copy",
                menu: Some(MenuPlacement {
                    menu: MenuKind::File,
                    group: 0,
                }),
                shortcut: Shortcut::primary("cmd-shift-s", "ctrl-shift-s"),
            },
            CommandSpec {
                id: QUIT,
                label: "command.app.quit",
                menu: Some(MenuPlacement {
                    menu: MenuKind::App,
                    group: 9,
                }),
                shortcut: Shortcut::primary("cmd-q", "ctrl-q"),
            },
            CommandSpec {
                id: ZOOM_IN,
                label: "command.view.zoom_in",
                menu: view(0),
                shortcut: Shortcut::primary("cmd-=", "ctrl-="),
            },
            CommandSpec {
                id: ZOOM_OUT,
                label: "command.view.zoom_out",
                menu: view(0),
                shortcut: Shortcut::primary("cmd--", "ctrl--"),
            },
            CommandSpec {
                id: ZOOM_RESET,
                label: "command.view.zoom_reset",
                menu: view(0),
                shortcut: Shortcut::primary("cmd-0", "ctrl-0"),
            },
            CommandSpec {
                id: SWITCH_LANGUAGE,
                label: "command.view.switch_language",
                menu: view(1),
                shortcut: Shortcut::NONE,
            },
            CommandSpec {
                id: EDIT_UNDO,
                label: "command.edit.undo",
                menu: Some(MenuPlacement {
                    menu: MenuKind::Edit,
                    group: 0,
                }),
                shortcut: Shortcut::primary("cmd-z", "ctrl-z"),
            },
            CommandSpec {
                id: EDIT_REDO,
                label: "command.edit.redo",
                menu: Some(MenuPlacement {
                    menu: MenuKind::Edit,
                    group: 0,
                }),
                shortcut: Shortcut::primary("cmd-shift-z", "ctrl-y"),
            },
            CommandSpec {
                id: EDIT_SELECT_ALL,
                label: "command.edit.select_all",
                menu: Some(MenuPlacement {
                    menu: MenuKind::Edit,
                    group: 1,
                }),
                shortcut: Shortcut::primary("cmd-a", "ctrl-a"),
            },
            CommandSpec {
                id: EDIT_BACKSPACE,
                label: "command.edit.backspace",
                menu: None,
                shortcut: Shortcut::primary("backspace", "backspace"),
            },
            CommandSpec {
                id: EDIT_DELETE,
                label: "command.edit.delete",
                menu: None,
                shortcut: Shortcut::primary("delete", "delete"),
            },
            CommandSpec {
                id: EDIT_ENTER,
                label: "command.edit.new_paragraph",
                menu: None,
                shortcut: Shortcut::primary("enter", "enter"),
            },
            CommandSpec {
                id: EDIT_LEFT,
                label: "command.edit.left",
                menu: None,
                shortcut: Shortcut::primary("left", "left"),
            },
            CommandSpec {
                id: EDIT_RIGHT,
                label: "command.edit.right",
                menu: None,
                shortcut: Shortcut::primary("right", "right"),
            },
            CommandSpec {
                id: EDIT_EXTEND_LEFT,
                label: "command.edit.extend_left",
                menu: None,
                shortcut: Shortcut::primary("shift-left", "shift-left"),
            },
            CommandSpec {
                id: EDIT_EXTEND_RIGHT,
                label: "command.edit.extend_right",
                menu: None,
                shortcut: Shortcut::primary("shift-right", "shift-right"),
            },
            CommandSpec {
                id: EDIT_UP,
                label: "command.edit.up",
                menu: None,
                shortcut: Shortcut::primary("up", "up"),
            },
            CommandSpec {
                id: EDIT_DOWN,
                label: "command.edit.down",
                menu: None,
                shortcut: Shortcut::primary("down", "down"),
            },
            CommandSpec {
                id: EDIT_EXTEND_UP,
                label: "command.edit.extend_up",
                menu: None,
                shortcut: Shortcut::primary("shift-up", "shift-up"),
            },
            CommandSpec {
                id: EDIT_EXTEND_DOWN,
                label: "command.edit.extend_down",
                menu: None,
                shortcut: Shortcut::primary("shift-down", "shift-down"),
            },
            CommandSpec {
                id: EDIT_HOME,
                label: "command.edit.line_start",
                menu: None,
                shortcut: Shortcut::primary("cmd-left", "home"),
            },
            CommandSpec {
                id: EDIT_END,
                label: "command.edit.line_end",
                menu: None,
                shortcut: Shortcut::primary("cmd-right", "end"),
            },
            CommandSpec {
                id: EDIT_EXTEND_HOME,
                label: "command.edit.extend_line_start",
                menu: None,
                shortcut: Shortcut::primary("cmd-shift-left", "shift-home"),
            },
            CommandSpec {
                id: EDIT_EXTEND_END,
                label: "command.edit.extend_line_end",
                menu: None,
                shortcut: Shortcut::primary("cmd-shift-right", "shift-end"),
            },
            CommandSpec {
                id: EDIT_CANCEL,
                label: "command.edit.cancel_composition",
                menu: None,
                shortcut: Shortcut::primary("escape", "escape"),
            },
        ])
    }

    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }

    pub fn get(&self, id: CommandId) -> Option<&CommandSpec> {
        self.commands.iter().find(|command| command.id == id)
    }

    /// Looks up a command by its textual identifier (command line, search).
    pub fn find(&self, name: &str) -> Option<&CommandSpec> {
        self.commands.iter().find(|command| command.id.0 == name)
    }

    /// The single entry point for menus, shortcuts and the command line.
    pub fn invoke(&self, host: &mut dyn CommandHost, id: CommandId) -> Result<(), InvokeError> {
        if self.get(id).is_none() {
            return Err(InvokeError::Unknown(id.0.to_owned()));
        }
        match host.availability(id) {
            Availability::Enabled => host.execute(id).map_err(InvokeError::Failed),
            Availability::Disabled(reason) => Err(InvokeError::Disabled(reason)),
        }
    }

    /// Menus in display order, groups separated. The platform is a
    /// parameter so platform-specific placement (Quit moves to File outside
    /// macOS once File exists) stays testable here.
    pub fn menus(&self, _platform: Platform) -> Vec<MenuModel> {
        let mut grouped: BTreeMap<MenuKind, BTreeMap<u8, Vec<CommandId>>> = BTreeMap::new();
        for command in &self.commands {
            if let Some(placement) = command.menu {
                grouped
                    .entry(placement.menu)
                    .or_default()
                    .entry(placement.group)
                    .or_default()
                    .push(command.id);
            }
        }
        grouped
            .into_iter()
            .map(|(kind, groups)| {
                let mut entries = Vec::new();
                for (index, (_, ids)) in groups.into_iter().enumerate() {
                    if index > 0 {
                        entries.push(MenuEntry::Separator);
                    }
                    entries.extend(ids.into_iter().map(MenuEntry::Command));
                }
                MenuModel { kind, entries }
            })
            .collect()
    }

    /// Duplicate identifiers, conflicting shortcuts per platform and labels
    /// missing from any catalog.
    pub fn validate(&self, catalogs: &[&Catalog]) -> Vec<RegistryIssue> {
        let mut issues = Vec::new();
        let mut seen = BTreeSet::new();
        for command in &self.commands {
            if !seen.insert(command.id) {
                issues.push(RegistryIssue::DuplicateId(command.id));
            }
            for catalog in catalogs {
                let keys = std::iter::once(command.label)
                    .chain(command.menu.map(|placement| placement.menu.label_key()));
                for key in keys {
                    if !catalog.contains(key) {
                        issues.push(RegistryIssue::MissingLabel {
                            command: command.id,
                            key,
                            locale: catalog.locale().tag(),
                        });
                    }
                }
            }
        }
        for platform in [Platform::MacOs, Platform::Windows, Platform::Linux] {
            let mut owners: BTreeMap<&'static str, CommandId> = BTreeMap::new();
            for command in &self.commands {
                if let Some(keys) = command.shortcut.for_platform(platform)
                    && let Some(previous) = owners.insert(keys, command.id)
                {
                    issues.push(RegistryIssue::ShortcutConflict {
                        platform,
                        keys,
                        commands: (previous, command.id),
                    });
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::l10n::Locale;

    struct Host {
        zoom: u16,
        log: Vec<CommandId>,
    }

    impl CommandHost for Host {
        fn availability(&self, id: CommandId) -> Availability {
            match id {
                ZOOM_IN if self.zoom >= 150 => Availability::Disabled("zoom.max"),
                _ => Availability::Enabled,
            }
        }

        fn execute(&mut self, id: CommandId) -> Result<(), String> {
            if id == ZOOM_IN {
                self.zoom += 10;
            }
            self.log.push(id);
            Ok(())
        }
    }

    #[test]
    fn builtin_registry_is_consistent_with_shipped_catalogs() {
        let catalogs: Vec<&Catalog> = Locale::ALL.iter().map(|&l| Catalog::shipped(l)).collect();
        assert_eq!(Registry::builtin().validate(&catalogs), vec![]);
    }

    #[test]
    fn menus_shortcuts_and_names_reach_the_same_command() {
        let registry = Registry::builtin();
        let mut host = Host {
            zoom: 100,
            log: Vec::new(),
        };
        // Menu entry, shortcut lookup and textual name all yield ZOOM_IN.
        let from_menu = registry
            .menus(Platform::MacOs)
            .into_iter()
            .flat_map(|menu| menu.entries)
            .find_map(|entry| match entry {
                MenuEntry::Command(id) if id == ZOOM_IN => Some(id),
                _ => None,
            })
            .unwrap();
        let from_shortcut = registry
            .commands()
            .iter()
            .find(|c| c.shortcut.for_platform(Platform::Windows) == Some("ctrl-="))
            .unwrap()
            .id;
        let from_name = registry.find("view.zoom_in").unwrap().id;
        for id in [from_menu, from_shortcut, from_name] {
            registry.invoke(&mut host, id).unwrap();
        }
        assert_eq!(host.log, vec![ZOOM_IN; 3]);
        assert_eq!(host.zoom, 130);
        host.zoom = 150;
        assert_eq!(
            registry.invoke(&mut host, ZOOM_IN),
            Err(InvokeError::Disabled("zoom.max"))
        );
        assert_eq!(
            registry.invoke(&mut host, CommandId("file.save")),
            Err(InvokeError::Unknown("file.save".into()))
        );
    }

    #[test]
    fn menus_group_with_separators_and_validation_finds_conflicts() {
        let registry = Registry::builtin();
        let view = registry
            .menus(Platform::Linux)
            .into_iter()
            .find(|menu| menu.kind == MenuKind::View)
            .unwrap();
        assert_eq!(
            view.entries,
            vec![
                MenuEntry::Command(ZOOM_IN),
                MenuEntry::Command(ZOOM_OUT),
                MenuEntry::Command(ZOOM_RESET),
                MenuEntry::Separator,
                MenuEntry::Command(SWITCH_LANGUAGE),
            ]
        );
        let mut commands = registry.commands().to_vec();
        commands.push(CommandSpec {
            id: ZOOM_IN,
            label: "missing.label",
            menu: None,
            shortcut: Shortcut::primary("cmd-q", "ctrl-0"),
        });
        let catalog = Catalog::shipped(Locale::En);
        let issues = Registry::new(commands).validate(&[catalog]);
        assert!(issues.contains(&RegistryIssue::DuplicateId(ZOOM_IN)));
        assert!(issues.iter().any(|i| matches!(
            i,
            RegistryIssue::MissingLabel {
                key: "missing.label",
                ..
            }
        )));
        assert!(issues.iter().any(|i| matches!(
            i,
            RegistryIssue::ShortcutConflict {
                platform: Platform::MacOs,
                keys: "cmd-q",
                ..
            }
        )));
        assert!(issues.iter().any(|i| matches!(
            i,
            RegistryIssue::ShortcutConflict {
                platform: Platform::Linux,
                keys: "ctrl-0",
                ..
            }
        )));
    }
}

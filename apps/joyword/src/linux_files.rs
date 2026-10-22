//! Linux file entry when GPUI does not draw a menu bar.
//!
//! `gpui-pre-linux` 0.3.9 stores `set_menus` and never paints them. Its file
//! prompt waits on the XDG desktop portal with no timeout; on UKUI/X11 that
//! request can return nothing and show no window. Open, save-copy and export
//! therefore ask for a path inside the window. Browse tries zenity, qarma,
//! then kdialog, and leaves the typed path usable when those programs are
//! missing or fail.

use std::path::{Path, PathBuf};
use std::process::Command;

use joyword_shell::command::{
    CommandId, EDIT_COPY, EDIT_CUT, EDIT_PASTE, EDIT_REDO, EDIT_UNDO, FILE_EXPORT_PDF, FILE_OPEN,
    FILE_SAVE, FILE_SAVE_COPY,
};

/// File commands, then Edit commands. The window bar splits at index 4.
pub(crate) const LINUX_BAR: &[CommandId] = &[
    FILE_OPEN,
    FILE_SAVE,
    FILE_SAVE_COPY,
    FILE_EXPORT_PDF,
    EDIT_UNDO,
    EDIT_REDO,
    EDIT_CUT,
    EDIT_COPY,
    EDIT_PASTE,
];

pub(crate) const DIALOG_PROGRAMS: &[&str] = &["zenity", "qarma", "kdialog"];

/// GPUI on Linux keeps menus in memory and does not draw a menu bar.
pub(crate) fn window_needs_command_bar() -> bool {
    cfg!(target_os = "linux")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathRole {
    Open,
    SaveCopy,
    ExportPdf,
    Print,
}

pub(crate) struct PathSheet {
    pub role: PathRole,
    pub draft: crate::field_edit::FieldBuffer,
    pub focus: gpui::FocusHandle,
    pub note: Option<String>,
    pub browse: bool,
    pub browsing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LinuxPick {
    Chosen(PathBuf),
    Cancelled,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DialogOutcome {
    Chosen(PathBuf),
    Cancelled,
    TryNext,
}

/// `ctrl-e` becomes `Ctrl+E`. Single letters are uppercased; words stay as named.
pub(crate) fn chord_label(keys: &str) -> String {
    keys.split('-')
        .map(|part| {
            if part.chars().count() == 1 {
                part.to_ascii_uppercase()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

pub(crate) fn first_program(dirs: &[PathBuf]) -> Option<&'static str> {
    DIALOG_PROGRAMS
        .iter()
        .copied()
        .find(|program| dirs.iter().any(|dir| dir.join(program).is_file()))
}

pub(crate) fn dialog_program_available() -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    first_program(&dirs).is_some()
}

pub(crate) fn dialog_argv(
    program: &str,
    role: PathRole,
    directory: &Path,
    suggested: Option<&str>,
) -> Vec<String> {
    let file = match suggested {
        Some(name) => directory.join(name),
        None => directory.to_path_buf(),
    };
    let file = file.display().to_string();
    let directory = directory.display().to_string();
    if program == "kdialog" {
        return match role {
            PathRole::Open => vec![program.to_owned(), "--getopenfilename".into(), directory],
            PathRole::SaveCopy | PathRole::ExportPdf | PathRole::Print => {
                vec![program.to_owned(), "--getsavefilename".into(), file]
            }
        };
    }
    let mut args = vec![program.to_owned(), "--file-selection".into()];
    match role {
        PathRole::Open => {
            args.push("--title".into());
            args.push("Open".into());
        }
        PathRole::SaveCopy | PathRole::ExportPdf | PathRole::Print => {
            args.push("--save".into());
            args.push("--confirm-overwrite".into());
            args.push("--filename".into());
            args.push(file);
            args.push("--title".into());
            args.push(match role {
                PathRole::ExportPdf => "Export PDF".into(),
                PathRole::Print => "Print".into(),
                PathRole::SaveCopy | PathRole::Open => "Save".into(),
            });
        }
    }
    args
}

pub(crate) fn interpret_dialog(success: bool, stdout: &str, stderr: &str) -> DialogOutcome {
    let path = stdout.lines().next().map(str::trim).unwrap_or("");
    if success && !path.is_empty() {
        DialogOutcome::Chosen(PathBuf::from(path))
    } else if !success && path.is_empty() && stderr.trim().is_empty() {
        DialogOutcome::Cancelled
    } else {
        DialogOutcome::TryNext
    }
}

pub(crate) fn resolve_attempts(attempts: &[DialogOutcome]) -> LinuxPick {
    for attempt in attempts {
        match attempt {
            DialogOutcome::Chosen(path) => return LinuxPick::Chosen(path.clone()),
            DialogOutcome::Cancelled => return LinuxPick::Cancelled,
            DialogOutcome::TryNext => {}
        }
    }
    LinuxPick::Unavailable
}

/// Run the first installed helper. A cancel stops the search. A missing
/// display or a crash tries the next program, then reports unavailable.
pub(crate) fn run_first_dialog(
    role: PathRole,
    directory: &Path,
    suggested: Option<&str>,
) -> LinuxPick {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let mut attempts = Vec::new();
    for program in DIALOG_PROGRAMS {
        if !dirs.iter().any(|dir| dir.join(program).is_file()) {
            continue;
        }
        attempts.push(run_program(program, role, directory, suggested));
        if matches!(
            attempts.last(),
            Some(DialogOutcome::Chosen(_) | DialogOutcome::Cancelled)
        ) {
            break;
        }
    }
    resolve_attempts(&attempts)
}

fn run_program(
    program: &str,
    role: PathRole,
    directory: &Path,
    suggested: Option<&str>,
) -> DialogOutcome {
    let argv = dialog_argv(program, role, directory, suggested);
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    match command.output() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DialogOutcome::TryNext,
        Err(_) => DialogOutcome::TryNext,
        Ok(output) => interpret_dialog(
            output.status.success(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        ),
    }
}

/// Accept a typed path. Open requires an existing file. Save and export
/// require a file path whose parent directory exists.
pub(crate) fn accept_typed_path(role: PathRole, text: &str) -> Result<PathBuf, &'static str> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err("path");
    }
    let path = PathBuf::from(trimmed);
    match role {
        PathRole::Open => {
            if path.is_file() {
                Ok(path)
            } else {
                Err("path")
            }
        }
        PathRole::SaveCopy | PathRole::ExportPdf | PathRole::Print => {
            if path.is_dir() {
                return Err("path");
            }
            match path.parent() {
                Some(parent) if parent.as_os_str().is_empty() || parent.is_dir() => Ok(path),
                Some(_) => Err("path"),
                None => Ok(path),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_bar_covers_open_save_and_export() {
        assert_eq!(
            &LINUX_BAR[..4],
            &[FILE_OPEN, FILE_SAVE, FILE_SAVE_COPY, FILE_EXPORT_PDF]
        );
        assert_eq!(
            &LINUX_BAR[4..],
            &[EDIT_UNDO, EDIT_REDO, EDIT_CUT, EDIT_COPY, EDIT_PASTE]
        );
    }

    #[test]
    fn chord_labels_spell_modifiers() {
        assert_eq!(chord_label("ctrl-e"), "Ctrl+E");
        assert_eq!(chord_label("ctrl-shift-e"), "Ctrl+Shift+E");
    }

    #[test]
    fn dialog_programs_prefer_zenity_and_build_save_argv() {
        let dir = std::env::temp_dir().join(format!("joyword-dialog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kdialog"), b"").unwrap();
        assert_eq!(first_program(std::slice::from_ref(&dir)), Some("kdialog"));
        std::fs::write(dir.join("zenity"), b"").unwrap();
        assert_eq!(first_program(std::slice::from_ref(&dir)), Some("zenity"));
        assert_eq!(first_program(&[dir.join("missing")]), None);

        let open = dialog_argv("zenity", PathRole::Open, Path::new("/tmp"), None);
        assert_eq!(open[0], "zenity");
        assert!(open.iter().any(|arg| arg == "--file-selection"));
        assert!(!open.iter().any(|arg| arg == "--save"));

        let save = dialog_argv(
            "kdialog",
            PathRole::ExportPdf,
            Path::new("/tmp"),
            Some("note.pdf"),
        );
        assert_eq!(
            save,
            vec![
                "kdialog".to_string(),
                "--getsavefilename".into(),
                Path::new("/tmp").join("note.pdf").display().to_string(),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dialog_cancel_stops_and_a_display_error_tries_the_next_program() {
        assert!(matches!(
            interpret_dialog(true, "/tmp/a.docx\n", ""),
            DialogOutcome::Chosen(path) if path == Path::new("/tmp/a.docx")
        ));
        assert_eq!(interpret_dialog(false, "", ""), DialogOutcome::Cancelled);
        assert_eq!(
            interpret_dialog(false, "", "cannot open display"),
            DialogOutcome::TryNext
        );
        assert!(matches!(
            resolve_attempts(&[
                DialogOutcome::TryNext,
                DialogOutcome::Chosen(PathBuf::from("/tmp/b.docx")),
            ]),
            LinuxPick::Chosen(path) if path == Path::new("/tmp/b.docx")
        ));
        assert_eq!(
            resolve_attempts(&[DialogOutcome::Cancelled]),
            LinuxPick::Cancelled
        );
        assert_eq!(
            resolve_attempts(&[DialogOutcome::TryNext, DialogOutcome::TryNext]),
            LinuxPick::Unavailable
        );
    }

    #[test]
    fn typed_open_requires_a_file_and_save_requires_a_parent_directory() {
        let dir = std::env::temp_dir().join(format!("joyword-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.docx");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(accept_typed_path(PathRole::Open, "  ").unwrap_err(), "path");
        assert_eq!(
            accept_typed_path(PathRole::Open, file.to_str().unwrap()).unwrap(),
            file
        );
        assert!(
            accept_typed_path(PathRole::Open, dir.join("missing.docx").to_str().unwrap()).is_err()
        );
        let dest = dir.join("copy.docx");
        assert_eq!(
            accept_typed_path(PathRole::SaveCopy, dest.to_str().unwrap()).unwrap(),
            dest
        );
        let missing_parent = dir.join("nope").join("copy.docx");
        assert!(accept_typed_path(PathRole::ExportPdf, missing_parent.to_str().unwrap()).is_err());
        assert!(accept_typed_path(PathRole::SaveCopy, dir.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

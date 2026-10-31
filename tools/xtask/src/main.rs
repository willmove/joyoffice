//! Product-scoped engineering commands; no dependency on a product runtime.
use std::process::{Command, ExitCode};

fn workspace_script(start: &std::path::Path) -> Option<std::path::PathBuf> {
    start.ancestors().find_map(|directory| {
        let script = directory.join("tools/xtask/product.py");
        (directory.join("Cargo.toml").is_file() && script.is_file()).then_some(script)
    })
}

fn supported_python(candidate: &std::ffi::OsStr) -> bool {
    Command::new(candidate)
        .args([
            "-c",
            "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)",
        ])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn main() -> ExitCode {
    // A shared CARGO_TARGET_DIR can reuse this executable in another checkout.
    // Resolve the caller's workspace at runtime, never the compiled-in source path.
    let Some(script) = std::env::current_dir()
        .ok()
        .as_deref()
        .and_then(workspace_script)
    else {
        eprintln!("Cannot locate JoyOffice workspace from the current directory.");
        return ExitCode::FAILURE;
    };
    let candidates = match std::env::var_os("JOYOFFICE_PYTHON") {
        Some(value) => vec![value],
        None => [
            "python3",
            "python3.15",
            "python3.14",
            "python3.13",
            "python3.12",
            "python3.11",
            "/opt/homebrew/bin/python3",
            "python",
        ]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect(),
    };
    let Some(python) = candidates.into_iter().find(|value| supported_python(value)) else {
        eprintln!(
            "JoyOffice engineering commands require Python 3.11 or newer. Set JOYOFFICE_PYTHON to a supported interpreter."
        );
        return ExitCode::FAILURE;
    };
    match Command::new(python)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .arg(script)
        .args(std::env::args_os().skip(1))
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(1, 255) as u8),
        Err(error) => {
            eprintln!("Cannot run product engineering tool: {error}");
            ExitCode::FAILURE
        }
    }
}

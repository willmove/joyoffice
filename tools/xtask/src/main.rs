//! Product-scoped engineering commands; no dependency on a product runtime.
use std::process::{Command, ExitCode};

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
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("product.py");
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

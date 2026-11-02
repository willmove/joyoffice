#![forbid(unsafe_code)]
use std::{
    path::PathBuf,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(cwd) => cwd
            .ancestors()
            .find(|p| p.join("tools/xtask/product.py").is_file())
            .map(PathBuf::from),
        Err(_) => None,
    };
    let Some(root) = root else {
        eprintln!("Cannot locate JoyDeck workspace");
        return ExitCode::FAILURE;
    };
    let python = std::env::var_os("JOYOFFICE_PYTHON").or_else(|| {
        ["python", "python3"]
            .into_iter()
            .find(|program| {
                Command::new(program)
                    .args([
                        "-c",
                        "import sys; sys.exit(0 if sys.version_info >= (3,11) else 1)",
                    ])
                    .status()
                    .is_ok_and(|s| s.success())
            })
            .map(Into::into)
    });
    let Some(python) = python else {
        eprintln!("Python 3.11+ required");
        return ExitCode::FAILURE;
    };
    match Command::new(python)
        .arg(root.join("tools/xtask/product.py"))
        .args(std::env::args_os().skip(1))
        .current_dir(root)
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("Product command failed: {status}");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("Product command failed: {error}");
            ExitCode::FAILURE
        }
    }
}

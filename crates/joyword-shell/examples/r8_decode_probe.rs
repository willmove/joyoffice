//! Subprocess probe for scripts/joyword/fuzz-r8.py. Panics must reach the driver.
use sha2::{Digest, Sha256};
use std::{io::Read, process::ExitCode};

const LIMIT: usize = 1 << 20;

fn probe() -> Result<u8, String> {
    let mode = std::env::args().nth(1).ok_or("missing decode target")?;
    let mut data = Vec::new();
    std::io::stdin()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    if data.len() > LIMIT {
        return Err("fuzz input exceeds 1 MiB".into());
    }
    let accepted = match mode.as_str() {
        "docx" => {
            let session = joyword_shell::reader::preflight_word_package(&data).is_ok()
                && joyword_engine::io::open_bytes("fuzz.docx", &data).is_ok();
            let inspect = joyword_docx::legacy_inspection::project(&data, 1024).is_ok();
            session || inspect
        }
        "jwr-single" | "jwr-slot" => {
            joyword_shell::validate_legacy_recovery_record(&data, mode == "jwr-single").is_ok()
        }
        _ => return Err("unknown decode target".into()),
    };
    println!(
        "{}",
        serde_json::json!({
            "target": mode,
            "sha256": format!("{:x}", Sha256::digest(&data)),
            "accepted": accepted,
        })
    );
    Ok(if accepted { 0 } else { 2 })
}

fn main() -> ExitCode {
    // Match the application workers' stack budget in debug and coverage builds.
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe)
        .map_err(|error| error.to_string())
        .and_then(|worker| worker.join().map_err(|_| "decode panicked".to_owned())?);
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(70)
        }
    }
}

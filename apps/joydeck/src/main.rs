#![forbid(unsafe_code)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use std::process::ExitCode;

fn main() -> ExitCode {
    match joydeck_ui_gpui::run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("JoyDeck: {error}");
            ExitCode::FAILURE
        }
    }
}

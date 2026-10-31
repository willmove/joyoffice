#![forbid(unsafe_code)]
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
use std::process::ExitCode;
fn main() -> ExitCode {
    let mut options = joysheet_ui_gpui::LaunchOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--diagnostics" => options.diagnostics = true,
            "--sample" => match args.next().filter(|s| !s.starts_with('-')) {
                Some(sample) => options.sample = Some(sample),
                None => {
                    eprintln!("--sample requires a sample name");
                    return ExitCode::FAILURE;
                }
            },
            "--control" => match args.next().and_then(|p| p.parse::<u16>().ok()).filter(|port| *port > 0) {
                Some(port) => options.control = Some(port),
                None => {
                    eprintln!("--control requires a port between 1 and 65535");
                    return ExitCode::FAILURE;
                }
            },
            "--version" | "-V" => {
                println!("Joysheet {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!("joysheet [workbook.xlsx] [--sample sales|budget|grades] [--control PORT] [--diagnostics]");
                return ExitCode::SUCCESS;
            }
            _ if arg.starts_with('-') => {
                eprintln!("unknown option: {arg}");
                return ExitCode::FAILURE;
            }
            _ if options.path.is_none() => options.path = Some(arg),
            _ => {
                eprintln!("only one workbook path is accepted");
                return ExitCode::FAILURE;
            }
        }
    }
    match joysheet_ui_gpui::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Joysheet: {error}");
            ExitCode::FAILURE
        }
    }
}

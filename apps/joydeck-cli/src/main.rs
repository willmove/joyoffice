#![forbid(unsafe_code)]
use std::process::ExitCode;

fn execute() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => {
            println!(
                "JoyDeck CLI\n  diagnostics  Build/runtime capabilities\n  sample       Inspect the bundled original preview\n  --version    Product version"
            );
        }
        [command] if command == "--help" || command == "help" => {
            println!("Usage: joydeck-cli [diagnostics|sample|--version]");
        }
        [command] if command == "--version" => println!("JoyDeck {}", env!("CARGO_PKG_VERSION")),
        [command] if command == "diagnostics" => {
            let report = joydeck_shell::diagnostics();
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        [command] if command == "sample" => {
            let sample = joydeck_shell::PreviewDeck::bundled()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&sample).map_err(|e| e.to_string())?
            );
        }
        _ => return Err("unknown command or extra argument; use --help".into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("JoyDeck CLI: {error}");
            ExitCode::FAILURE
        }
    }
}

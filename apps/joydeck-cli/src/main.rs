#![forbid(unsafe_code)]
use std::process::ExitCode;

fn execute() -> Result<(), String> {
    execute_args(&std::env::args().skip(1).collect::<Vec<_>>())
}

fn execute_args(args: &[String]) -> Result<(), String> {
    use joydeck_shell::{Cancellation, Document};
    use std::path::Path;
    if let [command, path, rest @ ..] = args {
        if command == "inspect" && rest.is_empty() {
            let doc = Document::open(Path::new(path), &Cancellation::default())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&doc.info).map_err(|e| e.to_string())?
            );
            return Ok(());
        }
        if command == "render" {
            let mut output = None;
            let mut page = None;
            let mut all = false;
            let mut scale = 1.;
            let mut scale_set = false;
            let mut iter = rest.iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--output" if output.is_none() => {
                        output = Some(iter.next().ok_or("--output needs a new directory")?)
                    }
                    "--page" if page.is_none() => {
                        page = Some(
                            iter.next()
                                .ok_or("--page needs a number")?
                                .parse::<usize>()
                                .map_err(|_| "invalid page")?
                                .checked_sub(1)
                                .ok_or("pages start at 1")?,
                        )
                    }
                    "--all" if !all => all = true,
                    "--scale" if !scale_set => {
                        scale = iter
                            .next()
                            .ok_or("--scale needs a number")?
                            .parse::<f64>()
                            .map_err(|_| "invalid scale")?;
                        scale_set = true;
                    }
                    _ => return Err(format!("unknown or repeated option: {arg}")),
                }
            }
            if all && page.is_some() {
                return Err("use either --all or --page".into());
            }
            let output = Path::new(output.ok_or("--output NEW_DIRECTORY is required")?);
            let cancel = Cancellation::default();
            let doc = Document::open(Path::new(path), &cancel)?;
            let pages = if all {
                (0..doc.info.slides.len()).collect()
            } else {
                vec![page.unwrap_or(0)]
            };
            joydeck_shell::document::render_to_directory(&doc, output, &pages, scale, &cancel)?;
            println!("Rendered {} page(s) to {}", pages.len(), output.display());
            return Ok(());
        }
    }
    match args {
        [] => {
            println!(
                "JoyDeck CLI · R1\n  inspect INPUT.pptx|INPUT.deckcraft\n  render INPUT --output NEW_DIRECTORY [--all|--page N] [--scale 1]\n  diagnostics | sample | --version"
            );
        }
        [command] if command == "--help" || command == "help" => {
            println!(
                "Usage: joydeck-cli inspect INPUT\n       joydeck-cli render INPUT --output NEW_DIRECTORY [--all|--page N] [--scale 1]\n       joydeck-cli [diagnostics|sample|--version]\nOutputs slide-NNNN.png and inspect.json. Existing directories are refused."
            );
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

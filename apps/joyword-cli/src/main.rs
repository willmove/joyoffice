mod epub;
mod exchange;
mod inspect;
mod merge;
mod visual;

use joyword_shell::reader::{Reader, publish_new};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::AtomicBool,
};

fn report(mut value: serde_json::Value) -> Result<(), String> {
    use std::io::Write;
    inspect::canonicalize_keys(&mut value);
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, &value).map_err(|error| error.to_string())?;
    writeln!(output).map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}

fn argument(args: &[OsString], index: usize) -> Result<&str, String> {
    args.get(index)
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Missing or invalid argument; use --help".into())
}
fn path(args: &[OsString], index: usize) -> Result<PathBuf, String> {
    args.get(index)
        .map(PathBuf::from)
        .ok_or_else(|| "Missing path; use --help".into())
}
fn run(args: &[OsString]) -> Result<(), String> {
    let command = args.first().and_then(|s| s.to_str()).unwrap_or("--help");
    let cancel = AtomicBool::new(false);
    match command {
        "mcp" => {
            if let [_, flag, address] = args
                && flag == "--connect"
            {
                let addr: std::net::SocketAddrV4 = address
                    .to_str()
                    .ok_or("invalid control address")?
                    .parse()
                    .map_err(|_| "use 127.0.0.1:PORT")?;
                if *addr.ip() != std::net::Ipv4Addr::LOCALHOST || addr.port() == 0 {
                    return Err("use 127.0.0.1:PORT".into());
                }
                let backend = joyword_mcp::Remote::lazy(&addr.to_string()).document_only();
                return joyword_mcp::Server::new(Box::new(backend))
                    .serve(std::io::stdin().lock(), std::io::stdout().lock())
                    .map_err(|e| e.to_string());
            }
            let (mut read, mut write) = (None, None);
            let mut i = 1;
            while i < args.len() {
                match argument(args, i)? {
                    "--automation-read-root" => read = Some(path(args, i + 1)?),
                    "--automation-write-root" => write = Some(path(args, i + 1)?),
                    other => return Err(format!("Unknown MCP option: {other}")),
                }
                i += 2;
            }
            let roots = joyword_engine::file_access::AutomationRoots::new(
                read.as_deref(),
                write.as_deref(),
            )?
            .ok_or("MCP requires an explicit --automation-read-root or --automation-write-root")?;
            if !joyword_engine::file_access::confine_process(roots.clone()) {
                return Err("Automation roots already set".into());
            }
            let backend = joyword_mcp::Headless::default().with_automation_roots(Some(roots));
            joyword_mcp::Server::new(Box::new(backend))
                .serve(std::io::stdin().lock(), std::io::stdout().lock())
                .map_err(|e| e.to_string())?;
        }
        "--version" | "-V" => println!("JoyWord {}", env!("CARGO_PKG_VERSION")),
        "--help" | "-h" => {
            println!(
                "JoyWord\ncommands | capabilities | inspect|text [--max-text-bytes N] [--compatibility] [--] INPUT.docx\ninspect|text --session [--] INPUT\nrecover JOURNAL [--save NEW.docx]\nconvert INPUT OUTPUT.pdf|.png|.txt|.md|.html|.rtf|.odt|.ott|.fodt|.tex|.xml|.docx\nrender INPUT.docx OUTPUT.png [--page 1] [--scale 1.333]\nrender INPUT.docx --font PATH:FACE --output DIR [--dpi N] [--page N]\nconvert INPUT.docx --to pdf|pdfa-2b --font PATH:FACE --output FILE.pdf [--archive-year YYYY]\nmerge --template DOCX --data CSV [--map FIELD=COLUMN] (--preview-row N | --output-parent DIR)\nexchange INPUT --from FORMAT --to FORMAT (--plan | --output FILE [--accept-losses])\nsample OUTPUT.docx\nmcp --automation-read-root DIR [--automation-write-root DIR]\nrun INPUT.docx --cmd COMMAND=JSON [--save NEW.docx]\nOutputs must be new paths; desktop editing and safe DOCX save are available."
            );
            println!("{}", epub::USAGE);
            println!("mcp --connect 127.0.0.1:PORT");
        }
        "capabilities" => println!(
            "{}",
            serde_json::json!({"product":"joyword","stage":"R8","desktop":"gpui-kit","docxOpen":true,"readOnlyDesktop":false,"documentEditing":true,"ribbonFormatting":true,"stylePanel":true,"formatPainter":true,"tableEditing":true,"pictureEditing":true,"references":true,"comments":true,"trackChanges":true,"compare":true,"mailMerge":true,"formatExchange":true,"sessionMacros":true,"mcpStdio":true,"mcpDesktopBridge":true,"mcpDesktopUiAutomation":false,"pdfExport":true,"headlessEngine":true,"save":"CLI new output only"})
        ),
        "commands" => {
            let session =
                joyword_engine::Session::new(joyword_engine::blank_document(&Default::default()));
            println!("{}", session.registry.describe());
        }
        "sample" => {
            let doc = joyword_engine::sample::sample_document();
            let bytes = joyword_docx::write(&doc).map_err(|e| e.to_string())?;
            publish_new(&path(args, 1)?, &bytes)?;
        }
        "inspect" | "text" => {
            let inputs = match &args[2..] {
                [end, rest @ ..] if end == "--" => rest,
                rest => rest,
            };
            let [input] = inputs else {
                return Err(format!("Usage: {command} --session [--] INPUT"));
            };
            if inputs.len() == args.len() - 2 && input.to_string_lossy().starts_with('-') {
                return Err(format!("Usage: {command} --session [--] INPUT"));
            }
            let reader = Reader::open(Path::new(input), &cancel)?;
            let inspection = reader.inspection();
            if command == "text" {
                print!("{}", inspection.text);
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&inspection).map_err(|e| e.to_string())?
                );
            }
        }
        "convert" | "render" => {
            let reader = Reader::open(&path(args, 1)?, &cancel)?;
            let output = path(args, 2)?;
            let ext = output
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let bytes = if command == "render" || ext == "png" {
                let mut page = 0;
                let mut scale = 96.0 / 72.0;
                let mut i = 3;
                while i < args.len() {
                    match argument(args, i)? {
                        "--page" => {
                            page = argument(args, i + 1)?
                                .parse::<usize>()
                                .ok()
                                .and_then(|p| p.checked_sub(1))
                                .ok_or("Page is 1-based")?
                        }
                        "--scale" => {
                            scale = argument(args, i + 1)?
                                .parse::<f32>()
                                .map_err(|e| e.to_string())?
                        }
                        other => return Err(format!("Unknown render option {other}")),
                    }
                    i += 2;
                }
                let bytes = reader.raster(page, scale, &cancel)?.to_png();
                if bytes.is_empty() {
                    return Err("PNG encoding failed".into());
                }
                bytes
            } else {
                if args.len() != 3 {
                    return Err("Usage: convert INPUT OUTPUT".into());
                }
                reader.export_job(&ext, false)?.bytes()?
            };
            publish_new(&output, &bytes)?;
            println!(
                "{}",
                serde_json::json!({"output":output,"pages":reader.summary.pages.len(),"fingerprint":reader.summary.fingerprint})
            );
        }
        "recover" => {
            let mut inputs = &args[1..];
            if inputs.first().is_some_and(|arg| arg == "--") {
                inputs = &inputs[1..];
            }
            let (journal, output) = match inputs {
                [journal] => (journal, None),
                [journal, flag, output] if flag == "--save" => (journal, Some(output)),
                _ => return Err("Usage: recover [--] JOURNAL [--save NEW.docx]".into()),
            };
            let reader = Reader::recover_legacy(Path::new(journal), &cancel)?;
            if let Some(notice) = reader.recovery_notice {
                eprintln!("{notice}");
            }
            if let Some(output) = output {
                reader.save_job(PathBuf::from(output), true)?.run()?;
            }
            println!(
                "{}",
                serde_json::to_string(&reader.inspection()).map_err(|e| e.to_string())?
            );
        }
        "run" => {
            let mut reader = Reader::open(&path(args, 1)?, &cancel)?;
            reader.require_editable_source()?;
            let mut save = None;
            let mut i = 2;
            while i < args.len() {
                match argument(args, i)? {
                    "--cmd" => {
                        let spec = argument(args, i + 1)?;
                        let (id, json) = spec.split_once('=').unwrap_or((spec, "{}"));
                        let params = serde_json::from_str(json).map_err(|e| e.to_string())?;
                        reader.execute(id, &params)?;
                    }
                    "--save" => save = Some(path(args, i + 1)?),
                    other => return Err(format!("Unknown run option {other}")),
                }
                i += 2;
            }
            if let Some(output) = save {
                if !Path::new(&output)
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("docx"))
                {
                    return Err("--save requires a new DOCX path".into());
                }
                publish_new(
                    &output,
                    &joyword_docx::write(&reader.engine().doc).map_err(|e| e.to_string())?,
                )?;
            }
            println!(
                "{}",
                serde_json::to_string(&reader.inspection()).map_err(|e| e.to_string())?
            );
        }
        other => return Err(format!("Unsupported command: {other}")),
    }
    Ok(())
}
fn main() -> ExitCode {
    // Match the desktop's separation of document work from its entry thread.
    // Debug builds can exhaust the default Windows main stack while loading
    // bounded nested OOXML and laying out the first page.
    let result = std::thread::Builder::new()
        .name("joyword-cli".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let args: Vec<_> = std::env::args_os().skip(1).collect();
            let command = args.first().and_then(|arg| arg.to_str());
            let legacy_inspection = matches!(command, Some("inspect" | "text"))
                && args.get(1).is_none_or(|arg| arg != "--session");
            let legacy_visual =
                matches!(command, Some("render" | "convert")) && visual::is_legacy(&args[1..]);
            if legacy_inspection
                || legacy_visual
                || matches!(command, Some("epub" | "exchange" | "merge"))
            {
                let result = joyword_shell::reader::guarded(|| match command {
                    Some("epub") => epub::run(&args[1..]),
                    Some("exchange") => exchange::run(&args[1..]),
                    Some("merge") => merge::run(&args[1..]),
                    Some("render") => visual::run(visual::Command::Render, &args[1..]),
                    Some("convert") => visual::run(visual::Command::Convert, &args[1..]),
                    _ => {
                        inspect::run(command == Some("inspect"), &args[1..]).map_err(str::to_owned)
                    }
                });
                Ok(match result {
                    Ok(status) => status,
                    Err(error) => {
                        use std::io::Write;
                        let _ = writeln!(std::io::stderr().lock(), "joyword-cli: {error}");
                        2
                    }
                })
            } else {
                joyword_shell::reader::guarded(|| run(&args)).map(|()| 0)
            }
        })
        .map_err(|error| format!("Cannot start JoyWord worker: {error}"))
        .and_then(|worker| {
            worker
                .join()
                .map_err(|_| "JoyWord worker failed".to_string())?
        });
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

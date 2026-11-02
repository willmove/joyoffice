#![forbid(unsafe_code)]
use std::process::ExitCode;
mod output;
mod scene_bench;
mod stdio;

fn execute() -> Result<(), String> {
    execute_args(&std::env::args().skip(1).collect::<Vec<_>>())
}

fn execute_args(args: &[String]) -> Result<(), String> {
    if let [command, input, mode_flag, mode, output_flag, output] = args
        && command == "exchange"
        && mode_flag == "--mode"
        && output_flag == "--output"
    {
        let editor = joydeck_shell::editor::Editor::open(
            std::path::Path::new(input),
            joydeck_shell::editor::recovery_root(),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&joydeck_shell::exchange::export(
                &editor,
                std::path::Path::new(output),
                mode,
                &Default::default()
            )?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let [command, flag, root] = args
        && matches!(command.as_str(), "control" | "mcp")
        && flag == "--root"
    {
        return stdio::run(std::path::Path::new(root), command == "mcp");
    }
    if args.first().is_some_and(|s| s == "export") {
        return output::run(&args[1..]);
    }
    if let [command, input] = args
        && command == "print"
    {
        let cancel = joydeck_shell::Cancellation::default();
        let doc = joydeck_shell::Document::open(std::path::Path::new(input), &cancel)?;
        println!(
            "{}",
            joydeck_shell::output::print(
                &doc,
                &joydeck_shell::output::Options::default(),
                &cancel
            )?
        );
        return Ok(());
    }
    if let [command, input] = args
        && command == "media-inspect"
    {
        println!(
            "{}",
            serde_json::to_string_pretty(&joydeck_shell::playback::inspect(std::path::Path::new(
                input
            ))?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let [command, input, flag, output] = args
        && command == "migrate"
        && flag == "--output"
    {
        if std::path::Path::new(output)
            .extension()
            .and_then(|e| e.to_str())
            != Some("joydeck")
        {
            return Err("migration output must be a new .joydeck file".into());
        }
        let mut editor = joydeck_shell::editor::Editor::open(
            std::path::Path::new(input),
            joydeck_shell::editor::recovery_root(),
        )?;
        let saved = editor.save_job(output.into())?.run()?;
        editor.apply_saved(saved);
        println!(
            "{}",
            serde_json::to_string_pretty(&editor.display()?.info).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    use joydeck_shell::{Cancellation, Document};
    use std::path::Path;
    if let [
        command,
        input,
        page_flag,
        page,
        count_flag,
        count,
        output_flag,
        output,
    ] = args
        && command == "scene-bench"
        && page_flag == "--page"
        && count_flag == "--iterations"
        && output_flag == "--output"
    {
        return scene_bench::run(
            Path::new(input),
            page.parse().map_err(|_| "invalid page")?,
            count.parse().map_err(|_| "invalid iteration count")?,
            Path::new(output),
        );
    }
    if let [command, input] = args
        && command == "content"
    {
        let doc = Document::open(Path::new(input), &Cancellation::default())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&doc.presentation).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let [command, input, picture, flag, output] = args
        && command == "import-picture"
        && flag == "--output"
    {
        let mut editor = joydeck_shell::editor::Editor::open(
            Path::new(input),
            joydeck_shell::editor::recovery_root(),
        )?;
        editor
            .session
            .execute(joydeck_shell::platform::picture_command(
                Path::new(picture),
                false,
            )?)?;
        let saved = editor.save_job(output.into())?.run()?;
        editor.apply_saved(saved);
        println!(
            "{}",
            serde_json::to_string_pretty(&editor.display()?.info).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let [command] = args
        && command == "commands"
    {
        println!(
            "{}",
            serde_json::to_string_pretty(joydeck_engine::COMMANDS).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let [command, root] = args
        && command == "recovery-list"
    {
        let recovery = joydeck_shell::storage::Recovery::new(root.into());
        let entries = recovery.list()?;
        let rows = entries
            .iter()
            .map(|e| serde_json::json!({"record":e.record,"active":e.active,"problem":e.problem}))
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if let [command, root, session, flag, output] = args
        && command == "recover"
        && flag == "--output"
    {
        let recovery = joydeck_shell::storage::Recovery::new(root.into());
        let entry = recovery
            .list()?
            .into_iter()
            .find(|e| &e.record.session == session && !e.active && e.problem.is_none())
            .ok_or("recovery entry unavailable")?;
        let editor = joydeck_shell::editor::Editor::restore(root.into(), &entry.record)?;
        editor.save_job(output.into())?.run()?;
        println!("Recovered to {output}");
        return Ok(());
    }
    if let [command, input, script_flag, script, output_flag, output] = args
        && command == "edit"
        && script_flag == "--script"
        && output_flag == "--output"
    {
        let mut editor = if input == "--new" {
            joydeck_shell::editor::Editor::new(joydeck_shell::editor::recovery_root())?
        } else {
            joydeck_shell::editor::Editor::open(
                Path::new(input),
                joydeck_shell::editor::recovery_root(),
            )?
        };
        let bytes = joydeck_shell::platform::read_bounded(Path::new(script), 1 << 20)?;
        let commands: Vec<joydeck_engine::Command> =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if commands.len() > 1000 {
            return Err("command batch exceeds 1000 steps".into());
        }
        for command in commands {
            editor.session.execute(command)?;
        }
        let saved = editor.save_job(output.into())?.run()?;
        editor.apply_saved(saved);
        println!(
            "{}",
            serde_json::to_string_pretty(&editor.display()?.info).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    // Probe is an explicit CLI operation: writes recovery, then remains alive for crash tests.
    if let [command, input, script, root] = args
        && command == "checkpoint-probe"
    {
        use std::io::Write;
        let mut editor = joydeck_shell::editor::Editor::open(Path::new(input), root.into())?;
        let bytes = joydeck_shell::platform::read_bounded(Path::new(script), 1 << 20)?;
        let commands: Vec<joydeck_engine::Command> =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        for command in commands {
            editor.session.execute(command)?;
        }
        editor.checkpoint_job().checkpoint()?;
        println!("READY {}", editor.recovery.session);
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        loop {
            std::thread::park();
        }
    }
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
                "JoyDeck CLI · R6/R7\n  migrate LEGACY.joydeck --output NEW.joydeck\n  exchange INPUT --mode preserve|regenerate --output NEW.pptx|NEW.potx|NEW.ppsx\n  export INPUT --format pdf|png|jpeg|gif|mp4|outline|rtf|vtt --output NEW_PATH [--pages 1,3] [--dpi 144] [--scale 1] [--layout slides|notes|handouts-3|handouts-6] [--seconds 1] [--fps 10] [--transparent]\n  print INPUT | media-inspect LOCAL_MEDIA\n  control --root ABS_DIRECTORY | mcp --root ABS_DIRECTORY\n  inspect INPUT.pptx|INPUT.deckcraft\n  render INPUT --output NEW_DIRECTORY [--all|--page N] [--scale 1]\n  edit INPUT|--new --script JSON --output NEW.pptx|NEW.joydeck\n  content INPUT\n  import-picture INPUT IMAGE --output NEW.pptx|NEW.joydeck\n  scene-bench INPUT --page N --iterations N --output NEW_DIRECTORY\n  commands | recovery-list DIR | recover DIR SESSION --output NEW.joydeck\n  diagnostics | sample | --version"
            );
        }
        [command] if command == "--help" || command == "help" => {
            println!(
                "JoyDeck R6/R7: use joydeck-cli without arguments for the complete command list.\nUsage: joydeck-cli inspect INPUT\n       joydeck-cli render INPUT --output NEW_DIRECTORY [--all|--page N] [--scale 1]\n       joydeck-cli [diagnostics|sample|--version]\nOutputs slide-NNNN.png and inspect.json. Existing directories are refused."
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

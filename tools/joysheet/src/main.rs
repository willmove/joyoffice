mod distribution;
mod licenses;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap_or(Path::new(".")).to_path_buf()
}
fn cargo(args: &[&str]) -> Result<(), String> {
    eprintln!("cargo {}", args.join(" "));
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(args)
        .current_dir(root())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed ({status})", args.join(" "))) }
}
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("dist") => dist(&args),
        Some("inventory") => inventory(),
        Some("smoke") => smoke(&args),
        Some("layers") => layers(),
        Some("wasm") => wasm(),
        Some("bench") => bench(),
        _ => {
            eprintln!("cargo xtask ci | dist [--no-build] | smoke --stage M0..M8 [--engine-only] | inventory | layers | wasm | bench");
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
fn ci() -> Result<(), String> {
    cargo(&["fmt", "--all", "--", "--check"])?;
    cargo(&["clippy", "--workspace", "--all-targets", "--features", "joysheet/automation", "--locked", "--", "-D", "warnings"])?;
    cargo(&["test", "--workspace", "--features", "joysheet/automation", "--locked"])?;
    layers()
}
fn layers() -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["tree", "--workspace", "--locked", "--prefix", "none"])
        .current_dir(root())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let tree = String::from_utf8_lossy(&output.stdout);
    for line in tree.lines() {
        let name = line.split_whitespace().next().unwrap_or_default();
        if name == "egui" || name == "eframe" || name.starts_with("egui-") {
            return Err(format!("legacy UI dependency: {line}"));
        }
    }
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
        .current_dir(root())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    for package in metadata["packages"].as_array().into_iter().flatten() {
        let name = package["name"].as_str().unwrap_or_default();
        if name.starts_with("joysheet-") {
            for dep in package["dependencies"].as_array().into_iter().flatten() {
                let dependency = dep["name"].as_str().unwrap_or_default();
                if dependency.starts_with("joysheet-") || dependency.starts_with("gpui") || ["rfd", "arboard", "winit"].contains(&dependency) {
                    return Err(format!("layer violation: {name} depends on {dependency}"));
                }
            }
        }
    }
    println!("Layer checks passed; no egui/eframe in the workspace graph.");
    Ok(())
}
fn wasm() -> Result<(), String> {
    cargo(&["check", "--locked", "--target", "wasm32-unknown-unknown", "-p", "joysheet-engine", "-p", "joysheet-chart", "-p", "joysheet-pdf"])
}
fn smoke(args: &[String]) -> Result<(), String> {
    let stage = args.windows(2).find(|w| w.first().is_some_and(|s| s == "--stage")).and_then(|w| w.get(1)).map(String::as_str).unwrap_or("M3");
    if !["M0", "M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8"].contains(&stage) {
        return Err("unknown milestone".into());
    }
    let output = root().join("artifacts").join(stage);
    let engine = joysheet_app::smoke::run(&output)?;
    let engine_only = args.iter().any(|a| a == "--engine-only");
    if !engine_only {
        if !cfg!(target_os = "windows") {
            return Err("Native automation harness currently targets Windows; use --engine-only for headless core checks.".into());
        }
        cargo(&["build", "--locked", "-p", "joysheet", "-p", "joysheet-cli", "--features", "joysheet/automation"])?;
        let status = Command::new("pwsh.exe")
            .args([
                "-NoProfile",
                "-File",
                "scripts/desktop-smoke.ps1",
                "-Stage",
                if ["M4", "M5", "M6", "M7", "M8"].contains(&stage) { "M3" } else { stage },
                "-M4Task",
                if ["M4", "M5", "M6", "M7", "M8"].contains(&stage) { "4" } else { "0" },
                "-M5Task",
                if ["M5", "M6", "M7", "M8"].contains(&stage) { "4" } else { "0" },
                "-M6Task",
                if ["M6", "M7", "M8"].contains(&stage) { "4" } else { "0" },
                "-M7Task",
                if ["M7", "M8"].contains(&stage) { "4" } else { "0" },
                "-M8Task",
                if stage == "M8" { "4" } else { "0" },
                "-WindowsTask",
                if stage == "M8" { "4" } else { "0" },
                "-WindowsQualityTask",
                if stage == "M8" { "4" } else { "0" },
            ])
            .current_dir(root())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("native desktop smoke failed".into());
        }
    }
    let report = json!({"stage":stage,"engine":engine,
        "nativeRuntime":if engine_only{"not run"}else{"Windows production GPUI view and native GPU readback passed"},
        "milestoneComplete":false});
    std::fs::write(output.join("engine-smoke.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
fn bench() -> Result<(), String> {
    use joysheet_app::geometry::{GridLayout, SheetGeometry};
    let sh = joysheet_sheet();
    let geo = SheetGeometry::new(&sh);
    let start = std::time::Instant::now();
    let mut max_cells = 0;
    let mut frames = Vec::new();
    for row in (0..1_000_000u32).step_by(997) {
        let frame = std::time::Instant::now();
        let layout = GridLayout::new(&sh, &geo, 0.0, geo.rows.start(row), 1600.0, 900.0);
        max_cells = max_cells.max(layout.painted_cell_bound());
        std::hint::black_box(layout);
        frames.push(frame.elapsed().as_secs_f64() * 1000.0);
    }
    frames.sort_by(f64::total_cmp);
    let p95 = frames.get(frames.len() * 95 / 100).copied().unwrap_or_default();
    let report = json!({"kind":"geometry-only sparse-sheet benchmark","frames":frames.len(),"maxVisibleCells":max_cells,
        "layoutP95Ms":p95,"totalMs":start.elapsed().as_secs_f64()*1000.0,"gpuFrameTimeMeasured":false});
    std::fs::create_dir_all(root().join("artifacts")).map_err(|e| e.to_string())?;
    std::fs::write(root().join("artifacts/geometry-benchmark.json"), report.to_string()).map_err(|e| e.to_string())?;
    println!("{report}");
    Ok(())
}
fn joysheet_sheet() -> joysheet_engine::model::Sheet {
    joysheet_engine::model::Sheet::new("Sparse million")
}
fn inventory() -> Result<(), String> {
    let specs = joysheet_engine::command_specs();
    let registered:Vec<_>=specs.iter().map(|s|json!({"id":s.id,"label":s.label,"menu":s.menu,"params":s.params,"undoable":s.undoable,"interface":"routed by command center and CLI; dedicated GPUI workflow verification tracked in MIGRATION.md"})).collect();
    let catalog_only: Vec<_> =
        joysheet_engine::catalog::CATALOG.iter().filter(|(id, _, _, _)| !specs.iter().any(|s| s.id == *id)).map(|(id, _, _, _)| *id).collect();
    let report =
        json!({"registeredCount":registered.len(),"registered":registered,"catalogOnlyNotCounted":catalog_only,"desktopParityComplete":false});
    std::fs::write(root().join("docs/commands.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("Recorded {} registered engine commands; catalog-only entries excluded.", specs.len());
    Ok(())
}
fn files_under(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            files.extend(files_under(&path)?);
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
fn dist(args: &[String]) -> Result<(), String> {
    use std::io::{Read, Write};
    if !args.iter().any(|a| a == "--no-build") {
        cargo(&["build", "--release", "--locked", "-p", "joysheet", "-p", "joysheet-cli", "--features", "joysheet/automation"])?;
    }
    let label = format!("joysheet-{}-M8-preview6-{}-{}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);
    let directory = root().join("dist").join(&label);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let ext = std::env::consts::EXE_SUFFIX;
    for name in ["joysheet", "joysheet-cli"] {
        let name = format!("{name}{ext}");
        std::fs::copy(root().join("target/release").join(&name), directory.join(name)).map_err(|e| e.to_string())?;
    }
    for name in ["README.md", "LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md"] {
        std::fs::copy(root().join(name), directory.join(name)).map_err(|e| e.to_string())?;
    }
    for name in ["STATUS.md", "MIGRATION.md", "VALIDATION.md", "UPSTREAM-PATCHES.md"] {
        std::fs::copy(root().join("docs").join(name), directory.join(name)).map_err(|e| e.to_string())?;
    }
    for path in files_under(&root().join("docs"))? {
        let relative = path.strip_prefix(root()).map_err(|e| e.to_string())?.to_path_buf();
        let destination = directory.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(path, destination).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(directory.join("assets")).map_err(|e| e.to_string())?;
    for path in files_under(&root().join("assets"))? {
        std::fs::copy(&path, directory.join("assets").join(path.file_name().ok_or("Invalid asset")?)).map_err(|e| e.to_string())?;
    }
    licenses::collect(&root(), &directory.join("licenses"))?;

    let manifest = json!({"version":env!("CARGO_PKG_VERSION"),"stage":"M8-preview6","milestoneComplete":false,"profile":"release","gpuiKit":"0.8.1","gpuiSnapshot":"0.3.10","rust":"1.98.1","os":std::env::consts::OS,"arch":std::env::consts::ARCH,"automation":true,"publisherSigned":false,"sourceManifest":"docs/source-checkpoint.json","windowsRuntime":"MSVC x64 CRT copied app-local; built-in Windows PDF renderer; PowerShell 7 needed only for the native print dialog (not bundled)"});
    std::fs::write(directory.join("BUILD.json"), serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if cfg!(target_os = "windows") {
        std::fs::create_dir_all(directory.join("scripts")).map_err(|e| e.to_string())?;
        std::fs::copy(root().join("scripts/desktop-smoke.ps1"), directory.join("scripts/desktop-smoke.ps1")).map_err(|e| e.to_string())?;
        std::fs::copy(root().join("scripts/stress-cases.ps1"), directory.join("scripts/stress-cases.ps1")).map_err(|e| e.to_string())?;
        std::fs::write(directory.join("Run dense sheet benchmark.cmd"),
            "@echo off\r\ncd /d \"%~dp0\"\r\npwsh.exe -NoProfile -File \"scripts\\desktop-smoke.ps1\" -BinaryDirectory \".\" -Stage M1 -StressCases %*\r\npause\r\n"
        ).map_err(|e| e.to_string())?;

        std::fs::write(
            directory.join("Run performance benchmark.cmd"),
            "@echo off\r\ncd /d \"%~dp0\"\r\npwsh.exe -NoProfile -File \"scripts\\desktop-smoke.ps1\" -BinaryDirectory \".\" -Stage M1 -Performance -FormulaCases %*\r\npause\r\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            directory.join("Open advanced workflow.cmd"),
            "@echo off\r\ncd /d \"%~dp0\"\r\njoysheet.exe \"docs\\samples\\M7-workflow.xlsx\"\r\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            directory.join("Open data workflow.cmd"),
            "@echo off\r\ncd /d \"%~dp0\"\r\njoysheet.exe \"docs\\samples\\M5-workflow.xlsx\"\r\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(directory.join("Open sales sample.cmd"), "@echo off\r\ncd /d \"%~dp0\"\r\njoysheet.exe --sample sales\r\n")
            .map_err(|e| e.to_string())?;
    }
    if cfg!(target_os = "windows") {
        std::fs::write(
            directory.join("Open drawing workflow.cmd"),
            "@echo off\r\ncd /d \"%~dp0\"\r\njoysheet.exe \"docs\\samples\\M6-workflow.xlsx\"\r\n",
        )
        .map_err(|e| e.to_string())?;
    }
    if cfg!(target_os = "windows") {
        for (name, sample) in [
            ("Open complex formatting.cmd", "Windows-format.xlsx"),
            ("Open Windows boundaries.cmd", "Windows-boundaries.xlsx"),
            ("Open Windows quality.cmd", "Windows-quality.xlsx"),
        ] {
            std::fs::write(directory.join(name), format!("@echo off\r\ncd /d \"%~dp0\"\r\njoysheet.exe \"docs\\samples\\{sample}\"\r\n"))
                .map_err(|e| e.to_string())?;
        }
    }
    distribution::native(&root(), &directory, &label)?;
    let file = std::fs::File::create(root().join("dist").join(format!("{label}.zip"))).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipWriter::new(file);
    for path in files_under(&directory)? {
        let mut bytes = Vec::new();
        std::fs::File::open(&path).map_err(|e| e.to_string())?.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let name = path.strip_prefix(&directory).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        let mode = if name.starts_with("joysheet") || name.ends_with("/joysheet") || name.ends_with("/joysheet-cli") || name.ends_with("/AppRun") {
            0o755
        } else {
            0o644
        };
        archive.start_file(format!("{label}/{name}"), zip::write::SimpleFileOptions::default().unix_permissions(mode)).map_err(|e| e.to_string())?;
        archive.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    archive.finish().map_err(|e| e.to_string())?;
    let checkpoint = root().join("docs/source-checkpoint.json");
    if checkpoint.is_file() {
        let snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&checkpoint).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(root().join("dist").join(format!("joysheet-{}-M8-preview6-source.zip", env!("CARGO_PKG_VERSION"))))
            .map_err(|e| e.to_string())?;
        let mut source_zip = zip::ZipWriter::new(file);
        let mut paths: Vec<_> = snapshot["files"].as_array().into_iter().flatten().filter_map(|f| f["path"].as_str().map(str::to_string)).collect();
        paths.push("docs/source-checkpoint.json".into());
        for name in paths {
            let relative = Path::new(&name);
            if relative.is_absolute() || relative.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return Err("invalid source archive path".into());
            }
            source_zip
                .start_file(format!("joysheet/{name}"), zip::write::SimpleFileOptions::default().unix_permissions(0o644))
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut std::fs::File::open(root().join(relative)).map_err(|e| e.to_string())?, &mut source_zip).map_err(|e| e.to_string())?;
        }
        source_zip.finish().map_err(|e| e.to_string())?;
    }
    println!("Portable build: {}", directory.display());
    Ok(())
}

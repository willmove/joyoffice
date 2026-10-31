//! Product-specific engine smoke, inventory and geometry benchmark tools.
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("smoke") => smoke(&args),
        Some("inventory") => inventory(),
        Some("bench") => bench(),
        _ => Err("expected smoke --stage M0..M8, inventory or bench".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
fn smoke(args: &[String]) -> Result<(), String> {
    let stage = args.windows(2).find(|w| w.first().is_some_and(|s| s == "--stage")).and_then(|w| w.get(1)).map(String::as_str).unwrap_or("M8");
    if !["M0", "M1", "M2", "M3", "M4", "M5", "M6", "M7", "M8"].contains(&stage) {
        return Err("unknown milestone".into());
    }
    let output = root().join("artifacts/joysheet").join(stage);
    let engine = joysheet_app::smoke::run(&output)?;
    let report = json!({"stage":stage,"engine":engine,"nativeRuntime":"not run by engine helper","milestoneComplete":false});
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
    std::fs::create_dir_all(root().join("artifacts/joysheet")).map_err(|e| e.to_string())?;
    std::fs::write(root().join("artifacts/joysheet/geometry-benchmark.json"), report.to_string()).map_err(|e| e.to_string())?;
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
    std::fs::write(root().join("docs/joysheet/commands.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("Recorded {} registered engine commands; catalog-only entries excluded.", specs.len());
    Ok(())
}

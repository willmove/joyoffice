use joydeck_shell::{
    Cancellation, Document,
    output::{self, Options},
};
use std::path::Path;
pub fn run(args: &[String]) -> Result<(), String> {
    let input = args
        .first()
        .ok_or("export INPUT --format pdf|png|jpeg|gif|mp4|outline|rtf|vtt --output NEW_PATH")?;
    let mut opts = Options::default();
    let mut destination = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut iter = args[1..].iter();
    while let Some(flag) = iter.next() {
        if !seen.insert(flag) {
            return Err(format!("repeated option: {flag}"));
        }
        if flag == "--transparent" {
            opts.transparent = true;
            continue;
        }
        let value = iter.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--format" => opts.format = value.clone(),
            "--output" => destination = Some(value),
            "--pages" => {
                opts.pages = Some(
                    value
                        .split(',')
                        .map(|v| {
                            v.parse::<usize>()
                                .ok()
                                .and_then(|n| n.checked_sub(1))
                                .ok_or_else(|| "invalid page list, starts at 1".into())
                        })
                        .collect::<Result<_, String>>()?,
                )
            }
            "--scale" => opts.scale = value.parse().map_err(|_| "invalid scale")?,
            "--dpi" => opts.dpi = value.parse().map_err(|_| "invalid DPI")?,
            "--seconds" => opts.seconds = value.parse().map_err(|_| "invalid seconds")?,
            "--fps" => opts.fps = value.parse().map_err(|_| "invalid FPS")?,
            "--layout" => {
                opts.layout = serde_json::from_value(match value.as_str() {
                    "slides" => serde_json::json!({"kind":"slides"}),
                    "notes" => serde_json::json!({"kind":"notes"}),
                    "handouts-3" => serde_json::json!({"kind":"handouts","per_page":3}),
                    "handouts-6" => serde_json::json!({"kind":"handouts","per_page":6}),
                    _ => return Err("layout is slides/notes/handouts-3/handouts-6".into()),
                })
                .map_err(|e| e.to_string())?
            }
            _ => return Err(format!("unknown export option: {flag}")),
        }
    }
    let cancel = Cancellation::default();
    let doc = Document::open(Path::new(input), &cancel)?;
    let report = output::export(
        &doc,
        Path::new(destination.ok_or("--output required")?),
        &opts,
        &cancel,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

use joyword_shell::epub;
use serde_json::json;
use std::{ffi::OsString, path::Path, sync::atomic::AtomicBool};

pub const USAGE: &str = "epub INPUT.docx (--plan | --output NEW.epub --accept-losses) [--title TITLE] [--language TAG] [--modified UTC]";

pub fn run(args: &[OsString]) -> Result<u8, String> {
    let mut args = args.iter();
    let input = args.next().ok_or(USAGE)?;
    if input == "--help" && args.next().is_none() {
        println!("{USAGE}");
        return Ok(0);
    }
    let input = Path::new(input);
    let (mut output, mut title, mut lang, mut modified) = (None, None, None, None);
    let (mut plan, mut accept) = (false, false);
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--plan") if !plan => plan = true,
            Some("--accept-losses") if !accept => accept = true,
            Some("--output") if output.is_none() => {
                output = Some(Path::new(args.next().ok_or("--output needs a path")?));
            }
            Some("--title") if title.is_none() => title = Some(option_text(&mut args)?),
            Some("--language") if lang.is_none() => lang = Some(option_text(&mut args)?),
            Some("--modified") if modified.is_none() => modified = Some(option_text(&mut args)?),
            _ => return Err("unknown or duplicate EPUB option".into()),
        }
    }
    if plan == output.is_some() || (plan && accept) {
        return Err(USAGE.into());
    }
    let doc = epub::open_docx(input)?;
    let fallback = input.file_stem().unwrap_or_default().to_string_lossy();
    let modified = modified.map_or_else(epub::modified_now, |value| Ok(value.to_owned()))?;
    let plan = epub::export(
        &doc,
        title.unwrap_or(&fallback),
        lang.unwrap_or("und"),
        &modified,
    )?;
    let mut report = json!({
        "schema_version": 1, "mode": "plan", "bytes": plan.bytes().len(),
        "paragraphs": plan.paragraphs, "images": plan.images, "losses": plan.losses,
        "source_overwritten": false, "writes": 0, "modified": modified,
    });
    let mut code = 0;
    if let Some(path) = output {
        if accept {
            plan.publish(path, &AtomicBool::new(false))?;
            report["mode"] = json!("export");
            report["output"] = json!(path);
            report["writes"] = json!(1);
        } else {
            eprintln!(
                "Review EPUB conversion losses with --plan and pass --accept-losses to publish"
            );
            code = 2;
        }
    }
    super::report(report)?;
    Ok(code)
}

fn option_text<'a>(args: &mut std::slice::Iter<'a, OsString>) -> Result<&'a str, String> {
    args.next()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "EPUB option needs UTF-8 text".into())
}

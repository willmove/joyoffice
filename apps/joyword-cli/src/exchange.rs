//! Explicit file IO for the pure shell text-exchange plan.
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use joyword_shell::exchange::{self, Format};
use joyword_shell::exchange_output;
use serde_json::json;

pub(crate) struct Options {
    input: PathBuf,
    from: Format,
    to: Format,
    output: Option<PathBuf>,
    accept: bool,
}

pub(crate) fn options(
    mut args: impl Iterator<Item = OsString>,
) -> Result<Option<Options>, &'static str> {
    let input = args.next().ok_or("exchange requires an input file")?;
    if input == "--help" && args.next().is_none() {
        return Ok(None);
    }
    let mut from = None;
    let mut to = None;
    let mut output = None;
    let mut accept = false;
    let mut plan = false;
    while let Some(arg) = args.next() {
        if arg == "--from" || arg == "--to" {
            let format = args
                .next()
                .and_then(|s| s.to_str().and_then(Format::parse))
                .ok_or("invalid exchange format")?;
            let target = if arg == "--from" { &mut from } else { &mut to };
            if target.replace(format).is_some() {
                return Err("duplicate exchange format");
            }
        } else if arg == "--output" {
            if output
                .replace(PathBuf::from(args.next().ok_or("missing output path")?))
                .is_some()
            {
                return Err("duplicate output path");
            }
        } else if arg == "--plan" && !plan {
            plan = true;
        } else if arg == "--accept-losses" && !accept {
            accept = true;
        } else {
            return Err("unknown or duplicate exchange option");
        }
    }
    if plan == output.is_some() || (plan && accept) {
        return Err("choose --plan or --output [--accept-losses]");
    }
    let to = to.ok_or("missing --to")?;
    if let Some(destination) = &output
        && !exchange_output::valid_extension(to, destination)
    {
        return Err("output extension does not match exchange format");
    }
    Ok(Some(Options {
        input: input.into(),
        from: from.ok_or("missing --from")?,
        to,
        output,
        accept,
    }))
}

pub(crate) fn execute(options: Options) -> Result<u8, &'static str> {
    if !fs::metadata(&options.input)
        .map_err(|_| "cannot inspect exchange input")?
        .is_file()
    {
        return Err("exchange input must be a regular file");
    }
    let file = File::open(&options.input).map_err(|_| "cannot open exchange input")?;
    if !file
        .metadata()
        .map_err(|_| "cannot inspect exchange input")?
        .is_file()
    {
        return Err("exchange input must be a regular file");
    }
    let mut input = Vec::new();
    file.take(exchange::MAX_INPUT as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|_| "cannot read exchange input")?;
    let plan = exchange::plan(&input, options.from, options.to)?;
    let losses: Vec<_> = plan
        .losses
        .items
        .iter()
        .map(|l| json!({"kind":format!("{:?}",l.kind),"description":l.description}))
        .collect();
    let mut report = json!({"schema_version":1,"mode":"plan","from":options.from.name(),"to":options.to.name(),"bytes":plan.bytes().len(),"paragraphs":plan.paragraphs,"losses":losses,"writes":0,"source_overwritten":false});
    let mut code = 0;
    if let Some(output) = options.output {
        if !options.accept && plan.losses.has_loss() {
            code = 2;
        } else {
            exchange_output::write_new(
                &plan,
                &output,
                true,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .map_err(|error| {
                eprintln!("{error}");
                "text exchange publication failed; check whether output was already published"
            })?;
            report["mode"] = json!("export");
            report["output"] = json!(output);
            report["writes"] = json!(1);
        }
    }
    crate::canonicalize_keys(&mut report);
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &report)
        .map_err(|_| "cannot write exchange JSON (output may already be published)")?;
    stdout
        .write_all(b"\n")
        .map_err(|_| "cannot write exchange report (output may already be published)")?;
    stdout
        .flush()
        .map_err(|_| "cannot flush exchange report (output may already be published)")?;
    Ok(code)
}

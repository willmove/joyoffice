use joyword_shell::{
    docx::DocxSession,
    epub::{self, Metadata},
};
use serde_json::json;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

pub(crate) struct Options {
    input: PathBuf,
    output: Option<PathBuf>,
    title: Option<String>,
    language: String,
    modified: Option<String>,
    accept: bool,
}
pub(crate) fn options(
    mut args: impl Iterator<Item = OsString>,
) -> Result<Option<Options>, &'static str> {
    let Some(input) = args.next() else {
        return Err("epub needs a DOCX input");
    };
    if input == "--help" && args.next().is_none() {
        return Ok(None);
    }
    let mut options = Options {
        input: input.into(),
        output: None,
        title: None,
        language: "und".into(),
        modified: None,
        accept: false,
    };
    let mut plan = false;
    let mut language = false;
    while let Some(arg) = args.next() {
        if arg == "--plan" && !plan && options.output.is_none() {
            plan = true;
        } else if arg == "--accept-losses" && !options.accept {
            options.accept = true;
        } else if arg == "--output" && options.output.is_none() && !plan {
            options.output = Some(args.next().ok_or("--output needs a path")?.into());
        } else {
            let value = args
                .next()
                .and_then(|v| v.into_string().ok())
                .ok_or("epub option needs UTF-8 text")?;
            if arg == "--title" && options.title.is_none() {
                options.title = Some(value);
            } else if arg == "--language" && !language {
                options.language = value;
                language = true;
            } else if arg == "--modified" && options.modified.is_none() {
                options.modified = Some(value);
            } else {
                return Err("unknown, duplicate or conflicting epub option");
            }
        }
    }
    if !plan && options.output.is_none() {
        return Err("epub needs --plan or --output");
    }
    if plan && options.accept {
        return Err("--accept-losses requires --output");
    }
    Ok(Some(options))
}
pub(crate) fn execute(options: Options) -> Result<u8, &'static str> {
    let result = (|| -> Result<_, String> {
        let bytes = joyword_shell::mail_merge::read_local(&options.input, 32 << 20)
            .map_err(|e| e.to_string())?;
        let source = DocxSession::open_default(&bytes).map_err(|e| e.to_string())?;
        if !source.editability().is_unrestricted() {
            return Err("restricted DOCX cannot be converted to EPUB".into());
        }
        let title = options.title.unwrap_or_else(|| {
            options
                .input
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let modified = options
            .modified
            .map(Ok)
            .unwrap_or_else(epub::modified_now)
            .map_err(|e| e.to_string())?;
        let plan = epub::export(
            source.session().document(),
            &Metadata {
                title: &title,
                language: &options.language,
                modified: &modified,
            },
        )
        .map_err(|e| e.to_string())?;
        let losses: Vec<_> = plan
            .losses
            .iter()
            .map(|loss| json!({"location":loss.location,"detail":loss.detail}))
            .collect();
        let mut report = json!({"schema_version":1,"mode":"plan","bytes":plan.bytes().len(),"paragraphs":plan.paragraphs,"images":plan.images,"losses":losses,"source_overwritten":false,"writes":0,"modified":modified});
        let code = if let Some(output) = options.output {
            if !options.accept && !plan.losses.is_empty() {
                eprintln!(
                    "EPUB conversion has losses; review --plan and explicitly pass --accept-losses"
                );
                2
            } else {
                let path = epub::write_new(&plan, &output, &AtomicBool::new(false))
                    .map_err(|e| e.to_string())?;
                report["mode"] = json!("export");
                report["output"] = json!(path);
                report["writes"] = json!(1);
                0
            }
        } else {
            0
        };
        Ok((report, code))
    })();
    let (mut report, code) = result.map_err(|error| {
        eprintln!("EPUB: {error}");
        "EPUB conversion refused"
    })?;
    crate::canonicalize_keys(&mut report);
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &report).map_err(|_| "cannot write EPUB JSON")?;
    stdout.write_all(b"\n").map_err(|_| "cannot write stdout")?;
    Ok(code)
}

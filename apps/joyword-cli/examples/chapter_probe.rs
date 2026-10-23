//! Report the chapter page settings a DOCX imports.
//!
//! Usage: `chapter_probe <file.docx>`
//!
//! The page numbers themselves are drawn by the layout, which needs a font
//! and a scene. This tool answers the narrower question a user asks first:
//! did the file's `w:pgNumType` become typed chapter settings, or did it stay
//! an opaque fragment? A typed import reports the chapter level and the
//! separator; a preserved one reports the preserved fragment count.

use std::process::ExitCode;

use joyoffice_opc::Budget;
use joyword_docx::import;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: chapter_probe <file.docx>");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        eprintln!("usage: chapter_probe <file.docx>");
        return ExitCode::from(2);
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("cannot read the file: {error}");
            return ExitCode::from(2);
        }
    };
    let result = match import(&bytes, &Budget::default()) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("the file was refused: {error}");
            return ExitCode::from(1);
        }
    };
    let document = &result.document;
    let mut typed = 0;
    for (index, section) in document.sections().iter().enumerate() {
        let numbering = &section.numbering;
        let level = numbering.chapter_style;
        let separator = numbering.chapter_separator;
        if level.is_some() || separator != joyword_model::ChapterSeparator::default() {
            typed += 1;
            println!("section {index}: chapter level {level:?}, separator {separator:?}");
        } else {
            println!("section {index}: no chapter settings");
        }
    }
    println!("sections with chapter settings: {typed}");
    let reasons: Vec<String> = result
        .editability
        .read_only_reasons()
        .map(|reason| format!("{reason:?}"))
        .collect();
    println!("read only reasons: {}", reasons.join(", "));
    if typed == 0 {
        println!(
            "no section carries a chapter level, so the page numbers stay plain; \
             the file needs a heading-1 paragraph with a numbering definition and \
             a w:pgNumType with w:chapStyle"
        );
    }
    ExitCode::SUCCESS
}

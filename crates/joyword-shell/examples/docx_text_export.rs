//! Headless text-export smoke example; binary DOCX goes to stdout.
//! cargo run -p joyword-shell --example docx_text_export -- input.docx prefix > output.docx
//! Use a disposable output path: shell redirection controls the output file.
use std::error::Error;
use std::io::Write;

use joyoffice_opc::Budget;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: docx_text_export INPUT.docx PREFIX (binary DOCX on stdout)".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let mut session = DocxSession::open(&bytes, &Budget::default())?;
    let document = session.session().document();
    let paragraph = document
        .body()
        .blocks
        .get(0)
        .ok_or("no first block")?
        .as_paragraph()
        .ok_or("first block is not a paragraph")?;
    let at = Position {
        story: document.body_id(),
        paragraph: paragraph.id(),
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    session.apply(Transaction::new(
        session.session().version(),
        vec![Command::InsertText {
            at,
            text: args[2].clone(),
        }],
    ))?;
    let plan = session.export_plan()?;
    eprintln!(
        "verified {} {:?}; warnings: {:?}",
        plan.version(),
        plan.mode(),
        plan.warnings()
    );
    std::io::stdout().lock().write_all(plan.bytes())?;
    Ok(())
}

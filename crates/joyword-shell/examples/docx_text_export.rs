//! Headless text-export smoke example; optionally publish a new file.
//! cargo run -p joyword-shell --example docx_text_export -- input.docx prefix > output.docx
//! Or pass a NEW output path as a third argument, without shell redirection.
//! The storage path never overwrites an existing destination.
use std::error::Error;
use std::io::Write;

use joyoffice_opc::Budget;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::save_export_copy;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: docx_text_export INPUT.docx PREFIX [NEW_OUTPUT.docx] (otherwise binary DOCX on stdout)".into());
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
    if let Some(output) = args.get(3) {
        let receipt = save_export_copy(&plan, std::path::Path::new(output))?;
        eprintln!(
            "published {} ({} bytes, {}, {:?})",
            receipt.path().display(),
            receipt.bytes(),
            receipt.version(),
            receipt.durability()
        );
    } else {
        std::io::stdout().lock().write_all(plan.bytes())?;
    }
    Ok(())
}

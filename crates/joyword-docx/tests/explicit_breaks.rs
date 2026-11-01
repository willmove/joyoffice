//! ECMA-376 §17.3.3.1: inserted break kinds survive a formatted rewrite.
use joyoffice_opc::Budget;
use joyword_docx::{import, plan_export};
use joyword_engine::{Command, ParagraphChange, Session, Transaction};
use joyword_model::{Bias, BreakKind, InlineKind, Justification, Position, ScalarOffset};

#[test]
fn inserted_page_and_column_breaks_round_trip_with_paragraph_formatting() {
    for kind in [BreakKind::Page, BreakKind::Column, BreakKind::Line] {
        let original = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
        let imported = import(original, &Budget::default()).unwrap();
        let mut session = Session::new(imported.document);
        let paragraph = session.document().body().blocks.get(0).unwrap().id();
        let story = session.document().body_id();
        session
            .apply(Transaction::new(
                session.version(),
                vec![
                    Command::FormatParagraph {
                        story,
                        paragraph,
                        change: ParagraphChange::Justification(Some(Justification::Center)),
                    },
                    Command::InsertInline {
                        at: Position {
                            story,
                            paragraph,
                            offset: ScalarOffset(0),
                            bias: Bias::After,
                        },
                        kind: InlineKind::Break(kind),
                    },
                ],
            ))
            .unwrap();
        let plan = plan_export(&imported.backing, &session.snapshot(), &Budget::default()).unwrap();
        let reimported = import(plan.bytes(), &Budget::default()).unwrap();
        assert_eq!(
            session.document().semantic_difference(&reimported.document),
            None
        );
        assert_eq!(
            reimported
                .document
                .body()
                .blocks
                .get(0)
                .unwrap()
                .as_paragraph()
                .unwrap()
                .inlines()[0]
                .kind,
            InlineKind::Break(kind)
        );
    }
}

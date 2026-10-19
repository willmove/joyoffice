//! T300 slice 2: review commands, the author preference, and the three views.

use std::fs;
use std::path::PathBuf;

use joyword_model::RevisionView;
use joyword_shell::command::{
    ArgValue, EDIT_INSERT, FILE_OPEN, InvokeError, REVIEW_ACCEPT, REVIEW_ACCEPT_ALL, REVIEW_AUTHOR,
    REVIEW_FINAL, REVIEW_MARKUP, REVIEW_ORIGINAL, REVIEW_REJECT, REVIEW_REJECT_ALL, REVIEW_TRACK,
};
use joyword_shell::host::Office;
use joyword_shell::l10n::Locale;
use joyword_shell::prefs::PrefsStore;

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn open_office(dir: &std::path::Path) -> Office {
    let source = dir.join("sample.docx");
    fs::write(&source, fixture()).unwrap();
    let mut office = Office::new();
    office.bind_preferences(dir.join("prefs"), Locale::ZhCn);
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();
    office
}

fn body_text(office: &Office) -> String {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn revision_authors(office: &Office) -> Vec<String> {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .flat_map(|paragraph| paragraph.runs().to_vec())
        .filter_map(|run| run.revision.map(|mark| mark.author.to_string()))
        .collect()
}

#[test]
fn review_commands_use_the_registry_and_the_author_preference() {
    let dir = tempfile::tempdir().unwrap();
    let mut office = open_office(dir.path());
    let before = body_text(&office);

    for id in [REVIEW_MARKUP, REVIEW_FINAL, REVIEW_ORIGINAL, REVIEW_TRACK] {
        match office.invoke(id, &[]) {
            Err(InvokeError::PendingInput { missing }) => panic!("{id} asked for {missing:?}"),
            other => other.unwrap(),
        }
    }
    office
        .invoke(REVIEW_TRACK, &[("enabled", ArgValue::Bool(false))])
        .unwrap();
    assert!(!office.active().unwrap().tracking_enabled());
    match office.invoke(REVIEW_ACCEPT, &[]) {
        Err(InvokeError::PendingInput { missing }) => panic!("accept asked for {missing:?}"),
        Err(InvokeError::Failed(message)) => assert!(message.contains("review.none"), "{message}"),
        Ok(()) => panic!("accept without a revision succeeded"),
        Err(error) => panic!("{error:?}"),
    }
    assert!(matches!(
        office.invoke(REVIEW_AUTHOR, &[]),
        Err(InvokeError::PendingInput { .. })
    ));

    office
        .invoke(
            REVIEW_AUTHOR,
            &[("name", ArgValue::Text("  林审  ".into()))],
        )
        .unwrap();
    assert_eq!(office.revision_author(), "林审");
    let loaded = PrefsStore::new(dir.path().join("prefs")).load(Locale::ZhCn);
    assert_eq!(loaded.prefs.author_name, "林审");
    assert!(loaded.writable);

    office
        .invoke(REVIEW_TRACK, &[("enabled", ArgValue::Bool(true))])
        .unwrap();
    assert!(office.active().unwrap().tracking_enabled());
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("增".into()))])
        .unwrap();
    assert_eq!(revision_authors(&office), vec!["林审".to_string()]);
    assert!(body_text(&office).contains('增'));

    office.invoke(REVIEW_ACCEPT, &[]).unwrap();
    assert!(revision_authors(&office).is_empty());
    assert!(body_text(&office).contains('增'));
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(revision_authors(&office), vec!["林审".to_string()]);
    office.invoke(REVIEW_REJECT, &[]).unwrap();
    assert_eq!(body_text(&office), before);
    assert!(revision_authors(&office).is_empty());

    office
        .invoke(REVIEW_AUTHOR, &[("name", ArgValue::Text("   ".into()))])
        .unwrap();
    assert_eq!(office.revision_author(), "");
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("记".into()))])
        .unwrap();
    assert_eq!(revision_authors(&office), vec!["JoyWord".to_string()]);
    office.invoke(REVIEW_REJECT_ALL, &[]).unwrap();
    assert_eq!(body_text(&office), before);

    office
        .invoke(REVIEW_TRACK, &[("enabled", ArgValue::Bool(true))])
        .unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("全".into()))])
        .unwrap();
    office.invoke(REVIEW_ACCEPT_ALL, &[]).unwrap();
    assert!(revision_authors(&office).is_empty());
    assert!(body_text(&office).contains('全'));

    office.invoke(REVIEW_FINAL, &[]).unwrap();
    assert_eq!(office.review_view(), RevisionView::Final);
    office.invoke(REVIEW_ORIGINAL, &[]).unwrap();
    assert_eq!(office.review_view(), RevisionView::Original);
    office.invoke(REVIEW_MARKUP, &[]).unwrap();
    assert_eq!(office.review_view(), RevisionView::FinalWithMarkup);

    let kept = office.revision_author().to_owned();
    let rejected = office.invoke(REVIEW_AUTHOR, &[("name", ArgValue::Text("林\n审".into()))]);
    assert!(matches!(rejected, Err(InvokeError::Failed(_))));
    assert_eq!(office.revision_author(), kept);
    let long = "审".repeat(129);
    let rejected = office.invoke(REVIEW_AUTHOR, &[("name", ArgValue::Text(long))]);
    assert!(matches!(rejected, Err(InvokeError::Failed(_))));
    assert_eq!(office.revision_author(), kept);

    let locked = dir.path().join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(
        locked.join("preferences.conf"),
        "schema=99\nauthor.name=Future\n",
    )
    .unwrap();
    let mut newer = Office::new();
    newer.bind_preferences(&locked, Locale::En);
    assert_eq!(newer.revision_author(), "Future");
    let refused = newer.invoke(REVIEW_AUTHOR, &[("name", ArgValue::Text("New".into()))]);
    assert!(matches!(refused, Err(InvokeError::Failed(_))));
    assert_eq!(newer.revision_author(), "Future");
    assert_eq!(
        PrefsStore::new(&locked).load(Locale::En).prefs.author_name,
        "Future"
    );

    let mut bare = Office::new();
    assert!(matches!(
        bare.invoke(REVIEW_TRACK, &[]),
        Err(InvokeError::Disabled(_))
    ));
    bare.invoke(REVIEW_MARKUP, &[]).unwrap();
    assert_eq!(bare.review_view(), RevisionView::FinalWithMarkup);
}

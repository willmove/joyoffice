use crate::{Comment, Hyperlink, Sheet};
use joysheet_core::CellRef;

#[test]
fn comments_and_hyperlinks_survive_json() {
    let mut s = Sheet::new("S");
    let a1 = CellRef::new(0, 0);
    let c3 = CellRef::new(2, 2);
    s.comments.insert(
        a1,
        Comment {
            author: "Example".into(),
            text: "note".into(),
            replies: vec![("B".into(), "r".into())],
            threaded: true,
            resolved: false,
            visible: false,
            dates: vec![],
            note_box: None,
        },
    );
    s.hyperlinks.insert(c3, Hyperlink { target: "Sheet1!A1".into(), tooltip: Some("jump".into()), external: false });
    let json = serde_json::to_string(&s).expect("serialize");
    let back: Sheet = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.comments, s.comments);
    assert_eq!(back.hyperlinks, s.hyperlinks);
}

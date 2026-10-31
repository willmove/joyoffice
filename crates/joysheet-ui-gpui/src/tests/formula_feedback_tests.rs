use super::*;
#[test]
fn caret_nested_calls_and_quoted_references() {
    let text = "=SUM(A1,IF(B2,3,4),\"A99\",'数据'!$C$3:$D$4)";
    let caret = text.find(",\"A99\"").unwrap();
    assert_eq!(context(text, caret), Some(("SUM".into(), 1)));
    let refs = references(text);
    assert_eq!(refs.len(), 3);
    assert_eq!(refs[2].sheet.as_deref(), Some("数据"));
    assert_eq!(refs[2].range, RangeRef::parse("C3:D4").unwrap());
    let span = argument_span("IF(test,yes,no)", 1);
    assert_eq!(&"IF(test,yes,no)"[span], "yes");
    assert!(references("=SUM(\"A1\",42)").is_empty());
}

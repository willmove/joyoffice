use joyword_model::CoreProps;

#[test]
fn print_timestamp_is_optional_in_older_snapshots() -> Result<(), serde_json::Error> {
    let old: CoreProps = serde_json::from_str(r#"{"title":"Existing document"}"#)?;
    assert!(old.last_printed.is_empty());
    assert!(
        !serde_json::to_value(&old)?
            .as_object()
            .is_some_and(|v| v.contains_key("lastPrinted"))
    );
    let mut stamped = old;
    stamped.last_printed = "2026-10-17T21:42:00Z".into();
    let snapshot = serde_json::to_vec(&stamped)?;
    assert_eq!(serde_json::from_slice::<CoreProps>(&snapshot)?, stamped);
    Ok(())
}

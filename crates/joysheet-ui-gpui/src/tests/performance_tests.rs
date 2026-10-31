#[test]
fn completion_cannot_be_attributed_to_another_document_or_older_frame() {
    let engine = joysheet_app::EngineClient::start().unwrap();
    engine.execute("file.new", serde_json::json!({})).unwrap().result.unwrap();
    let before = engine.latest();
    let v = before.active().unwrap();
    let start = std::time::Instant::now();
    let ticket = engine.submit("cell.set", serde_json::json!({"input":"42"}), Default::default()).unwrap();
    let mut p = super::Performance::default();
    p.begin(ticket.id, v.window, v.doc, v.sheet, start, 0.);
    let c = ticket.wait(std::time::Duration::from_secs(5)).unwrap();
    p.completed(&c, std::time::Instant::now());
    p.applied(c.snapshot.sequence);
    let after = c.snapshot.active().unwrap();
    p.paint(after.window, after.doc + 1, after.sheet, c.snapshot.sequence, after.revision, std::time::Instant::now(), Default::default());
    p.paint(
        after.window,
        after.doc,
        after.sheet,
        c.snapshot.sequence.saturating_sub(1),
        after.revision,
        std::time::Instant::now(),
        Default::default(),
    );
    assert_eq!(p.report(true)["editCount"], 0);
    p.paint(after.window, after.doc, after.sheet, c.snapshot.sequence, after.revision, std::time::Instant::now(), Default::default());
    let report = p.report(true);
    assert_eq!(report["editCount"], 1);
    assert_eq!(report["edits"][0]["request"], c.request);
    for key in ["totalMs", "queueMs", "engineMs", "snapshotMs", "uiPollMs", "frameWaitMs"] {
        assert!(report["edits"][0][key].as_f64().unwrap() >= 0.);
    }
}

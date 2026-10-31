#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn parses_commands() {
    assert_eq!(parse_command("home.bold={\"range\":\"A1\"}").unwrap(), Some(("home.bold".into(), json!({"range": "A1"}))));
    assert_eq!(parse_command("edit.undo").unwrap(), Some(("edit.undo".into(), json!({}))));
    assert_eq!(
        parse_command("cell.set {\"cell\":\"A1\",\"input\":\"=1\"}").unwrap(),
        Some(("cell.set".into(), json!({"cell": "A1", "input": "=1"})))
    );
    assert_eq!(parse_command("{\"command\":\"edit.redo\"}").unwrap(), Some(("edit.redo".into(), json!({}))));
    assert_eq!(parse_command("  # comment").unwrap(), None);
    assert!(parse_command("x={bad").is_err());
    // A key this does not read is a mistake: the command used to run with no parameters and
    // the script still exited 0, so a typo made a valid, wrong workbook (#2256).
    for (line, named) in [
        (r#"{"command":"home.insertSheet","args":{"name":"R"}}"#, "`args`"),
        (r#"{"command":"home.insertSheet","name":"R"}"#, "`name`"),
        (r#"{"command":"home.insertSheet","params":{},"note":"why"}"#, "`note`"),
    ] {
        let e = parse_command(line).expect_err("refused");
        assert!(e.contains("unknown key") && e.contains(named), "{line}: {e}");
        assert!(e.contains(r#"{"command": "id", "params": {…}}"#), "{line}: {e}");
    }
    // Several at once are listed in one message, sorted.
    let e = parse_command(r#"{"command":"x","name":"R","args":{}}"#).expect_err("refused");
    assert!(e.contains("unknown keys `args`, `name`"), "{e}");
    // `id` is still the accepted alternative to `command`, and `params` still carries them.
    assert_eq!(parse_command(r#"{"id":"edit.undo"}"#).unwrap(), Some(("edit.undo".into(), json!({}))));
    assert_eq!(
        parse_command(r#"{"command":"home.insertSheet","params":{"name":"R"}}"#).unwrap(),
        Some(("home.insertSheet".into(), json!({"name": "R"})))
    );
}

#[test]
fn value_texts() {
    assert_eq!(value_text(&json!(6.0)), "6");
    assert_eq!(value_text(&json!(0.25)), "0.25");
    assert_eq!(value_text(&json!(1e300)), "1e+300");
    assert_eq!(value_text(&json!(-1.5e300)), "-1.5e+300");
    assert_eq!(value_text(&json!(1e-20)), "1e-20");
    assert_eq!(value_text(&json!(1e-300)), "1e-300");
    assert_eq!(value_text(&json!(0.0001)), "0.0001");
    assert_eq!(value_text(&json!(true)), "TRUE");
    assert_eq!(value_text(&json!({"error": "#DIV/0!"})), "#DIV/0!");
    assert_eq!(value_text(&json!([[1.0, 2.0], [3.0, "x"]])), "1\t2\n3\tx");
}

#[test]
fn args_parsing() {
    let v: Vec<String> = ["a", "--range", "A1", "--csv", "--sheet=S"].iter().map(|s| s.to_string()).collect();
    let a = Args::parse(&v, &["range", "sheet"], &["csv"]).unwrap();
    assert_eq!(a.positional, ["a"]);
    assert_eq!(a.opt("range"), Some("A1"));
    assert_eq!(a.opt("sheet"), Some("S"));
    assert!(a.has("csv"));
    assert!(Args::parse(&["--nope".to_string()], &[], &[]).is_err());
    assert!(Args::parse(&["--range".to_string()], &["range"], &[]).is_err());
}

#[test]
fn convert_xlsx_to_pdf_keeps_links() {
    let dir = std::env::temp_dir().join(format!("joysheet-cli-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("links.xlsx");
    let output = dir.join("links.pdf");
    let mut s = blank();
    s.execute("insert.link", json!({"cell": "A1", "target": "https://example.org"})).unwrap();
    let bytes = joysheet_engine::io::save_file_bytes(&s.doc().unwrap().wb, input.to_str().unwrap()).unwrap();
    joysheet_engine::io::write_file(input.to_str().unwrap(), &bytes).unwrap();
    convert(&[input.to_string_lossy().into_owned(), output.to_string_lossy().into_owned()]).unwrap();
    let pdf = std::fs::read(&output).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(pdf.windows(b"/URI (https://example.org)".len()).any(|w| w == b"/URI (https://example.org)"));
    std::fs::remove_dir_all(dir).unwrap();
}

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{W, import};
use joyword_model::Block;
use joyword_shell::mail_merge::{CsvData, Limits, MergeError, MergeTemplate};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

fn template(body: &str) -> Vec<u8> {
    let original = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap();
    let package = Package::read(&original, &Budget::default()).unwrap();
    let xml = format!(r#"<w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#);
    let mut parts = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Stored,
    )];
    for part in package.parts() {
        parts.push((
            part.name.item_name(),
            if part.name.as_str() == "/word/document.xml" {
                xml.as_bytes()
            } else {
                &part.data
            },
            part.method,
        ));
    }
    write_archive(&parts).unwrap()
}

fn simple() -> Vec<u8> {
    template("<w:p><w:r><w:t>Dear {{name}}, your ID is {{id}}.</w:t></w:r></w:p>")
}

#[test]
fn existing_inline_objects_are_preserved_or_refused_before_editing() {
    let bytes = template(
        "<w:p><w:r><w:t>Before</w:t><w:br/></w:r></w:p><w:p><w:r><w:t>{{name}}</w:t></w:r></w:p>",
    );
    let data = CsvData::parse(b"name\nAlice", &Limits::default()).unwrap();
    let result = MergeTemplate::parse(&bytes, &Limits::default())
        .unwrap()
        .bind(data, &BTreeMap::new())
        .unwrap()
        .preview(0)
        .unwrap();
    assert_eq!(result.text, "Before\n\nAlice");
    let opened = import(&result.docx, &Budget::default()).unwrap();
    let paragraph = opened
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert_eq!(paragraph.inlines().len(), 1);
    let same = template("<w:p><w:r><w:t>Before</w:t><w:br/><w:t>{{name}}</w:t></w:r></w:p>");
    assert!(matches!(
        MergeTemplate::parse(&same, &Limits::default()),
        Err(MergeError::Invalid(
            "template fields in paragraphs with existing inline objects are unsupported"
        ))
    ));
    let split = template("<w:p><w:r><w:t>{{na</w:t><w:br/><w:t>me}}</w:t></w:r></w:p>");
    assert!(MergeTemplate::parse(&split, &Limits::default()).is_err());
}

#[test]
fn pending_paragraph_and_table_property_revisions_are_refused() {
    let paragraph = template(
        r#"<w:p><w:pPr><w:pPrChange w:id="1" w:author="Fixture"><w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange></w:pPr><w:r><w:t>{{name}}</w:t></w:r></w:p>"#,
    );
    assert!(matches!(
        MergeTemplate::parse(&paragraph, &Limits::default()),
        Err(MergeError::Invalid(
            "template contains unresolved revisions"
        ))
    ));
    let table = template(
        r#"<w:tbl><w:tblPr><w:tblPrChange w:id="2" w:author="Fixture"><w:tblPr/></w:tblPrChange></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr/><w:p><w:r><w:t>{{name}}</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
    assert!(matches!(
        MergeTemplate::parse(&table, &Limits::default()),
        Err(MergeError::Invalid(
            "template contains unresolved revisions"
        ))
    ));
}

#[test]
fn fields_inside_table_cells_roundtrip_without_changing_the_grid() {
    let bytes = template(
        r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>{{name}}</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
    let data = CsvData::parse("name\n甲乙".as_bytes(), &Limits::default()).unwrap();
    let result = MergeTemplate::parse(&bytes, &Limits::default())
        .unwrap()
        .bind(data, &BTreeMap::new())
        .unwrap()
        .preview(0)
        .unwrap();
    assert_eq!(result.text, "甲乙");
    let opened = import(&result.docx, &Budget::default()).unwrap();
    let Some(Block::Table(table)) = opened.document.body().blocks.get(0) else {
        panic!("missing merged table")
    };
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].cells.len(), 1);
    assert_eq!(table.grid.len(), 1);
}

#[test]
fn csv_quotes_bom_crlf_empty_values_and_unicode_are_literal() {
    let data = CsvData::parse(
        "\u{feff}name,id\r\n\"甲,🙂\",\"a\"\"b\"\r\n乙,\r\n".as_bytes(),
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(data.rows(), 2);
    assert_eq!(data.headers(), &["name", "id"]);
    let job = MergeTemplate::parse(&simple(), &Limits::default())
        .unwrap()
        .bind(data, &BTreeMap::new())
        .unwrap();
    assert_eq!(job.preview(0).unwrap().text, "Dear 甲,🙂, your ID is a\"b.");
    assert_eq!(job.preview(1).unwrap().text, "Dear 乙, your ID is .");
}

#[test]
fn malformed_csv_and_encoding_are_refused_before_a_job_exists() {
    for input in [
        "",
        "name\n",
        "name,name\na,b",
        ",id\na,b",
        "name,id\na",
        "name,id\na,b,c",
        "name\n\"open",
        "name\na\"b",
        "name\n\"a\"x",
        "name\ra",
        "name\na\0b",
    ] {
        assert!(
            CsvData::parse(input.as_bytes(), &Limits::default()).is_err(),
            "{input:?}"
        );
    }
    assert!(CsvData::parse(&[0xff, 0xfe, 0x41, 0x00], &Limits::default()).is_err());
    assert!(
        CsvData::parse(
            format!("name\n{}", joyword_model::OBJECT_MARKER).as_bytes(),
            &Limits::default()
        )
        .is_err()
    );
}

#[test]
fn parser_enforces_each_input_budget_including_quoted_cells() {
    for limits in [
        Limits {
            csv_bytes: 3,
            ..Limits::default()
        },
        Limits {
            rows: 1,
            ..Limits::default()
        },
        Limits {
            columns: 1,
            ..Limits::default()
        },
        Limits {
            cell_bytes: 1,
            ..Limits::default()
        },
    ] {
        assert!(CsvData::parse(b"a,bb\nx,y\nx,y", &limits).is_err());
    }
    assert!(
        CsvData::parse(
            b"a\n\"xx\"",
            &Limits {
                cell_bytes: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        CsvData::parse(
            b"a\nx",
            &Limits {
                rows: 0,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn mapping_is_explicit_case_sensitive_and_unknown_mappings_are_errors() {
    let source = MergeTemplate::parse(&simple(), &Limits::default()).unwrap();
    let data = CsvData::parse(b"Full Name,Code\nAlice,007", &Limits::default()).unwrap();
    assert!(matches!(
        source.bind(data.clone(), &BTreeMap::new()),
        Err(MergeError::MissingColumn(_))
    ));
    let mut mapping = BTreeMap::from([
        ("name".into(), "Full Name".into()),
        ("id".into(), "Code".into()),
    ]);
    let job = source.bind(data.clone(), &mapping).unwrap();
    assert_eq!(job.preview(0).unwrap().text, "Dear Alice, your ID is 007.");
    assert!(job.preview(1).is_err());
    mapping.insert("unused".into(), "Code".into());
    assert!(matches!(
        source.bind(data, &mapping),
        Err(MergeError::UnknownField(_))
    ));
}

#[test]
fn values_cannot_inject_new_fields_or_xml_and_first_brace_style_survives() {
    let bytes = template(
        r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>Prefix </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>{{na</w:t></w:r><w:r><w:t>me}} end</w:t></w:r></w:p>"#,
    );
    let source = MergeTemplate::parse(&bytes, &Limits::default()).unwrap();
    let data = CsvData::parse(b"name\n{{id}} & &lt; <w:t>", &Limits::default()).unwrap();
    let output = source
        .bind(data, &BTreeMap::new())
        .unwrap()
        .preview(0)
        .unwrap();
    assert_eq!(output.text, "Prefix {{id}} & &lt; <w:t> end");
    let imported = import(&output.docx, &Budget::default()).unwrap();
    let p = imported
        .document
        .body()
        .blocks
        .iter()
        .find_map(Block::as_paragraph)
        .unwrap();
    assert_eq!(
        p.props.justification,
        Some(joyword_model::Justification::Center)
    );
    assert_eq!(
        p.format_at(joyword_model::ScalarOffset(8)).1.bold,
        Some(true)
    );
    assert_eq!(p.format_at(joyword_model::ScalarOffset(2)).1.bold, None);
    assert_eq!(
        p.format_at(joyword_model::ScalarOffset(p.len())).1.bold,
        None
    );
}

#[test]
fn repeated_fields_and_multiline_values_use_engine_line_breaks() {
    let source = MergeTemplate::parse(
        &template("<w:p><w:r><w:t>{{ name }} / {{name}}</w:t></w:r></w:p>"),
        &Limits::default(),
    )
    .unwrap();
    let data = CsvData::parse(b"name\n\"first\r\nsecond\tend\"", &Limits::default()).unwrap();
    let result = source
        .bind(data, &BTreeMap::new())
        .unwrap()
        .preview(0)
        .unwrap();
    assert_eq!(result.text, "first\nsecond\tend / first\nsecond\tend");
}

#[test]
fn malformed_templates_no_fields_and_protected_content_are_refused() {
    for text in [
        "ordinary text",
        "{{unclosed",
        "unmatched}}",
        "{{}}",
        "{{a{b}}",
    ] {
        assert!(
            MergeTemplate::parse(
                &template(&format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")),
                &Limits::default()
            )
            .is_err()
        );
    }
    let protected = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-005.docx"),
    )
    .unwrap();
    assert!(MergeTemplate::parse(&protected, &Limits::default()).is_err());
    assert!(
        MergeTemplate::parse(
            &simple(),
            &Limits {
                template_bytes: 8,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        MergeTemplate::parse(
            &simple(),
            &Limits {
                fields: 1,
                ..Limits::default()
            }
        )
        .is_err()
    );
}

#[test]
fn batch_uses_unique_directories_never_changes_inputs_and_is_readable() {
    let parent = tempfile::tempdir().unwrap();
    let original = simple();
    let template_path = parent.path().join("template.docx");
    std::fs::write(&template_path, &original).unwrap();
    let source = MergeTemplate::parse(&original, &Limits::default()).unwrap();
    let data = CsvData::parse(b"name,id\nAlice,1\nBob,2", &Limits::default()).unwrap();
    let job = source.bind(data, &BTreeMap::new()).unwrap();
    let cancel = AtomicBool::new(false);
    let first = job.write_batch(parent.path(), &cancel, |_| {}).unwrap();
    let second = job.write_batch(parent.path(), &cancel, |_| {}).unwrap();
    assert_ne!(first.directory, second.directory);
    assert_eq!(std::fs::read(template_path).unwrap(), original);
    assert_eq!(first.files.len(), 2);
    for (i, path) in first.files.iter().enumerate() {
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(bytes, job.preview(i).unwrap().docx);
        import(&bytes, &Budget::default()).unwrap();
    }
}

#[test]
fn cancelled_or_over_budget_batches_leave_no_partial_directory() {
    let parent = tempfile::tempdir().unwrap();
    let source = MergeTemplate::parse(&simple(), &Limits::default()).unwrap();
    let data = CsvData::parse(b"name,id\nAlice,1\nBob,2", &Limits::default()).unwrap();
    let job = source.bind(data.clone(), &BTreeMap::new()).unwrap();
    let cancel = AtomicBool::new(true);
    assert!(matches!(
        job.write_batch(parent.path(), &cancel, |_| {}),
        Err(MergeError::Cancelled)
    ));
    cancel.store(false, Ordering::Release);
    assert!(matches!(
        job.write_batch(parent.path(), &cancel, |_| cancel
            .store(true, Ordering::Release)),
        Err(MergeError::Cancelled)
    ));
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
    let limits = Limits {
        output_bytes: 1,
        ..Limits::default()
    };
    let job = MergeTemplate::parse(&simple(), &limits)
        .unwrap()
        .bind(data, &BTreeMap::new())
        .unwrap();
    assert!(
        job.write_batch(parent.path(), &AtomicBool::new(false), |_| {})
            .is_err()
    );
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

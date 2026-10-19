//! Headless round trip for the committed feature fixtures.
//!
//! These tests use the docx and model APIs already linked by joyword-cli.
//! Command-path edits that need the engine (regex `$1` replacement, table
//! structure commands, accept/reject) are not invoked here.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

use joyoffice_opc::Budget;
use joyword_docx::{
    DiagnosticKind, ExportError, ExportMode, ImageOutcome, PreservedReason, import, plan_export,
};
use joyword_model::{
    AnchorWrap, Block, BreakKind, DumpOptions, HeaderFooterKind, InlineKind, Measure, NumberFormat,
    PageFieldKind, RevisionKind, ScalarOffset, SectionBreak, StyleId, TabAlign, TabLeader,
    VerticalMerge,
};
use serde_json::Value;

struct Fixture {
    id: &'static str,
    bytes: &'static [u8],
    /// Sorted diagnostic codes. Empty means the importer typed the package.
    diagnostics: &'static [&'static str],
    /// A plain body paragraph can be patched. wrapTight locks the package.
    editable: bool,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        id: "DOCX-FEAT-T200-NUM",
        bytes: include_bytes!("../../../tests/fixtures/feature/DOCX-FEAT-T200-NUM.docx"),
        diagnostics: &[],
        editable: true,
    },
    Fixture {
        id: "DOCX-FEAT-T201-TABLE",
        bytes: include_bytes!("../../../tests/fixtures/feature/DOCX-FEAT-T201-TABLE.docx"),
        diagnostics: &[],
        editable: true,
    },
    Fixture {
        id: "DOCX-FEAT-T202-PAGES",
        bytes: include_bytes!("../../../tests/fixtures/feature/DOCX-FEAT-T202-PAGES.docx"),
        diagnostics: &[],
        editable: true,
    },
    Fixture {
        id: "DOCX-FEAT-T203-IMAGES",
        bytes: include_bytes!("../../../tests/fixtures/feature/DOCX-FEAT-T203-IMAGES.docx"),
        diagnostics: &[
            "image:rId4:Unsupported",
            "not-imported:http://schemas.openxmlformats.org/officeDocument/2006/relationships/image:/word/media/inline.png",
            "not-imported:http://schemas.openxmlformats.org/officeDocument/2006/relationships/image:/word/media/wrap-none.png",
            "not-imported:http://schemas.openxmlformats.org/officeDocument/2006/relationships/image:/word/media/wrap-square.png",
            "not-imported:http://schemas.openxmlformats.org/officeDocument/2006/relationships/image:/word/media/wrap-tight.png",
            "preserved:w:drawing:Unsupported",
        ],
        editable: false,
    },
    Fixture {
        id: "DOCX-FEAT-T300-REVIEW",
        bytes: include_bytes!("../../../tests/fixtures/feature/DOCX-FEAT-T300-REVIEW.docx"),
        diagnostics: &[],
        editable: true,
    },
];

const NOTE: &str = "/word/custom/note.xml";
const EDITABLE: &str = "可编辑句。";

fn budget() -> Budget {
    Budget::default()
}

fn diagnostic_code(kind: &DiagnosticKind) -> String {
    match kind {
        DiagnosticKind::Package(inner) => format!("package:{inner:?}"),
        DiagnosticKind::ExternalTarget { kind, .. } => format!("external:{kind:?}"),
        DiagnosticKind::Preserved { element, reason } => {
            format!("preserved:{element}:{reason:?}")
        }
        DiagnosticKind::ReadOnly(reason) => format!("read-only:{reason:?}"),
        DiagnosticKind::MissingPart { relationship, part } => {
            format!("missing:{relationship}:{part}")
        }
        DiagnosticKind::NotImported { relationship, part } => {
            format!("not-imported:{relationship}:{part}")
        }
        DiagnosticKind::InvalidValue {
            element,
            attribute,
            value,
        } => format!("invalid:{element}:{attribute}:{value}"),
        DiagnosticKind::StyleSkipped { id } => format!("style-skipped:{id:?}"),
        DiagnosticKind::SectionDefaults { missing } => format!("section-defaults:{missing}"),
        DiagnosticKind::PropertiesPreserved { count } => format!("properties:{count}"),
        DiagnosticKind::HintsSkipped { element, count } => format!("hints:{element}:{count}"),
        DiagnosticKind::UnpreservedWhitespace => "whitespace".into(),
        DiagnosticKind::EmptyBody => "empty-body".into(),
        DiagnosticKind::MacroProject => "macro".into(),
        DiagnosticKind::Image {
            relationship,
            outcome,
        } => format!("image:{relationship}:{outcome:?}"),
        DiagnosticKind::Model(issue) => format!("model:{issue}"),
        DiagnosticKind::RevisionSubset { element } => format!("revision-subset:{element}"),
        DiagnosticKind::MoveRange { name } => format!("move-range:{name}"),
    }
}

fn part_bytes(package: &[u8], name: &str) -> Vec<u8> {
    joyoffice_opc::Package::read(package, &budget())
        .unwrap()
        .part(name)
        .unwrap()
        .data
        .to_vec()
}

fn dump(document: &joyword_model::Document) -> String {
    document.canonical_dump(DumpOptions { node_ids: false })
}

fn body_paragraphs(document: &joyword_model::Document) -> Vec<String> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

fn append_editable(document: &mut joyword_model::Document) {
    let body = document.body_id();
    let index = document
        .story(body)
        .unwrap()
        .blocks
        .iter()
        .position(|block| {
            block
                .as_paragraph()
                .is_some_and(|paragraph| paragraph.plain_text(|_| String::new()) == EDITABLE)
        })
        .expect("editable paragraph");
    document
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("editable paragraph");
            };
            let end = ScalarOffset(paragraph.len());
            Arc::make_mut(paragraph).insert_text(end, "补").unwrap();
        });
}

fn assert_feature(id: &str, document: &joyword_model::Document) {
    match id {
        "DOCX-FEAT-T200-NUM" => assert_numbering(document),
        "DOCX-FEAT-T201-TABLE" => assert_table(document),
        "DOCX-FEAT-T202-PAGES" => assert_pages(document),
        "DOCX-FEAT-T203-IMAGES" => assert_images(document),
        "DOCX-FEAT-T300-REVIEW" => assert_review(document),
        other => panic!("unexpected fixture {other}"),
    }
}

fn assert_numbering(document: &joyword_model::Document) {
    let chain = document.styles().chain(&StyleId::new("Article"));
    assert!(chain.problem.is_none(), "{:?}", chain.problem);
    let ids: Vec<_> = chain.styles.iter().map(|style| style.id.as_str()).collect();
    assert_eq!(ids, ["Article", "Clause", "Body"]);
    assert_eq!(
        document
            .styles()
            .get(&StyleId::new("Clause"))
            .unwrap()
            .based_on,
        Some(StyleId::new("Body"))
    );
    let marks = joyword_model::list_marks(document);
    let mut labels = Vec::new();
    for block in document.body().blocks.iter() {
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        if let Some(label) = marks.get(&paragraph.id()) {
            labels.push(label.as_ref().unwrap().text.clone());
        }
    }
    assert_eq!(labels, ["3", "3.一", "4", "4.一"]);
    let tabs = document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .find(|paragraph| paragraph.plain_text(|_| String::new()).starts_with("金额"))
        .unwrap()
        .props
        .tabs
        .clone()
        .unwrap();
    assert_eq!(tabs[0].position, Measure::Twips(2880));
    assert_eq!(tabs[0].align, TabAlign::Left);
    assert_eq!(tabs[0].leader, TabLeader::Dot);
    assert!(
        body_paragraphs(document)
            .iter()
            .any(|text| text.contains("Order-014"))
    );
    assert_eq!(
        document.settings().default_tab_stop,
        Some(Measure::Twips(720))
    );
}

fn assert_table(document: &joyword_model::Document) {
    let Block::Table(table) = document.body().blocks.get(1).unwrap() else {
        panic!("typed contract table");
    };
    assert_eq!(
        table.grid,
        vec![
            Measure::Twips(1800),
            Measure::Twips(2200),
            Measure::Twips(2200)
        ]
    );
    let look = table.look.unwrap();
    assert!(look.first_row && look.first_column && look.last_column);
    assert!(!look.last_row && !look.no_h_band && !look.no_v_band);
    assert!(table.rows[0].header && table.rows[0].cant_split);
    assert_eq!(
        table.rows[0].cells[0].shading.unwrap().fill,
        joyword_model::Color::Rgb(0xFF, 0xF2, 0xCC)
    );
    assert_eq!(
        table.rows[0].cells[0].borders.bottom.unwrap().kind,
        joyword_model::BorderKind::Double
    );
    assert_eq!(
        table.rows[0].cells[0].borders.right.unwrap().kind,
        joyword_model::BorderKind::Nil
    );
    assert_eq!(table.rows[1].cells[0].grid_span, 2);
    assert_eq!(
        table.rows[1].cells[0].vertical_merge,
        VerticalMerge::Restart
    );
    assert_eq!(
        table.rows[2].cells[0].vertical_merge,
        VerticalMerge::Continue
    );
    let Block::Table(nested) = table.rows[2].cells[1].blocks.get(1).unwrap() else {
        panic!("one-level nested table");
    };
    assert_eq!(
        nested.rows[0].cells[0]
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| String::new()),
        "附注：虚构"
    );
}

fn assert_pages(document: &joyword_model::Document) {
    assert_eq!(document.settings().even_and_odd_headers, Some(true));
    let sections = document.sections();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].break_kind, SectionBreak::NextPage);
    assert_eq!(sections[0].title_page, Some(true));
    assert_eq!(sections[0].numbering.format, NumberFormat::Decimal);
    assert_eq!(sections[0].numbering.start, Some(1));
    let columns = sections[0].unequal_columns.as_ref().unwrap();
    assert_eq!(columns.columns[0].width, 2800);
    assert_eq!(columns.columns[0].space, 200);
    assert_eq!(columns.columns[1].width, 5306);
    let kinds: Vec<_> = sections[0].headers.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [
            HeaderFooterKind::First,
            HeaderFooterKind::Even,
            HeaderFooterKind::Default
        ]
    );
    assert!(
        sections[1].headers.is_empty(),
        "section 2 links by omission"
    );
    assert_eq!(sections[1].break_kind, SectionBreak::Continuous);
    assert_eq!(sections[1].numbering.format, NumberFormat::LowerRoman);
    assert_eq!(sections[1].numbering.start, Some(4));
    assert!(document.body().blocks.iter().any(|block| {
        block.as_paragraph().is_some_and(|paragraph| {
            paragraph.props.keep_next == Some(true) && paragraph.props.widow_control == Some(true)
        })
    }));
    assert!(document.body().blocks.iter().any(|block| {
        block
            .as_paragraph()
            .is_some_and(|paragraph| paragraph.props.keep_lines == Some(true))
    }));
    assert!(document.body().blocks.iter().any(|block| {
        block.as_paragraph().is_some_and(|paragraph| {
            paragraph
                .inlines()
                .iter()
                .any(|inline| matches!(inline.kind, InlineKind::Break(BreakKind::Column)))
        })
    }));
    let mut fields = Vec::new();
    for story in document.stories() {
        if story.kind != joyword_model::StoryKind::Header {
            continue;
        }
        for block in story.blocks.iter() {
            let Some(paragraph) = block.as_paragraph() else {
                continue;
            };
            for inline in paragraph.inlines() {
                if let InlineKind::PageField(field) = &inline.kind {
                    fields.push(field.kind);
                }
            }
        }
    }
    assert!(fields.contains(&PageFieldKind::Page));
    assert!(fields.contains(&PageFieldKind::NumPages));
}

fn assert_images(document: &joyword_model::Document) {
    assert_eq!(document.resources().len(), 3);
    let mut anchors = Vec::new();
    for block in document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
    {
        for inline in block.inlines() {
            match &inline.kind {
                InlineKind::Image { anchor, .. } => anchors.push(*anchor),
                InlineKind::Preserved(_) => anchors.push(None),
                _ => {}
            }
        }
    }
    assert_eq!(anchors.len(), 4);
    assert!(anchors[0].is_none(), "inline picture");
    assert!(matches!(
        anchors[1],
        Some(anchor) if matches!(anchor.wrap, AnchorWrap::None)
    ));
    assert!(matches!(
        anchors[2],
        Some(anchor) if matches!(anchor.wrap, AnchorWrap::Square(_))
    ));
    assert!(anchors[3].is_none(), "wrapTight stays preserved");
}

fn assert_review(document: &joyword_model::Document) {
    assert_eq!(document.settings().track_revisions, Some(true));
    let mut kinds = Vec::new();
    for block in document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
    {
        for run in block.runs() {
            if let Some(mark) = &run.revision {
                kinds.push(mark.kind);
            }
        }
    }
    assert_eq!(
        kinds,
        [
            RevisionKind::Insert,
            RevisionKind::Delete,
            RevisionKind::MoveFrom,
            RevisionKind::MoveTo
        ]
    );
    assert_eq!(document.move_markers().len(), 4);
    assert!(
        document
            .move_markers()
            .iter()
            .all(|marker| marker.name.as_ref() == "move-a")
    );
    let header = document
        .stories()
        .find(|story| story.kind == joyword_model::StoryKind::Header)
        .unwrap();
    let header_kinds: Vec<_> = header
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.revision.as_ref().map(|mark| mark.kind))
        .collect();
    assert_eq!(header_kinds, [RevisionKind::Insert, RevisionKind::Delete]);
}

#[test]
fn feature_fixtures_import_round_trip_and_keep_unknown_parts() {
    for fixture in FIXTURES {
        let imported = import(fixture.bytes, &budget()).unwrap_or_else(|error| {
            panic!("{}: import failed: {error}", fixture.id);
        });
        assert!(
            !imported.document.validate().has_errors(),
            "{}: {:?}",
            fixture.id,
            imported.document.validate()
        );
        let mut codes: Vec<_> = imported
            .diagnostics
            .iter()
            .map(|item| diagnostic_code(&item.kind))
            .collect();
        codes.sort();
        let expected: Vec<String> = fixture
            .diagnostics
            .iter()
            .map(|code| (*code).to_owned())
            .collect();
        assert_eq!(codes, expected, "{}", fixture.id);
        assert_feature(fixture.id, &imported.document);
        let note = part_bytes(fixture.bytes, NOTE);
        let plan =
            plan_export(&imported.backing, &imported.document.snapshot(), &budget()).unwrap();
        assert_eq!(plan.mode(), ExportMode::OriginalPackage, "{}", fixture.id);
        assert_eq!(plan.bytes(), fixture.bytes, "{}", fixture.id);
        assert_eq!(plan.origin_bytes(), Some(fixture.bytes), "{}", fixture.id);
        let copied = import(plan.bytes(), &budget()).unwrap();
        assert_eq!(
            dump(&imported.document),
            dump(&copied.document),
            "{}",
            fixture.id
        );
        assert!(
            imported
                .document
                .semantic_difference(&copied.document)
                .is_none()
        );
        assert_eq!(part_bytes(plan.bytes(), NOTE), note, "{}", fixture.id);

        if fixture.editable {
            assert!(imported.editability.is_unrestricted(), "{}", fixture.id);
            let mut edited = imported.document.clone();
            let before = body_paragraphs(&edited);
            append_editable(&mut edited);
            let plan = plan_export(&imported.backing, &edited.snapshot(), &budget()).unwrap();
            assert_eq!(plan.mode(), ExportMode::TextPatch, "{}", fixture.id);
            assert!(
                plan.preserved_parts()
                    .iter()
                    .any(|part| part.as_str() == NOTE),
                "{} preserved {:?}",
                fixture.id,
                plan.preserved_parts()
            );
            assert_eq!(part_bytes(plan.bytes(), NOTE), note, "{}", fixture.id);
            assert_eq!(plan.origin_bytes(), Some(fixture.bytes), "{}", fixture.id);
            let again = import(plan.bytes(), &budget()).unwrap();
            assert!(
                edited.semantic_difference(&again.document).is_none(),
                "{}",
                fixture.id
            );
            assert_eq!(dump(&edited), dump(&again.document), "{}", fixture.id);
            let after = body_paragraphs(&again.document);
            assert!(after.iter().any(|text| text == "可编辑句。补"), "{after:?}");
            assert_eq!(
                before
                    .iter()
                    .filter(|text| text.as_str() != EDITABLE)
                    .collect::<Vec<_>>(),
                after
                    .iter()
                    .filter(|text| text.as_str() != "可编辑句。补")
                    .collect::<Vec<_>>(),
                "{}",
                fixture.id
            );
        } else {
            assert!(!imported.editability.is_unrestricted(), "{}", fixture.id);
            assert!(imported.diagnostics.iter().any(|item| matches!(
                item.kind,
                DiagnosticKind::Image {
                    outcome: ImageOutcome::Unsupported,
                    ..
                } | DiagnosticKind::Preserved {
                    reason: PreservedReason::Unsupported,
                    ..
                }
            )));
            let mut edited = imported.document.clone();
            append_editable(&mut edited);
            let error = plan_export(&imported.backing, &edited.snapshot(), &budget()).unwrap_err();
            assert!(
                matches!(error, ExportError::Restricted),
                "{}: {error}",
                fixture.id
            );
        }
    }
}

fn cli() -> Command {
    let key = format!(
        "CARGO_TARGET_{}_UNKNOWN_LINUX_GNU_RUNNER",
        std::env::consts::ARCH.to_uppercase()
    );
    let runner = std::env::var(key).ok();
    let mut parts = runner.as_deref().unwrap_or("").split_whitespace();
    if let Some(program) = parts.next() {
        let mut process = Command::new(program);
        process.args(parts).arg(env!("CARGO_BIN_EXE_joyword-cli"));
        process
    } else {
        Command::new(env!("CARGO_BIN_EXE_joyword-cli"))
    }
}

fn font_spec() -> Option<String> {
    if let Ok(spec) = std::env::var("JOYWORD_TEST_FONT") {
        return Some(spec);
    }
    for (path, face) in [
        ("/System/Library/Fonts/Supplemental/Songti.ttc", 6),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
        ("C:\\Windows\\Fonts\\simsun.ttc", 0),
    ] {
        if Path::new(path).is_file() {
            return Some(format!("{path}:{face}"));
        }
    }
    assert!(
        std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_none(),
        "a real CJK font is required"
    );
    eprintln!("SKIPPED: no CJK font for visual CLI tests");
    None
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture_golden")
}

fn bless_or_compare(path: &Path, actual: &[u8]) {
    if std::env::var_os("JOYWORD_BLESS_GOLDEN").is_some() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, actual).unwrap();
        return;
    }
    let expected = std::fs::read(path).unwrap_or_else(|error| {
        panic!(
            "missing golden {}: {error}. Set JOYWORD_BLESS_GOLDEN=1 to write it.",
            path.display()
        );
    });
    assert_eq!(actual, expected.as_slice(), "golden {}", path.display());
}

fn write_input(dir: &Path, fixture: &Fixture) -> PathBuf {
    let path = dir.join(format!("{}.docx", fixture.id));
    std::fs::write(&path, fixture.bytes).unwrap();
    path
}

#[test]
fn feature_fixture_cli_projections_match_goldens() {
    for fixture in FIXTURES {
        let dir = tempfile::tempdir().unwrap();
        let input = write_input(dir.path(), fixture);
        let input = input.to_str().unwrap();
        let inspect = cli().args(["inspect", input]).output().unwrap();
        assert_eq!(
            inspect.status.code(),
            Some(0),
            "{:?} {}",
            inspect.stderr,
            fixture.id
        );
        let report: Value = serde_json::from_slice(&inspect.stdout).unwrap();
        assert_eq!(report["schema_version"], 1, "{}", fixture.id);
        assert_eq!(report["projection"]["name"], "imported-main-body");
        bless_or_compare(
            &golden_dir().join(format!("{}.inspect.json", fixture.id)),
            &inspect.stdout,
        );
        let text = cli().args(["text", input]).output().unwrap();
        assert_eq!(
            text.status.code(),
            Some(0),
            "{:?} {}",
            text.stderr,
            fixture.id
        );
        let text_report: Value = serde_json::from_slice(&text.stderr).unwrap();
        assert_eq!(text_report["text_bytes"], report["text_bytes"]);
        assert_eq!(text_report["paragraphs"], report["paragraphs"]);
        bless_or_compare(
            &golden_dir().join(format!("{}.txt", fixture.id)),
            &text.stdout,
        );
        let embedded = report["text"].as_str().unwrap().as_bytes();
        assert_eq!(embedded, text.stdout.as_slice(), "{}", fixture.id);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Visual {
    Success { pages: u64, omitted: u64 },
    Refused(&'static str),
}

fn expected_render(id: &str) -> Visual {
    match id {
        // Number labels and dot leaders share a glyph run whose pen does not
        // stay continuous. The CLI raster refuses that scene.
        "DOCX-FEAT-T200-NUM" => {
            Visual::Refused("joyword-cli: page raster refused: invalid content or budget")
        }
        "DOCX-FEAT-T201-TABLE" | "DOCX-FEAT-T300-REVIEW" => Visual::Success {
            pages: 1,
            omitted: 0,
        },
        "DOCX-FEAT-T202-PAGES" => Visual::Success {
            pages: 2,
            omitted: 0,
        },
        // wrapTight stays a preserved inline. Layout refuses that inline
        // rather than dropping the drawing.
        "DOCX-FEAT-T203-IMAGES" => {
            Visual::Refused("joyword-cli: document layout refused: unsupported content or budget")
        }
        other => panic!("unexpected fixture {other}"),
    }
}

fn expected_convert(id: &str) -> Visual {
    match id {
        "DOCX-FEAT-T200-NUM" | "DOCX-FEAT-T201-TABLE" | "DOCX-FEAT-T300-REVIEW" => {
            Visual::Success {
                pages: 1,
                omitted: 0,
            }
        }
        "DOCX-FEAT-T202-PAGES" => Visual::Success {
            pages: 2,
            omitted: 0,
        },
        "DOCX-FEAT-T203-IMAGES" => {
            Visual::Refused("joyword-cli: document layout refused: unsupported content or budget")
        }
        other => panic!("unexpected fixture {other}"),
    }
}

fn visual_result(output: &Output) -> Visual {
    if output.status.code() == Some(0) {
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        return Visual::Success {
            pages: report["pages"].as_u64().unwrap(),
            omitted: report["omitted_blocks"].as_u64().unwrap(),
        };
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr.lines().next().unwrap_or("").trim();
    const REFUSALS: &[&str] = &[
        "joyword-cli: document layout refused: unsupported content or budget",
        "joyword-cli: font cannot display the complete document",
        "joyword-cli: DOCX refused: invalid, unsupported or over budget",
        "joyword-cli: page raster refused: invalid content or budget",
        "joyword-cli: PDF writer refused the page scene",
    ];
    assert!(
        REFUSALS.contains(&line),
        "undocumented refusal: {line:?} status {:?}",
        output.status.code()
    );
    Visual::Refused(REFUSALS.iter().copied().find(|item| *item == line).unwrap())
}

#[test]
fn feature_fixture_render_and_convert_follow_the_subset() {
    let Some(font) = font_spec() else {
        return;
    };
    for fixture in FIXTURES {
        let dir = tempfile::tempdir().unwrap();
        let input = write_input(dir.path(), fixture);
        let input = input.to_str().unwrap();
        let render_dir = dir.path().join("pages");
        let rendered = cli()
            .args([
                "render",
                input,
                "--font",
                &font,
                "--output",
                render_dir.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        let render_outcome = visual_result(&rendered);
        assert_eq!(
            render_outcome,
            expected_render(fixture.id),
            "{}",
            fixture.id
        );
        if let Visual::Success { .. } = &render_outcome {
            assert!(render_dir.join("page-1.png").is_file(), "{}", fixture.id);
        }
        let pdf = dir.path().join(format!("{}.pdf", fixture.id));
        let converted = cli()
            .args([
                "convert",
                input,
                "--to",
                "pdf",
                "--font",
                &font,
                "--output",
                pdf.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            visual_result(&converted),
            expected_convert(fixture.id),
            "{}",
            fixture.id
        );
        if let Visual::Success { .. } = expected_convert(fixture.id) {
            let bytes = std::fs::read(&pdf).unwrap();
            assert!(bytes.starts_with(b"%PDF-"), "{}", fixture.id);
        }
    }
}

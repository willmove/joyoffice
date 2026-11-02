use joyword_model::Block;
use joyword_rtf::{RtfBudget, export_rtf, import_rtf};

fn text(document: &joyword_model::Document) -> String {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p.raw_text()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn unicode_surrogates_and_font_destinations_survive_real_round_trip() {
    let input = br"{\rtf1\ansi{\fonttbl{\f0 SecretFontName;}}\u20013?\u-10179?\u-8704?\par}";
    let document = import_rtf(input, RtfBudget::default()).unwrap().0;
    assert_eq!(text(&document), "中😀");
    let exported = export_rtf(&document).0;
    assert_eq!(
        text(
            &import_rtf(exported.as_bytes(), RtfBudget::default())
                .unwrap()
                .0
        ),
        "中😀"
    );
}

#[test]
fn malformed_groups_unicode_and_code_pages_refuse_instead_of_returning_partial_success() {
    for input in [
        br"{\rtf1 missing".as_slice(),
        br"{\rtf1 valid}extra",
        br"{\rtf1 valid}}",
        br"{\rtf1 \u-10179?}",
        br"{\rtf1 \u-8704?}",
        br"{\rtf1 \u999999?}",
        br"{\rtf1 \b999999999999999999 word}",
        br"{\rtf1 \'e9}",
        br"{\rtf1 \deleted secret}",
    ] {
        assert!(
            import_rtf(input, RtfBudget::default()).is_err(),
            "{input:?}"
        );
    }
    let budget = RtfBudget {
        max_text_bytes: 8,
        ..RtfBudget::default()
    };
    assert!(import_rtf(br"{\rtf1 abcdefgh}", budget).is_err());
}

#[test]
fn hidden_attribute_is_preserved_for_projection_checks() {
    let document = import_rtf(br"{\rtf1 public {\v secret} tail}", RtfBudget::default())
        .unwrap()
        .0;
    let Block::Paragraph(paragraph) = document.body().blocks.get(0).unwrap() else {
        panic!()
    };
    assert!(
        paragraph
            .runs()
            .iter()
            .any(|r| r.props.hidden == Some(true))
    );
}

#[test]
fn empty_paragraphs_are_not_dropped_from_interchange() {
    let document = import_rtf(br"{\rtf1 first\par\par last\par}", RtfBudget::default())
        .unwrap()
        .0;
    assert_eq!(text(&document), "first\n\nlast");
    let exported = export_rtf(&document).0;
    assert_eq!(
        text(
            &import_rtf(exported.as_bytes(), RtfBudget::default())
                .unwrap()
                .0
        ),
        "first\n\nlast"
    );
}

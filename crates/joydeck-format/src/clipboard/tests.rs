#![allow(clippy::expect_used, reason = "Test assertions")]

use super::*;
use joydeck_model::text::Align;

#[test]
fn html_fragment_keeps_inline_formats_and_safe_links() {
    let payload = r#"<p style="text-align:center"><strong>中文😀</strong> <span style="font: x; color:#2d6890; font-size:20px; font-family:'Avenir'; background-color:yellow"><em>Blue</em></span><br><a href="https://example.org/test?a=1&amp;b=2">site</a> <a href="file:///tmp/local">local</a></p><script>ignored <b>hidden</b></script><div hidden>secret</div>"#;
    let pre = "<html><head><title>Hidden header</title></head><body>Context";
    let post = "Context suffix</body></html>";
    let header = |a: usize, b: usize| {
        format!("Version:1.0\r\nStartFragment:{a:010}\r\nEndFragment:{b:010}\r\n")
    };
    let start = header(0, 0).len() + pre.len();
    let wire = format!(
        "{}{pre}{payload}{post}",
        header(start, start + payload.len())
    );
    let body = import_html(wire.as_bytes()).expect("valid HTML fragment");
    assert_eq!(body.text(), "中文😀 Blue\u{b}site local");
    let para = &body.paragraphs[0];
    assert_eq!(para.props.align, Some(Align::Center));
    assert_eq!(para.runs[0].props.bold, Some(true));
    let blue = para
        .runs
        .iter()
        .find(|r| r.text.contains("Blue"))
        .expect("styled run");
    assert_eq!(blue.props.size, Some(15.));
    assert_eq!(blue.props.italic, Some(true));
    assert_eq!(blue.props.font.as_deref(), Some("Avenir"));
    assert_eq!(blue.props.fill, Some(color_fill(Rgba::rgb(45, 104, 144))));
    let site = para
        .runs
        .iter()
        .find(|r| r.text == "site")
        .expect("link run");
    assert_eq!(
        site.props.link,
        url_link("https://example.org/test?a=1&b=2")
    );
    assert!(
        para.runs
            .iter()
            .find(|r| r.text.contains("local"))
            .expect("local text")
            .props
            .link
            .is_none()
    );
}

#[test]
fn rtf_fragment_preserves_styles_codepages_and_unicode() {
    let input = br#"{\rtf1\ansi\ansicpg936\deff0\uc1{\fonttbl{\f0 Arial;}{\f1 Avenir;}}{\colortbl;\red45\green104\blue144;}\qc{\b\f1\fs32 \'d6\'d0\'ce\'c4\u-10179?\u-8704?} {\i\cf1 Color}\line{\field{\*\fldinst HYPERLINK "https://example.org/rtf"}{\fldrslt web}}{\pict\bin4 {}}!}\par\pard Plain}"#;
    let body = import_rtf(input).expect("valid RTF fragment");
    assert_eq!(body.text(), "中文😀 Color\u{b}web\nPlain");
    let para = &body.paragraphs[0];
    assert_eq!(para.props.align, Some(Align::Center));
    assert_eq!(para.runs[0].props.font.as_deref(), Some("Avenir"));
    assert_eq!(para.runs[0].props.size, Some(16.));
    assert_eq!(para.runs[0].props.bold, Some(true));
    let color = para
        .runs
        .iter()
        .find(|r| r.text == "Color")
        .expect("color run");
    assert_eq!(color.props.italic, Some(true));
    assert_eq!(color.props.fill, Some(color_fill(Rgba::rgb(45, 104, 144))));
    let web = para
        .runs
        .iter()
        .find(|r| r.text == "web")
        .expect("field result");
    assert_eq!(web.props.link, url_link("https://example.org/rtf"));
    assert_eq!(body.paragraphs[1].props.align, None);
}

#[test]
fn malformed_and_oversized_fragments_do_not_escape_the_budget() {
    assert!(import_rtf(br#"{\rtf1\ansi{\pict\bin999 x}}"#).is_none());
    assert!(import_rtf(br#"{\rtf1\ansi open"#).is_none());
    assert!(import_html(&vec![b'x'; MAX_TEXT + 1]).is_none());
    let nested = format!("{}text", "<span>".repeat(MAX_DEPTH + 1));
    assert!(import_html(nested.as_bytes()).is_none());
    let unsafe_rtf =
        br#"{\rtf1{\field{\*\fldinst HYPERLINK "javascript:alert(1)"}{\fldrslt visible}}}"#;
    let body = import_rtf(unsafe_rtf).expect("field label remains visible");
    assert_eq!(body.text(), "visible");
    assert!(body.paragraphs[0].runs[0].props.link.is_none());
}

#[test]
fn rtf_hidden_text_is_scoped_and_explicitly_reset() {
    for (input, expected) in [
        (r"{\rtf1 Before{\v secret{nested}}After}", "BeforeAfter"),
        (r"{\rtf1 \v secret{\v0 Visible}secret\v0 End}", "VisibleEnd"),
        (r"{\rtf1 \b\v secret\plain Visible}", "Visible"),
    ] {
        let body = import_rtf(input.as_bytes()).expect("visible RTF text");
        assert_eq!(body.text(), expected, "{input}");
        assert_ne!(body.paragraphs[0].runs[0].props.bold, Some(true));
    }
}

#[test]
fn rtf_hidden_unicode_and_breaks_do_not_leak_into_visible_text() {
    let body = import_rtf(
        br"{\rtf1\ansi\uc1 Before{\v \'e9\u20013?\u-10179?\u-8704?\tab\line\par\emdash}After\line End}",
    )
    .expect("visible RTF text");
    assert_eq!(body.text(), "BeforeAfter\u{b}End");
    assert_eq!(body.paragraphs.len(), 1);
    assert!(import_rtf(br"{\rtf1\v secret}").is_none());
}

#[test]
fn hidden_rtf_consumes_paste_without_exposing_plain_text_fallback() {
    for input in [
        br"{\rtf1\v secret}".as_slice(),
        br"{\rtf1{\v secret}\v0}",
        br"{\rtf1\v\line\par}",
    ] {
        let mut fallback_called = false;
        let body = import_rtf_for_paste(input)
            .or_else(|| {
                fallback_called = true;
                None
            })
            .expect("hidden RTF consumes paste");
        assert!(body.is_empty());
        assert!(!fallback_called);
        assert!(import_rtf(input).is_none(), "legacy import contract");
    }
    assert_eq!(
        import_rtf_for_paste(br"{\rtf1{\v secret}Visible}")
            .expect("visible remainder")
            .text(),
        "Visible"
    );
    for input in [
        br"{\rtf1 ordinary unclosed".as_slice(),
        br"{\rtf1}",
        br"not RTF",
    ] {
        assert!(
            import_rtf_for_paste(input).is_none(),
            "unusable RTF still falls back"
        );
    }
}

#[test]
fn table_import_refuses_mixed_hidden_content_but_preserves_normal_fallback() {
    let rtf = br"{\rtf1 Visible\tab{\v secret}Other\par Next}";
    assert_eq!(
        import_rtf_for_paste(rtf).expect("visible text").text(),
        "Visible\tOther\nNext"
    );
    assert!(
        import_rtf_for_table(rtf)
            .expect("consume unsafe table paste")
            .is_empty()
    );
    for html in [
        r"<table><tr><td>Visible</td><td hidden>secret</td></tr></table>",
        r#"Visible<span style="display:none">secret</span>Other"#,
        r#"Visible<span style="visibility:hidden">secret</span>Other"#,
        r"<span hidden>secret</span>",
    ] {
        assert!(
            import_html_for_table(html.as_bytes())
                .expect("consume unsafe table paste")
                .is_empty()
        );
    }
    let html = b"<html><head><title>Metadata</title></head><body><table><tr><td>A</td><td>B</td></tr></table></body></html>";
    assert!(
        !import_html_for_table(html)
            .expect("ordinary HTML must not block TSV")
            .is_empty()
    );
    assert!(
        !import_rtf_for_table(br"{\rtf1 A\tab B}")
            .expect("ordinary RTF")
            .is_empty()
    );
    assert!(
        import_rtf_for_table(br"{\rtf1\v malformed")
            .expect("recognized hiding blocks even malformed RTF")
            .is_empty()
    );
    assert!(import_rtf_for_table(br"{\rtf1 malformed").is_none());
    assert!(import_html_for_table(&[0xff]).is_none());
    assert!(import_html_for_table(&vec![b'x'; MAX_INPUT + 1]).is_none());
}

#[test]
fn table_hiding_cannot_be_undone_by_later_output_budget_failure() {
    let text = "x".repeat(MAX_TEXT + 1);
    let rtf = format!(r"{{\rtf1\v secret\v0 {text}}}");
    assert!(
        import_rtf_for_table(rtf.as_bytes())
            .expect("hidden RTF consumes overflow")
            .is_empty()
    );
    let html = format!("<span hidden>secret</span>{text}");
    assert!(
        import_html_for_table(html.as_bytes())
            .expect("hidden HTML consumes overflow")
            .is_empty()
    );
    assert!(import_html_for_table(text.as_bytes()).is_none());
}

#[test]
fn recognized_hiding_survives_parse_and_output_failure_for_text_paste() {
    for input in [
        br"{\rtf1\v unclosed".as_slice(),
        br"{\rtf1 Visible{\v secret}",
        br"{\rtf1\v secret\v0\bin999 short}",
    ] {
        assert_eq!(
            import_rtf_for_paste(input).map(|b| b.is_empty()),
            Some(true)
        );
    }
    let text = "x".repeat(MAX_TEXT + 1);
    let rtf = format!(r"{{\rtf1\v secret\v0 {text}}}");
    assert_eq!(
        import_rtf_for_paste(rtf.as_bytes()).map(|b| b.is_empty()),
        Some(true)
    );
    let html = format!("<span hidden>secret</span>{text}");
    assert_eq!(
        import_html(html.as_bytes()).map(|b| b.is_empty()),
        Some(true)
    );
    assert!(import_rtf_for_paste(br"{\rtf1 ordinary unclosed").is_none());
    assert!(import_html(text.as_bytes()).is_none());
}

#[test]
fn fully_hidden_html_consumes_paste_without_rtf_or_plain_fallback() {
    for html in [
        "<span hidden>secret</span>",
        "<span style='display:none'>secret</span>",
        "<div style='visibility:hidden'><b>secret</b></div>",
        "<span hidden>secret",
    ] {
        let mut fallback = false;
        let body = import_html(html.as_bytes())
            .or_else(|| {
                fallback = true;
                import_rtf_for_paste(br"{\rtf1 secret}")
            })
            .expect("consume recognized hidden HTML");
        assert!(!fallback, "{html}");
        assert!(body.is_empty(), "{html}");
    }
    assert_eq!(
        import_html(b"Visible<span hidden>secret</span> remainder")
            .expect("visible remainder")
            .text(),
        "Visible remainder"
    );
    assert!(import_html(b"<span></span>").is_none());
    assert!(import_html(b"<head><title>metadata</title></head>").is_none());
}

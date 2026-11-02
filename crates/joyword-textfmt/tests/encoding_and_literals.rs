use joyword_model::Block;
use joyword_textfmt::{
    encoding::decode_and_normalize, export_markdown, import_markdown, import_plain_text,
};

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
fn malformed_bom_payloads_are_disclosed_and_never_silently_truncated() {
    for bytes in [
        vec![0xef, 0xbb, 0xbf, 0xff],
        vec![0xff, 0xfe, 0x00, 0xd8],
        vec![0xfe, 0xff, 0xdc, 0x00],
        vec![0xff, 0xfe, 0x41],
        vec![0xfe, 0xff, 0x41],
    ] {
        let decoded = decode_and_normalize(&bytes);
        assert_eq!(decoded.unmapped_chars, 1, "{bytes:?}");
        assert!(decoded.text.contains('\u{fffd}'));
        assert!(import_plain_text(&bytes).1.has_loss());
    }
    let valid = b"\xef\xbb\xbf\xef\xbf\xbd";
    assert_eq!(decode_and_normalize(valid).unmapped_chars, 0);
    assert!(!import_plain_text(valid).1.has_loss());
    assert_eq!(decode_and_normalize(b"\xef\xbf\xbd\xff").unmapped_chars, 1);
}

#[test]
fn literal_markdown_syntax_is_escaped_before_formatting_and_recovers_as_text() {
    let literal = "# 中文 *text* [click](javascript:alert) <script> &entity; | `code` \\";
    let document = import_plain_text(literal.as_bytes()).0;
    let output = export_markdown(&document).0;
    assert!(output.starts_with("\\#"));
    assert!(output.contains("\\<script\\>"));
    assert_eq!(text(&import_markdown(&output).0), literal);
}

#[test]
fn html_entities_decode_once_and_numeric_references_preserve_unicode() {
    use joyword_textfmt::html::unescape_html;
    assert_eq!(unescape_html("&amp;lt;script&amp;gt;"), "&lt;script&gt;");
    assert_eq!(unescape_html("&#20013;&#x1F600;&nbsp;"), "中😀\u{a0}");
    assert_eq!(
        unescape_html("&#0;&#xD800;&unknown;&"),
        "&#0;&#xD800;&unknown;&"
    );
    assert_eq!(unescape_html(&"&".repeat(100_000)).len(), 100_000);
}

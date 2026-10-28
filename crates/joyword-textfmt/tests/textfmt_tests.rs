use joyword_textfmt::{
    LineEnding, LossKind, PlainTextOptions, export_html, export_markdown, export_plain_text,
    import_html, import_markdown, import_plain_text,
};

#[test]
fn test_plain_text_bidirectional() {
    let raw_text = "段落一：JoyOffice 纯文本测试\r\n段落二：跨平台换行规范化\n段落三：第三行";
    let (doc, report) = import_plain_text(raw_text.as_bytes());
    assert!(!report.has_loss());

    let (exported, exp_report) = export_plain_text(
        &doc,
        PlainTextOptions {
            line_ending: LineEnding::Lf,
        },
    );
    assert!(!exp_report.has_loss());
    assert!(exported.contains("段落一：JoyOffice 纯文本测试"));
    assert!(exported.contains("段落二：跨平台换行规范化"));
    assert!(exported.contains("段落三：第三行"));
}

#[test]
fn test_markdown_bidirectional_and_security() {
    let md = "# 核心标题\n\n\
              这是带有 **加粗** 和 *斜体* 以及 `代码` 的段落。\n\n\
              | 列A | 列B |\n\
              | --- | --- |\n\
              | 甲 | 乙 |\n\n\
              [恶意链接](javascript:evil_call())\n";

    let (doc, report) = import_markdown(md);
    assert!(
        report
            .items
            .iter()
            .any(|item| item.kind == LossKind::ActiveContentStripped)
    );

    let (exported, _) = export_markdown(&doc);
    assert!(exported.contains("核心标题"));
    assert!(exported.contains("**加粗**"));
    assert!(exported.contains("*斜体*"));
    assert!(exported.contains("| 列A | 列B |"));
    assert!(exported.contains("| 甲 | 乙 |"));
    assert!(!exported.contains("javascript:"));
}

#[test]
fn test_html_bidirectional_and_sanitization() {
    let html = "<html><body>\
                <h1>大标题</h1>\
                <p>正文内容，含有 <strong>粗体</strong> 和 <em>强调</em>。</p>\
                <table border=\"1\"><tr><td>单元格A</td><td>单元格B</td></tr></table>\
                <script>alert('xss');</script>\
                </body></html>";

    let (doc, report) = import_html(html);
    assert!(
        report
            .items
            .iter()
            .any(|item| item.kind == LossKind::ActiveContentStripped)
    );

    let (exported, _) = export_html(&doc);
    assert!(exported.contains("大标题"));
    assert!(exported.contains("<strong>粗体</strong>"));
    assert!(exported.contains("<em>强调</em>"));
    assert!(exported.contains("<td>单元格A</td>"));
    assert!(!exported.contains("<script>"));
    assert!(!exported.contains("alert"));
}

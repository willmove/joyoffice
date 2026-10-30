//! Regression cases discovered during the M7 input audit.
use joyword_model::Block;
use joyword_textfmt::{export_html, import_html};

#[test]
fn unicode_case_expansion_does_not_move_active_element_boundaries() {
    for text in ["İ", "İİ", "İ中", "中İ🌍"] {
        for tag in ["script", "STYLE", "iframe", "OBJECT", "applet"] {
            let input = format!("<p>before</p><{tag}>{text}</{tag}><p>after</p>");
            let (document, losses) = import_html(&input);
            let (output, _) = export_html(&document);
            assert!(output.contains("before"));
            assert!(output.contains("after"), "{input}: {output}");
            assert!(!output.contains(text), "active payload: {output}");
            assert!(losses.has_loss());
        }
    }
}

#[test]
fn unicode_table_cells_keep_their_original_utf8_boundaries() {
    let (document, _) = import_html(
        "<TABLE>İ<TR>İ<TD>İ中🌍</TD><TD>乙</TD></TR><TR><TD>末İ</TD></TR></TABLE><p>end</p>",
    );
    let Block::Table(table) = document.body().blocks.get(0).expect("table") else {
        panic!("table not imported");
    };
    assert_eq!(table.rows.len(), 2);
    for (row, cell, expected) in [(0, 0, "İ中🌍"), (0, 1, "乙"), (1, 0, "末İ")] {
        let Block::Paragraph(paragraph) = table.rows[row].cells[cell].blocks.get(0).unwrap() else {
            panic!("cell paragraph");
        };
        assert_eq!(paragraph.raw_text(), expected);
    }
    assert!(export_html(&document).0.contains("end"));
}

#[test]
fn unterminated_active_payload_is_discarded_and_stray_closers_do_not_swallow_text() {
    let (document, _) = import_html("<p>before</p><SCRIPT>private payload<p>still script</p>");
    let output = export_html(&document).0;
    assert!(output.contains("before"));
    assert!(!output.contains("private payload"));
    assert!(!output.contains("still script"));
    let (document, _) = import_html("</script><p>keep</p></script><p>tail</p>");
    let output = export_html(&document).0;
    assert!(output.contains("keep"));
    assert!(output.contains("tail"));
}

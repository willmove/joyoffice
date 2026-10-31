use super::decode_zip_name;
use std::borrow::Cow;

#[test]
fn declared_utf8_and_ascii_names_borrow_the_original_bytes() {
    assert!(matches!(decode_zip_name(b"xl/workbook.xml", false).unwrap(), Cow::Borrowed(_)));
    assert_eq!(decode_zip_name("xl/图表.xml".as_bytes(), true).unwrap(), "xl/图表.xml");
    assert!(decode_zip_name(b"bad\xff", true).is_err());
}

#[test]
fn unflagged_names_keep_cp437_even_when_the_bytes_are_valid_utf8() {
    assert_eq!(decode_zip_name(b"\x82", false).unwrap(), "é");
    assert_eq!(decode_zip_name(b"\xc3\xa9", false).unwrap(), "├⌐");
    assert_eq!(decode_zip_name(b"\xc3\xa9", true).unwrap(), "é");
    assert_eq!(decode_zip_name(b"\xff", false).unwrap(), "\u{a0}");
}

#[test]
fn oversized_names_are_rejected_before_decoding() {
    assert!(decode_zip_name(&vec![b'a'; 65536], false).is_err());
}

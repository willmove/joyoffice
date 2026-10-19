//! Headless print-to-file. Native dialogs stay unverified and write nothing.

use std::io::Write;

use joyword_shell::print::{
    self, PrintToFile, copies_of, page_span, platform_print_dialog, print_to_file, publish_pdf,
    review_marks,
};

fn sample() -> Vec<u8> {
    b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\ntrailer\n<< >>\nstartxref\n0\n%%EOF\n".to_vec()
}

#[test]
fn publish_verifies_bytes_and_refuses_an_existing_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.pdf");
    let bytes = sample();
    let receipt = publish_pdf(&path, &bytes).unwrap();
    assert_eq!(std::fs::read(receipt.path()).unwrap(), bytes);
    assert_eq!(receipt.bytes() as usize, bytes.len());
    assert_eq!(receipt.copies(), 1);
    let before = std::fs::read(&path).unwrap();
    assert!(publish_pdf(&path, b"%PDF-1.4\n%%EOF\n").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn a_bad_header_and_the_wrong_extension_leave_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("out.pdf");
    let txt = dir.path().join("out.txt");
    assert!(publish_pdf(&pdf, b"not a pdf").is_err());
    assert!(publish_pdf(&txt, &sample()).is_err());
    assert!(!pdf.exists());
    assert!(!txt.exists());
}

#[test]
fn print_to_file_records_copies_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.pdf");
    let bytes = sample();
    let receipt = print_to_file(PrintToFile {
        destination: &path,
        pdf: &bytes,
        copies: 3,
        range: "2-4",
    })
    .unwrap();
    assert_eq!(receipt.copies(), 3);
    assert_eq!(receipt.range(), "2-4");
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(
        print_to_file(PrintToFile {
            destination: &dir.path().join("zero.pdf"),
            pdf: &bytes,
            copies: 0,
            range: "all",
        })
        .is_err()
    );
    assert!(
        print_to_file(PrintToFile {
            destination: &dir.path().join("many.pdf"),
            pdf: &bytes,
            copies: 100,
            range: "all",
        })
        .is_err()
    );
    assert!(!dir.path().join("zero.pdf").exists());
}

#[test]
fn page_range_copies_and_review_marks_are_bounded() {
    assert_eq!(page_span(None, 3).unwrap(), 0..3);
    assert_eq!(page_span(Some("all"), 3).unwrap(), 0..3);
    assert_eq!(page_span(Some("2-3"), 3).unwrap(), 1..3);
    assert_eq!(page_span(Some("2"), 3).unwrap(), 1..2);
    assert!(page_span(Some("0-1"), 3).is_err());
    assert!(page_span(Some("3-2"), 3).is_err());
    assert!(page_span(Some("1-4"), 3).is_err());
    assert!(page_span(Some("a"), 3).is_err());
    assert_eq!(copies_of(None).unwrap(), 1);
    assert_eq!(copies_of(Some("99")).unwrap(), 99);
    assert!(copies_of(Some("0")).is_err());
    assert!(review_marks(None).is_ok());
    assert!(review_marks(Some("laid-out")).is_ok());
    assert!(review_marks(Some("markup")).is_err());
    assert!(review_marks(Some("original")).is_err());
}

#[test]
fn the_native_dialog_is_not_verified_and_creates_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.pdf");
    let error = platform_print_dialog().unwrap_err();
    let message = error.to_string();
    assert!(message.contains("not verified"), "{message}");
    #[cfg(target_os = "linux")]
    {
        let linux = print::linux_print_dialog().unwrap_err();
        assert!(linux.to_string().contains("Linux"));
    }
    #[cfg(target_os = "macos")]
    {
        let macos = print::macos_print_dialog().unwrap_err();
        assert!(macos.to_string().contains("macOS"));
    }
    #[cfg(target_os = "windows")]
    {
        let windows = print::windows_print_dialog().unwrap_err();
        assert!(windows.to_string().contains("Windows"));
    }
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(b"stay").unwrap();
    let again = platform_print_dialog().unwrap_err();
    assert!(again.to_string().contains("not verified"));
    assert_eq!(std::fs::read(&path).unwrap(), b"stay");
}

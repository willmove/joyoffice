
use super::*;
#[test]
fn relationship_paths_and_xml_limits() {
    assert_eq!(
        resolve("ppt/slides/s1.xml", "../media/a.png").unwrap(),
        "ppt/media/a.png"
    );
    assert!(resolve("ppt/p.xml", "../../escape").is_err());
    assert!(resolve("ppt/p.xml", "https://example.com").is_err());
    assert!(xml("<!DOCTYPE a [<!ENTITY x 'x'>]><a>&x;</a>").is_err());
    assert!(xml(&format!("{}{}", "<a>".repeat(129), "</a>".repeat(129))).is_err());
}

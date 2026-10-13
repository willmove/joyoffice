//! Untrusted-package tests for the OPC layer (QUALITY-005): valid minimal
//! packages, container attacks, part-name attacks, hostile XML, external
//! targets and a deterministic mutation smoke run over the reader.

use joyoffice_opc::{
    Budget, BudgetError, Diagnostic, ExternalKind, Method, OpcError, Package, PartName,
    PartNameError, Target, XmlError, ZipError, write_archive,
};

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>甲方</w:t></w:r></w:p></w:body></w:document>"#;

const OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

fn document_rels(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

fn archive(items: &[(&str, &str)]) -> Vec<u8> {
    let items: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&items).unwrap()
}

fn minimal(extra: &[(&str, &str)]) -> Vec<u8> {
    let mut items = vec![
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", DOCUMENT),
    ];
    items.extend_from_slice(extra);
    archive(&items)
}

fn read(bytes: &[u8]) -> Result<Package, OpcError> {
    Package::read(bytes, &Budget::default())
}

#[test]
fn reads_a_minimal_package_and_keeps_exact_bytes() {
    let package = read(&minimal(&[])).unwrap();
    let main = package
        .relationships_of_type(None, OFFICE_DOCUMENT)
        .next()
        .expect("officeDocument relationship");
    let Target::Internal(name) = &main.target else {
        panic!("internal target expected")
    };
    let part = package.part(name.as_str()).unwrap();
    assert_eq!(&*part.data, DOCUMENT.as_bytes());
    assert_eq!(
        part.content_type.as_deref(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml")
    );
    assert_eq!(
        package
            .part("/_RELS/.RELS")
            .unwrap()
            .content_type
            .as_deref(),
        Some("application/vnd.openxmlformats-package.relationships+xml")
    );
    assert_eq!(package.content_types_xml(), CONTENT_TYPES.as_bytes());
    assert!(
        package.diagnostics().is_empty(),
        "{:?}",
        package.diagnostics()
    );
}

#[test]
fn stored_items_round_trip_through_the_writer() {
    let items = [
        (
            "[Content_Types].xml",
            CONTENT_TYPES.as_bytes(),
            Method::Stored,
        ),
        ("_rels/.rels", PACKAGE_RELS.as_bytes(), Method::Stored),
        ("word/document.xml", DOCUMENT.as_bytes(), Method::Deflated),
    ];
    let package = read(&write_archive(&items).unwrap()).unwrap();
    assert_eq!(package.part("/_rels/.rels").unwrap().method, Method::Stored);
    assert_eq!(package.parts().count(), 2);
}

#[test]
fn relationships_resolve_relative_targets_and_never_follow_external_ones() {
    let rels = document_rels(concat!(
        r#"<Relationship Id="rId1" Type="t/image" Target="media/image1.png"/>"#,
        r#"<Relationship Id="rId2" Type="t/hyperlink" Target="https://example.invalid/x" TargetMode="External"/>"#,
        r#"<Relationship Id="rId3" Type="t/attachedTemplate" Target="\\fileserver\share\normal.dotm" TargetMode="External"/>"#,
        r#"<Relationship Id="rId4" Type="t/subDocument" Target="file://fileserver/share/a.docx" TargetMode="External"/>"#,
        r#"<Relationship Id="rId5" Type="t/x" Target="../missing.xml"/>"#,
        r#"<Relationship Id="rId6" Type="t/x" Target="../../outside.xml"/>"#,
    ));
    let package = read(&minimal(&[
        ("word/_rels/document.xml.rels", &rels),
        ("word/media/image1.png", "png bytes"),
    ]))
    .unwrap();
    let document = PartName::new("/word/document.xml").unwrap();
    let list = package.relationships(Some(&document));
    assert_eq!(list.len(), 6);
    assert_eq!(
        list[0].target,
        Target::Internal(PartName::new("/word/media/image1.png").unwrap())
    );
    let kinds: Vec<_> = list[1..4]
        .iter()
        .map(|relationship| match &relationship.target {
            Target::External { kind, .. } => *kind,
            other => panic!("expected external, got {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            ExternalKind::Web,
            ExternalKind::NetworkShare,
            ExternalKind::NetworkShare
        ]
    );
    assert!(package.diagnostics().contains(&Diagnostic::DanglingTarget {
        source: "/word/document.xml".into(),
        id: "rId5".into(),
        target: "../missing.xml".into(),
    }));
    assert!(matches!(list[5].target, Target::Unresolved(_)));
    assert!(
        package
            .diagnostics()
            .iter()
            .any(|d| matches!(d, Diagnostic::InvalidTarget { id, .. } if id == "rId6"))
    );
    assert!(package.diagnostics().iter().any(
        |d| matches!(d, Diagnostic::MissingContentType { part } if part == "/word/media/image1.png")
    ));
}

#[test]
fn external_targets_are_classified_conservatively() {
    for (target, kind) in [
        ("https://example.invalid/a.png", ExternalKind::Web),
        ("HTTP://EXAMPLE.INVALID", ExternalKind::Web),
        ("\\\\server\\share\\t.dotx", ExternalKind::NetworkShare),
        ("//server/share/t.dotx", ExternalKind::NetworkShare),
        ("file://server/share/t.dotx", ExternalKind::NetworkShare),
        ("file:////server/share/t.dotx", ExternalKind::NetworkShare),
        ("file:///C:/templates/t.dotx", ExternalKind::File),
        ("C:\\templates\\t.dotx", ExternalKind::File),
        ("../other.docx", ExternalKind::File),
        ("mailto:legal@example.invalid", ExternalKind::Mail),
        ("ms-word:ofe|u|x", ExternalKind::Other),
    ] {
        assert_eq!(ExternalKind::classify(target), kind, "{target}");
    }
}

#[test]
fn rejects_unsafe_part_names() {
    for (item, expected) in [
        ("word/../evil.xml", PartNameError::DotSegment),
        ("word\\evil.xml", PartNameError::InvalidCharacter('\\')),
        ("/abs.xml", PartNameError::EmptySegment),
        ("word/a%2Fb.xml", PartNameError::EncodedSeparator),
    ] {
        let error = read(&minimal(&[(item, "x")])).unwrap_err();
        assert_eq!(
            error,
            OpcError::PartName {
                name: item.into(),
                error: expected
            }
        );
    }
    let duplicate = read(&minimal(&[("WORD/DOCUMENT.XML", "x")])).unwrap_err();
    assert_eq!(
        duplicate,
        OpcError::DuplicatePart {
            name: "WORD/DOCUMENT.XML".into()
        }
    );
    let prefix = read(&minimal(&[("word/document.xml/inner.xml", "x")])).unwrap_err();
    assert_eq!(
        prefix,
        OpcError::PrefixPart {
            name: "/word/document.xml".into()
        }
    );
    let missing = read(&archive(&[("_rels/.rels", PACKAGE_RELS)])).unwrap_err();
    assert_eq!(missing, OpcError::MissingContentTypes);
}

#[test]
fn rejects_hostile_xml_before_expansion() {
    let doctype = r#"<?xml version="1.0"?><!DOCTYPE r [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;">]><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="&b;" Type="t" Target="x"/></Relationships>"#;
    let error = read(&minimal(&[("word/_rels/document.xml.rels", doctype)])).unwrap_err();
    assert!(
        matches!(
            error,
            OpcError::Xml {
                error: XmlError::DocumentType,
                ..
            }
        ),
        "{error:?}"
    );

    let external = r#"<?xml version="1.0"?><!DOCTYPE r SYSTEM "file:///etc/passwd"><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#;
    let error = read(&archive(&[("[Content_Types].xml", external)])).unwrap_err();
    assert!(
        matches!(
            error,
            OpcError::Xml {
                error: XmlError::DocumentType,
                ..
            }
        ),
        "{error:?}"
    );

    let deep = document_rels(&format!("{}{}", "<x>".repeat(300), "</x>".repeat(300)));
    let error = read(&minimal(&[("word/_rels/document.xml.rels", &deep)])).unwrap_err();
    assert!(
        matches!(
            error,
            OpcError::Xml {
                error: XmlError::Budget(BudgetError::XmlDepth { .. }),
                ..
            }
        ),
        "{error:?}"
    );

    let unknown_entity = document_rels(r#"<Relationship Id="&evil;" Type="t" Target="x"/>"#);
    assert!(matches!(
        read(&minimal(&[(
            "word/_rels/document.xml.rels",
            &unknown_entity
        )])),
        Err(OpcError::Xml {
            error: XmlError::Malformed(_),
            ..
        })
    ));

    let duplicate = document_rels(
        r#"<Relationship Id="rId1" Type="t" Target="a.xml"/><Relationship Id="rId1" Type="t" Target="b.xml"/>"#,
    );
    assert_eq!(
        read(&minimal(&[("word/_rels/document.xml.rels", &duplicate)])).unwrap_err(),
        OpcError::DuplicateRelationshipId {
            part: "/word/_rels/document.xml.rels".into(),
            id: "rId1".into()
        }
    );

    let wrong_root = r#"<Types xmlns="urn:not-opc"/>"#;
    assert!(matches!(
        read(&archive(&[("[Content_Types].xml", wrong_root)])),
        Err(OpcError::Xml {
            error: XmlError::UnexpectedRoot,
            ..
        })
    ));
}

#[test]
fn budgets_stop_bombs_before_inflation() {
    // 64 MiB of zeros deflates to about 64 KiB: ratio ~1000.
    let zeros = vec![0u8; 64 << 20];
    let bomb = write_archive(&[
        (
            "[Content_Types].xml",
            CONTENT_TYPES.as_bytes(),
            Method::Deflated,
        ),
        ("word/zeros.bin", &zeros, Method::Deflated),
    ])
    .unwrap();
    assert!(bomb.len() < 1 << 20);
    let error = read(&bomb).unwrap_err();
    assert!(
        matches!(
            error,
            OpcError::Zip(ZipError::Budget(BudgetError::Ratio { .. }))
        ),
        "{error:?}"
    );

    let small = Budget {
        max_entries: 2,
        ..Budget::default()
    };
    assert!(matches!(
        Package::read(&minimal(&[]), &small),
        Err(OpcError::Zip(ZipError::Budget(BudgetError::Entries {
            count: 3,
            limit: 2
        })))
    ));
    let tight = Budget {
        max_part_bytes: 100,
        ..Budget::default()
    };
    assert!(matches!(
        Package::read(&minimal(&[]), &tight),
        Err(OpcError::Zip(ZipError::Budget(
            BudgetError::PartSize { .. }
        )))
    ));
}

/// Offset of the n-th central directory header.
fn central_header(bytes: &[u8], n: usize) -> usize {
    bytes
        .windows(4)
        .enumerate()
        .filter(|(_, window)| *window == [0x50, 0x4b, 0x01, 0x02])
        .nth(n)
        .map(|(at, _)| at)
        .unwrap()
}

#[test]
fn rejects_lying_and_overlapping_archives() {
    let good = minimal(&[]);

    // Declared uncompressed size smaller than the real content.
    let mut lying = good.clone();
    let cd = central_header(&lying, 2);
    lying[cd + 24..cd + 28].copy_from_slice(&10u32.to_le_bytes());
    assert!(matches!(
        read(&lying),
        Err(OpcError::Zip(ZipError::Corrupt { .. }))
    ));

    // Flipped CRC.
    let mut crc = good.clone();
    let cd = central_header(&crc, 2);
    crc[cd + 16] ^= 0xFF;
    assert!(matches!(
        read(&crc),
        Err(OpcError::Zip(ZipError::Corrupt { .. }))
    ));

    // Encryption flag.
    let mut encrypted = good.clone();
    let cd = central_header(&encrypted, 0);
    encrypted[cd + 8] |= 0x01;
    assert_eq!(
        read(&encrypted).unwrap_err(),
        OpcError::Zip(ZipError::Unsupported("encrypted item"))
    );

    // Unknown compression method.
    let mut method = good.clone();
    let cd = central_header(&method, 1);
    method[cd + 10..cd + 12].copy_from_slice(&12u16.to_le_bytes());
    assert_eq!(
        read(&method).unwrap_err(),
        OpcError::Zip(ZipError::Unsupported("compression method"))
    );

    // Second central entry pointing at the first entry's local header.
    let mut overlapping = good.clone();
    let first_offset = {
        let cd = central_header(&overlapping, 0);
        overlapping[cd + 42..cd + 46].to_vec()
    };
    let cd = central_header(&overlapping, 1);
    overlapping[cd + 42..cd + 46].copy_from_slice(&first_offset);
    let error = read(&overlapping).unwrap_err();
    assert!(
        matches!(error, OpcError::Zip(ZipError::Malformed(_))),
        "{error:?}"
    );

    assert_eq!(
        read(b"not a zip").unwrap_err(),
        OpcError::Zip(ZipError::NotZip)
    );
    assert!(read(&good[..good.len() / 2]).is_err());
}

#[test]
fn mutation_smoke_never_panics() {
    let rels = document_rels(r#"<Relationship Id="rId1" Type="t" Target="media/a.png"/>"#);
    let seed_bytes = minimal(&[
        ("word/_rels/document.xml.rels", &rels),
        ("word/media/a.png", "png"),
    ]);
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };
    let mut accepted = 0;
    for _ in 0..3_000 {
        let mut bytes = seed_bytes.clone();
        match next(4) {
            0 => {
                for _ in 0..1 + next(8) {
                    let at = next(bytes.len());
                    bytes[at] = next(256) as u8;
                }
            }
            1 => bytes.truncate(next(bytes.len())),
            2 => {
                let at = next(bytes.len());
                let len = next(64).min(bytes.len() - at);
                bytes.drain(at..at + len);
            }
            _ => {
                let at = next(bytes.len());
                bytes.splice(at..at, std::iter::repeat_n(next(256) as u8, 1 + next(32)));
            }
        }
        accepted += usize::from(read(&bytes).is_ok());
    }
    // Most mutations break the CRC or structure; any accepted package must
    // still have passed every check, which `read` returning Ok implies.
    assert!(accepted < 3_000);

    // Mutate package XML before archiving so the CRC is valid and the XML,
    // content-type and relationship layers see the damage.
    const NOISE: &[u8] = b"<>&\"/=:x\0\xff#";
    let xml_items = [
        CONTENT_TYPES.as_bytes(),
        PACKAGE_RELS.as_bytes(),
        rels.as_bytes(),
    ];
    for _ in 0..3_000 {
        let target = next(xml_items.len());
        let mut xml = xml_items[target].to_vec();
        for _ in 0..1 + next(6) {
            let at = next(xml.len());
            match next(3) {
                0 => xml[at] = NOISE[next(NOISE.len())],
                1 => {
                    xml.remove(at);
                }
                _ => xml.insert(at, NOISE[next(NOISE.len())]),
            }
        }
        let mut items: Vec<(&str, &[u8], Method)> = vec![
            ("[Content_Types].xml", xml_items[0], Method::Stored),
            ("_rels/.rels", xml_items[1], Method::Stored),
            ("word/_rels/document.xml.rels", xml_items[2], Method::Stored),
            ("word/document.xml", DOCUMENT.as_bytes(), Method::Stored),
            ("word/media/a.png", b"png", Method::Stored),
        ];
        items[target].1 = &xml;
        let _ = read(&write_archive(&items).unwrap());
    }
}

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_protect::crypto::{
    aes_cbc_decrypt, aes_cbc_encrypt, hmac_sha256, sha1, sha256, sha512,
};
use joyword_protect::{
    ActiveContentAuditor, ActiveContentKind, DocumentProtection, EncryptedPackageReader,
    EncryptionError, FormatSpoofRisk, ProtectionMode, SignatureStatus, SignatureVerifier,
};

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="bin" ContentType="application/vnd.ms-office.vbaProject"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>合同正文</w:t></w:r></w:p></w:body></w:document>"#;

fn make_package(extra: &[(&str, &[u8])]) -> Package {
    let mut items: Vec<(&str, &[u8], Method)> = vec![
        (
            "[Content_Types].xml",
            CONTENT_TYPES.as_bytes(),
            Method::Stored,
        ),
        ("_rels/.rels", PACKAGE_RELS.as_bytes(), Method::Stored),
    ];
    let has_doc = extra.iter().any(|(name, _)| *name == "word/document.xml");
    if !has_doc {
        items.push(("word/document.xml", DOCUMENT.as_bytes(), Method::Stored));
    }
    for (name, data) in extra {
        items.push((*name, *data, Method::Stored));
    }
    let zip_bytes = write_archive(&items).expect("write archive");
    Package::read(&zip_bytes, &Budget::default()).expect("read package")
}

#[test]
fn test_crypto_primitives() {
    let digest = sha256(b"abc");
    assert_eq!(
        format!("{:02x?}", digest)
            .replace(", ", "")
            .replace(['[', ']'], ""),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    let digest1 = sha1(b"abc");
    assert_eq!(
        format!("{:02x?}", digest1)
            .replace(", ", "")
            .replace(['[', ']'], ""),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );

    let mac = hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog");
    assert_eq!(mac.len(), 32);

    let key = [0x42u8; 32];
    let iv = [0x24u8; 16];
    let plaintext = b"JoyOffice confidential document content...";
    let enc = aes_cbc_encrypt(&key, &iv, plaintext).expect("encrypt");
    let dec = aes_cbc_decrypt(&key, &iv, &enc).expect("decrypt");
    assert_eq!(dec, plaintext);
}

#[test]
fn test_document_protection_password() {
    let password = "SecretPassword123";
    let salt = vec![0x11, 0x22, 0x33, 0x44];
    let spin = 100u32;

    let mut pwd_bytes = Vec::new();
    for c in password.encode_utf16() {
        pwd_bytes.extend_from_slice(&c.to_le_bytes());
    }
    let mut combined = salt.clone();
    combined.extend_from_slice(&pwd_bytes);
    let mut cur_hash = sha512(&combined).to_vec();
    for i in 0..spin {
        let mut buf = cur_hash;
        buf.extend_from_slice(&i.to_le_bytes());
        cur_hash = sha512(&buf).to_vec();
    }

    use joyword_protect::encrypted_package::encode_base64;
    let b64_hash = encode_base64(&cur_hash);
    let b64_salt = encode_base64(&salt);

    let dp = DocumentProtection::parse(
        Some("readOnly"),
        Some("1"),
        Some("SHA-512"),
        Some(&b64_hash),
        Some(&b64_salt),
        Some("100"),
    )
    .expect("parse document protection");

    assert_eq!(dp.mode, ProtectionMode::ReadOnly);
    assert!(dp.enforced);
    assert!(dp.verify_password("SecretPassword123"));
    assert!(!dp.verify_password("WrongPassword"));
}

#[test]
fn test_active_content_audit() {
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
    <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
      <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate" Target="\\malicious\share\template.dotm" TargetMode="External"/>
    </Relationships>"#;

    let vba_bytes = vec![0xAA; 100];
    let pkg = make_package(&[
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("word/vbaProject.bin", &vba_bytes),
    ]);

    let findings = ActiveContentAuditor::audit_opc_package(&pkg);
    assert!(findings.contains(&ActiveContentKind::VbaMacro));
    assert!(findings.iter().any(|f| matches!(
        f,
        ActiveContentKind::SuspiciousExternalLink(msg) if msg.contains("network share/UNC")
    )));

    let elf_bytes = [0x7f, b'E', b'L', b'F', 0, 0, 0, 0];
    assert!(matches!(
        ActiveContentAuditor::check_extension_spoofing("doc", &elf_bytes),
        FormatSpoofRisk::ExecutableFile(_)
    ));
}

#[test]
fn test_signature_verification_and_tamper_detection() {
    let digest = sha256(DOCUMENT.as_bytes());
    use joyword_protect::encrypted_package::encode_base64;
    let digest_b64 = encode_base64(&digest);

    let sig_xml = format!(
        r#"<Signature xmlns="http://www.w3.org/2000/09/xmldsig#">
            <SignedInfo>
                <Reference URI="/word/document.xml">
                    <DigestMethod Algorithm="http://www.w3.org/2001/04/xmlenc#sha256"/>
                    <DigestValue>{}</DigestValue>
                </Reference>
            </SignedInfo>
        </Signature>"#,
        digest_b64
    );

    let valid_pkg = make_package(&[("_xmlsignatures/sig1.xml", sig_xml.as_bytes())]);
    let results = SignatureVerifier::verify_package_signatures(&valid_pkg);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, SignatureStatus::Valid);

    // Tamper document part
    let tampered_doc = b"<w:document>TAMPERED</w:document>";
    let tampered_pkg = make_package(&[
        ("word/document.xml", tampered_doc),
        ("_xmlsignatures/sig1.xml", sig_xml.as_bytes()),
    ]);
    let tampered_results = SignatureVerifier::verify_package_signatures(&tampered_pkg);
    assert!(matches!(
        tampered_results[0].1,
        SignatureStatus::Tampered { .. }
    ));
}

#[test]
fn test_encrypted_package_roundtrip() {
    let original_opc_data = b"PK\x03\x04Test OOXML Package inside encrypted container";
    let password = "JoyOfficeSafePassword2026!";

    let encrypted_cfb =
        EncryptedPackageReader::encrypt_agile(original_opc_data, password).expect("encrypt agile");
    assert!(EncryptedPackageReader::is_encrypted_package(&encrypted_cfb));

    let decrypted =
        EncryptedPackageReader::decrypt(&encrypted_cfb, password).expect("decrypt with password");
    assert_eq!(decrypted, original_opc_data);

    let err = EncryptedPackageReader::decrypt(&encrypted_cfb, "WrongPassword!")
        .expect_err("should fail with wrong password");
    assert_eq!(err, EncryptionError::InvalidPassword);
}

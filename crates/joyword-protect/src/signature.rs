//! XML-DSig signature verification and tamper detection for OPC packages.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::crypto::{constant_time_eq, sha1, sha256, sha512};
use crate::protection::decode_base64;
use joyoffice_opc::Package;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureStatus {
    Valid,
    Tampered {
        part: String,
        expected_digest: Vec<u8>,
        actual_digest: Vec<u8>,
    },
    MissingPart(String),
    MalformedSignature(String),
}

pub struct SignatureVerifier;

impl SignatureVerifier {
    /// Inspect package for digital signatures and verify integrity of signed parts.
    /// Returns a list of (signature_part_name, SignatureStatus).
    pub fn verify_package_signatures(package: &Package) -> Vec<(String, SignatureStatus)> {
        let mut results = Vec::new();

        for part in package.parts() {
            let path = part.name.as_str();
            if path.starts_with("/_xmlsignatures/sig") && path.ends_with(".xml") {
                let status = Self::verify_signature_xml(&part.data, package);
                results.push((path.to_string(), status));
            }
        }

        results
    }

    fn verify_signature_xml(xml_bytes: &[u8], package: &Package) -> SignatureStatus {
        let mut reader = Reader::from_reader(xml_bytes);
        reader.config_mut().trim_text(true);

        let mut curr_uri: Option<String> = None;
        let mut curr_alg: Option<String> = None;
        let mut in_digest_value = false;

        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let name = e.local_name();
                    if name.as_ref() == b"Reference" {
                        for attr in e.attributes().flatten() {
                            if let (b"URI", Ok(val)) =
                                (attr.key.as_ref(), std::str::from_utf8(&attr.value))
                            {
                                curr_uri = Some(val.to_string());
                            }
                        }
                    } else if name.as_ref() == b"DigestMethod" {
                        for attr in e.attributes().flatten() {
                            if let (b"Algorithm", Ok(val)) =
                                (attr.key.as_ref(), std::str::from_utf8(&attr.value))
                            {
                                curr_alg = Some(val.to_string());
                            }
                        }
                    } else if name.as_ref() == b"DigestValue" {
                        in_digest_value = true;
                    }
                }
                Ok(Event::Text(ref e)) if in_digest_value => {
                    let text = match std::str::from_utf8(e.as_ref()) {
                        Ok(t) => t.trim().to_string(),
                        Err(_) => {
                            return SignatureStatus::MalformedSignature("bad utf8 digest".into());
                        }
                    };
                    let expected_digest = match decode_base64(&text) {
                        Some(d) => d,
                        None => {
                            return SignatureStatus::MalformedSignature(
                                "base64 decode error".into(),
                            );
                        }
                    };

                    if let Some(ref uri) = curr_uri {
                        let clean_uri = uri.split('?').next().unwrap_or(uri);
                        let actual_bytes = match package.part(clean_uri) {
                            Some(p) => &p.data,
                            None => return SignatureStatus::MissingPart(clean_uri.to_string()),
                        };

                        let alg = curr_alg.as_deref().unwrap_or("");
                        let actual_digest = if alg.contains("sha256") {
                            sha256(actual_bytes).to_vec()
                        } else if alg.contains("sha512") {
                            sha512(actual_bytes).to_vec()
                        } else {
                            sha1(actual_bytes).to_vec()
                        };

                        if !constant_time_eq(&actual_digest, &expected_digest) {
                            return SignatureStatus::Tampered {
                                part: clean_uri.to_string(),
                                expected_digest,
                                actual_digest,
                            };
                        }
                    }
                    in_digest_value = false;
                }
                Ok(Event::End(ref e)) => {
                    if e.local_name().as_ref() == b"Reference" {
                        curr_uri = None;
                        curr_alg = None;
                    } else if e.local_name().as_ref() == b"DigestValue" {
                        in_digest_value = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return SignatureStatus::MalformedSignature(err.to_string()),
                _ => {}
            }
        }

        SignatureStatus::Valid
    }
}

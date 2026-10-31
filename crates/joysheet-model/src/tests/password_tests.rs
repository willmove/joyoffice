use super::*;

#[test]
fn legacy_hash_matches_the_published_example() {
    // The hash Excel writes for the password "password" (`password="83AF"`).
    assert_eq!(legacy_hash("password"), 0x83AF);
    assert!(PasswordHash::Legacy(0x83AF).verify("password").unwrap());
    assert!(!PasswordHash::Legacy(0x83AF).verify("Password").unwrap());
    assert_eq!(legacy_hash(""), 0xCE4B);
}

#[test]
fn iterated_hash_verifies_and_refuses_what_it_cannot_check() {
    let h = PasswordHash::new_sha512("s3cret", b"0123456789abcdef");
    assert!(h.verify("s3cret").unwrap());
    assert!(!h.verify("s3cret ").unwrap());
    assert!(matches!(&h, PasswordHash::Iterated { algorithm, spin_count: DEFAULT_SPIN_COUNT, .. } if algorithm == "SHA-512"));
    let PasswordHash::Iterated { hash, salt, .. } = h else { return };
    let other =
        |algorithm: &str, spin_count| PasswordHash::Iterated { algorithm: algorithm.into(), hash: hash.clone(), salt: salt.clone(), spin_count };
    assert!(other("SHA-1", 10).verify("s3cret").is_err());
    assert!(other("MD5", 10).verify("s3cret").is_err());
    assert!(other("SHA-512", MAX_SPIN_COUNT + 1).verify("s3cret").is_err());
    assert!(!other("SHA-256", 2).verify("s3cret").unwrap());
    let broken = PasswordHash::Iterated { algorithm: "SHA-512".into(), hash: "!!".into(), salt: salt.clone(), spin_count: 1 };
    assert!(broken.verify("s3cret").is_err());
}

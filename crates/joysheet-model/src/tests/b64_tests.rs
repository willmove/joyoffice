#[test]
fn roundtrip() {
    for data in [&b""[..], b"a", b"ab", b"abc", b"hello world", &[0, 255, 128, 7]] {
        assert_eq!(super::decode(&super::encode(data)).unwrap(), data);
    }
    assert_eq!(super::encode(b"Man"), "TWFu");
}

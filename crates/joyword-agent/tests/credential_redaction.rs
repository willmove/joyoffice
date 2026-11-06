//! Synthetic strings only; no system credential access.
use joyword_agent::redact;

#[test]
fn overlapping_credentials_do_not_leave_secret_fragments() {
    for secrets in [["token", "token-private"], ["token-private", "token"]] {
        assert_eq!(redact("key=token-private!", &secrets), "key=[redacted]!");
    }
    assert_eq!(redact("ababa", &["aba"]), "[redacted]");
    assert_eq!(redact("abcde", &["abc", "cde"]), "[redacted]");
}

#[test]
fn replacements_are_not_rescanned_as_credentials() {
    assert_eq!(
        redact("key=synthetic", &["synthetic", "redacted"]),
        "key=[redacted]"
    );
    assert_eq!(
        redact("synthetic synthetic", &["synthetic", "[redacted]"]),
        "[redacted] [redacted]"
    );
}

#[test]
fn unicode_empty_and_adjacent_credentials_preserve_other_text() {
    assert_eq!(
        redact("前缀密钥尾后缀", &["密钥", "钥尾", ""]),
        "前缀[redacted]后缀"
    );
    assert_eq!(redact("甲乙", &["甲", "乙"]), "[redacted][redacted]");
    assert_eq!(redact("unchanged 中文", &[""]), "unchanged 中文");
    assert_eq!(redact("", &["synthetic"]), "");
}

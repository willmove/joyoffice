use super::*;

#[test]
fn edits1_is_capped_for_very_long_words() {
    assert_eq!(edits1("helo").len(), 54 * 4 + 25);
    assert_eq!(edits1(&"a".repeat(MAX_SUGGEST_LEN)).len(), 54 * MAX_SUGGEST_LEN + 25);
    assert!(edits1(&"a".repeat(MAX_SUGGEST_LEN + 1)).is_empty());
    assert!(edits1(&"a".repeat(1_000_000)).is_empty());
}

use super::*;

#[test]
fn strings_continue_with_a_new_compression_flag() {
    // "ab" compressed, then "ç" and "d" as UTF-16 in a Continue part.
    let first: &[u8] = &[4, 0, 0, b'a', b'b'];
    let second: &[u8] = &[1, 0xE7, 0, b'd', 0];
    let parts = [first, second];
    let mut c = Cursor::new(&parts);
    assert_eq!(c.rich_string().unwrap(), "abçd");
    assert_eq!(c.remaining(), 0);
    let mut c = Cursor::new(&parts);
    assert!(c.chars(MAX_CHARS + 1, false).is_err());
}

#[test]
fn remaining_counts_every_part_wherever_the_cursor_is() {
    let parts: [&[u8]; 4] = [&[1, 2, 3], &[2, 3, 4, 5], &[3, 4, 5, 6, 7], &[4]];
    let mut c = Cursor::new(&parts);
    assert_eq!(c.remaining(), 13);
    for b in 1..=3u64 {
        assert_eq!(u64::from(c.u8().unwrap()), b, "part 0 in order");
    }
    assert_eq!(c.remaining(), 10, "after the first part is used up");
    for _ in 0..4 {
        c.u8().unwrap();
    }
    assert_eq!(c.remaining(), 6, "into the Continue parts");
    let mut c = Cursor::new(&parts);
    assert_eq!(u64::from(c.u8().unwrap()), 1);
    assert_eq!(c.rest().len(), 12);
    assert_eq!(c.remaining(), 0);
}

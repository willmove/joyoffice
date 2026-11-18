//! Hunspell dictionary entries and the byte-prefix records in installed compact dictionaries.
use std::borrow::Cow;

use super::flags::VARIATION_SELECTOR;

/// Stop before a tab or a space introducing a two-byte morphological field name.
/// The caller can apply this to a legacy byte encoding before decoding the flag field.
pub fn entry_bytes(line: &[u8]) -> &[u8] {
    let blank = |ch| matches!(ch, b' ' | b'\t');
    let mut last = 0;
    for (i, &ch) in line.iter().enumerate() {
        if ch == b'\t' {
            break;
        }
        if ch == b' ' {
            if let Some([a, b, b':']) = line.get(i + 1..i + 4)
                && !blank(*a)
                && !blank(*b)
            {
                break;
            }
        } else {
            last = i + 1;
        }
    }
    &line[..last]
}

pub struct Entry<'a> {
    pub root: Cow<'a, str>,
    pub flags: Cow<'a, str>,
}

pub fn entry(line: &str) -> Entry<'_> {
    // entry_bytes removes only whole ASCII delimiters and trailing spaces.
    let text = &line[..entry_bytes(line.as_bytes()).len()];
    let mut split = text.len();
    let mut escaped = false;
    for (at, _) in text.match_indices('/') {
        if at > 0 && text.as_bytes()[at - 1] == b'\\' {
            escaped = true;
        } else if at != 0 {
            split = at;
            break;
        }
    }
    let word = &text[..split];
    let flags = text.get(split + 1..).unwrap_or_default();
    Entry {
        root: if escaped {
            Cow::Owned(word.replace("\\/", "/"))
        } else {
            Cow::Borrowed(word)
        },
        flags: if flags.contains(VARIATION_SELECTOR) {
            Cow::Owned(flags.replace(VARIATION_SELECTOR, ""))
        } else {
            Cow::Borrowed(flags)
        },
    }
}

/// Hunspell AF fields use a one-based decimal alias followed by optional metadata.
pub fn alias_index(written: &str, aliases: usize) -> Option<usize> {
    let end = written
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(written.len());
    let index = written[..end].parse::<usize>().ok()?.checked_sub(1)?;
    (index < aliases).then_some(index)
}

/// In WCAFF2 the first byte is a length, even when its value is a slash or newline.
/// Prefixes count bytes and can end inside a UTF-8 scalar.
pub fn write_compact_root(output: &mut Vec<u8>, previous: &[u8], root: &[u8]) {
    let limit = previous.len().min(root.len()).min(usize::from(u8::MAX));
    let shared = previous[..limit]
        .iter()
        .zip(root)
        .position(|(a, b)| a != b)
        .unwrap_or(limit);
    output.push(shared as u8);
    output.extend_from_slice(&root[shared..]);
}

/// Decode into one bounded prefix buffer. Flag fields borrow the input, including for homonyms.
pub fn visit_compact_roots<'a>(
    mut body: &'a [u8],
    max_word: usize,
    mut accept: impl FnMut(&str, &'a str) -> Result<(), String>,
) -> Result<(), String> {
    let mut root = Vec::new();
    while let Some((&shared, record)) = body.split_first() {
        let end = record
            .iter()
            .position(|&b| b == b'\n')
            .ok_or("affix dictionary: truncated")?;
        let (row, rest) = record.split_at(end);
        body = &rest[1..];
        let shared = usize::from(shared);
        if shared > root.len() {
            return Err("affix dictionary: bad prefix".into());
        }
        let (suffix, flags) = match row.iter().position(|&b| b == b'/') {
            Some(i) => (&row[..i], &row[i + 1..]),
            None => (row, &[][..]),
        };
        if shared == 0 && suffix.is_empty() || suffix.len() > max_word.saturating_sub(shared) {
            return Err("affix dictionary: bad stem".into());
        }
        root.truncate(shared);
        root.extend_from_slice(suffix);
        let word =
            std::str::from_utf8(&root).map_err(|_| "affix dictionary: bad utf-8 in a stem")?;
        let flags =
            std::str::from_utf8(flags).map_err(|_| "affix dictionary: bad utf-8 in flags")?;
        accept(word, flags)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_rows_split_morphology_without_allocating_plain_words_or_aliases() {
        for (line, word, flags) in [
            ("ad hoc/2tail  po:adv", "ad hoc", "2tail"),
            ("  lab\tst:lab", "  lab", ""),
            ("co\\/op/7", "co/op", "7"),
            ("/path/X", "/path", "X"),
            ("no/☎️", "no", "☎"),
            ("ação   ", "ação", ""),
        ] {
            let parsed = entry(line);
            assert_eq!((parsed.root.as_ref(), parsed.flags.as_ref()), (word, flags));
        }
        assert!(matches!(entry("ad hoc/21").root, Cow::Borrowed(_)));
        assert!(matches!(entry("ad hoc/21").flags, Cow::Borrowed(_)));
        assert_eq!(
            entry_bytes(&[0xff, b'/', 0x80, b' ', b's', b't', b':', b'x']),
            &[0xff, b'/', 0x80]
        );
        assert_eq!(alias_index("0002extra", 2), Some(1));
        assert_eq!(alias_index("0", 2), None);
        assert_eq!(alias_index("99999999999999999999999999", 2), None);
    }

    #[test]
    fn prefix_records_span_scalars_and_binary_delimiters_and_reject_incomplete_data() {
        let prefix = "x".repeat(47);
        let stems = [
            "abcdefghijA".to_owned(),
            "abcdefghijB".to_owned(),
            prefix.clone() + "a",
            prefix + "b",
            "鲁".to_owned(),
            "鲃".to_owned(),
            "鲃".to_owned(),
        ];
        let mut bytes = Vec::new();
        let mut previous = "";
        for stem in &stems {
            write_compact_root(&mut bytes, previous.as_bytes(), stem.as_bytes());
            bytes.extend_from_slice(b"/AA\n");
            previous = stem;
        }
        let mut read = Vec::new();
        visit_compact_roots(&bytes, 120, |word, flags| {
            assert_eq!(flags, "AA");
            read.push(word.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(read, stems);
        for (raw, expected) in [
            (&b"\x00abc"[..], "truncated"),
            (b"\x01abc\n", "bad prefix"),
            (b"\x00\n", "bad stem"),
            (b"\x00abcde\n", "bad stem"),
            (b"\x00\xff\n", "bad utf-8 in a stem"),
            (b"\x00abc/\xff\n", "bad utf-8 in flags"),
        ] {
            let result = visit_compact_roots(raw, 4, |_, _| Ok(()));
            assert_eq!(result.unwrap_err(), format!("affix dictionary: {expected}"));
        }
    }
}

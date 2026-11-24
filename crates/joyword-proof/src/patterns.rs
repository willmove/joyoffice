//! Liang hyphenation patterns (TeX notation, e.g. `.ab1c`, `4t1ion`): parsing and application.
//!
//! The bundled English set is *ours*: trained from the public-domain Moby Hyphenator word list by
//! DesignCraft's patgen (see `examples/hyphgen.rs`). The Polish set is the TeX one by Hanna
//! Kołodziejska, Bogusław Jackowski and Marek Ryćko (hyph-utf8 `hyph-pl.tex`, used under its MIT
//! licence option), bundled unmodified: [`Patterns::parse`] reads TeX's `\patterns{…}` and
//! `\hyphenation{…}` blocks directly. The runtime matcher follows letter edges in a trie and
//! combines the weights of every overlapping pattern as described in Liang's 1983 thesis.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::num::NonZeroU32;

/// Word-boundary marker in patterns.
pub const DOT: char = '.';
/// Longest pattern the packed key supports (6 bits a letter in a `u128`; the Polish set's longest
/// is 14 including the dots).
pub const MAX_PATTERN_LEN: usize = 20;

/// Small, fast multiplicative hasher for packed integer keys and short strings (FxHash-style).
#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.write_u64(u64::from_le_bytes(*c));
        }
        for &b in rest {
            self.write_u64(b as u64);
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64);
    }
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_u128(&mut self, i: u128) {
        self.write_u64(i as u64);
        self.write_u64((i >> 64) as u64);
    }
}

pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

/// Maps characters to 6-bit codes (1 = word boundary, 2.. = letters); 0 = not in the alphabet.
#[derive(Clone, Debug)]
pub struct Alphabet {
    ascii: [u8; 128],
    other: Vec<(char, u8)>,
    next: u8,
}

impl Default for Alphabet {
    fn default() -> Self {
        let mut a = Alphabet {
            ascii: [0; 128],
            other: Vec::new(),
            next: 2,
        };
        a.ascii[DOT as usize] = 1;
        a
    }
}

impl Alphabet {
    pub fn code(&self, c: char) -> u8 {
        if (c as u32) < 128 {
            self.ascii[c as usize]
        } else {
            self.other.iter().find(|x| x.0 == c).map_or(0, |x| x.1)
        }
    }
    /// Code for `c`, adding it if new (None when the 6-bit alphabet is full).
    pub fn intern(&mut self, c: char) -> Option<u8> {
        let k = self.code(c);
        if k != 0 {
            return Some(k);
        }
        if self.next >= 63 {
            return None;
        }
        let k = self.next;
        self.next += 1;
        if (c as u32) < 128 {
            self.ascii[c as usize] = k;
        } else {
            self.other.push((c, k));
        }
        Some(k)
    }
}

/// Packed key of a code string (≤ [`MAX_PATTERN_LEN`] codes): 6 bits a code, the length on top.
#[inline]
pub fn pack(codes: &[u8]) -> u128 {
    let mut k = 0u128;
    for &c in codes {
        k = (k << 6) | c as u128;
    }
    k | ((codes.len() as u128) << 120)
}

/// A compiled pattern set, with its exception words (TeX's `\hyphenation{…}`).
#[derive(Clone, Debug, Default)]
pub struct Patterns {
    alpha: Alphabet,
    nodes: Vec<Node>,
    weights: Vec<u8>,
    pattern_count: usize,
    /// Lowercase word → its break points (char indices), overriding the patterns.
    exceptions: HashMap<String, Box<[usize]>>,
}

#[derive(Clone, Debug, Default)]
struct Node {
    child: Option<NonZeroU32>,
    sibling: Option<NonZeroU32>,
    weight_offset: u32,
    weight_count: u8,
    letter: u8,
}

/// Which TeX block [`Patterns::parse`] is reading.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Patterns,
    Exceptions,
    /// Another command's argument (`\message{…}`), ignored.
    Skip,
}

impl Patterns {
    /// Parse patterns: whitespace-separated, `%` comments to the end of the line (`#` too at the
    /// start of a line). A whole TeX pattern file works as is: `\patterns{…}` holds patterns and
    /// `\hyphenation{…}` exception words written with their hyphens (`ni-gdy`), and other
    /// commands' arguments (`\message{…}`) are skipped; outside any block, a token with a hyphen
    /// is an exception and anything else a pattern.
    pub fn parse(text: &str) -> Patterns {
        Self::parse_bounded(text, u64::MAX).unwrap_or_default()
    }

    pub(crate) fn parse_bounded(text: &str, trie_budget: u64) -> Option<Self> {
        let mut p = Patterns::default();
        let mut block = Block::Patterns;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let line = line.split('%').next().unwrap_or("");
            for tok in line.split_whitespace() {
                let mut tok = tok;
                if let Some(cmd) = tok.strip_prefix('\\') {
                    let (name, rest) = cmd.split_once('{').unwrap_or((cmd, ""));
                    block = match name {
                        "hyphenation" => Block::Exceptions,
                        "patterns" => Block::Patterns,
                        // shortcut: the argument ends at the first `}`, so nested braces would leak; no bundled file has them.
                        _ if cmd.contains('{') => Block::Skip,
                        _ => Block::Patterns,
                    };
                    tok = rest;
                }
                let closes = tok.ends_with('}');
                let tok = tok.trim_end_matches('}');
                if !tok.is_empty() && block != Block::Skip {
                    if block == Block::Exceptions || tok.contains('-') {
                        p.insert_exception(tok);
                    } else {
                        p.insert(tok);
                        if p.pattern_storage_bytes() > trie_budget {
                            return None;
                        }
                    }
                }
                if closes {
                    block = Block::Patterns;
                }
            }
        }
        Some(p)
    }

    /// Add one pattern like `a1b`; returns false if it can't be represented.
    pub fn insert(&mut self, pat: &str) -> bool {
        let mut codes = Vec::with_capacity(pat.len());
        let mut vals = vec![0u8];
        for c in pat.chars() {
            if let Some(d) = c.to_digit(10) {
                *vals.last_mut().unwrap_or(&mut 0) = d as u8;
            } else {
                let Some(k) = self.alpha.intern(c) else {
                    return false;
                };
                codes.push(k);
                vals.push(0);
            }
        }
        if codes.is_empty() || codes.len() > MAX_PATTERN_LEN {
            return false;
        }
        if self.nodes.is_empty() {
            self.nodes.push(Node::default());
        }
        let mut index = 0;
        for code in codes {
            if let Some(child) = self.edge(index, code) {
                index = child;
                continue;
            }
            let Some(child) = u32::try_from(self.nodes.len())
                .ok()
                .and_then(NonZeroU32::new)
            else {
                return false;
            };
            let sibling = self.nodes[index].child.replace(child);
            self.nodes.push(Node {
                letter: code,
                sibling,
                ..Node::default()
            });
            index = child.get() as usize;
        }
        let node = &mut self.nodes[index];
        if node.weight_count == 0 {
            let Ok(start) = u32::try_from(self.weights.len()) else {
                return false;
            };
            node.weight_offset = start;
            node.weight_count = vals.len() as u8;
            self.weights.extend(vals);
            self.pattern_count += 1;
        } else {
            let start = node.weight_offset as usize;
            self.weights[start..start + vals.len()].copy_from_slice(&vals);
        }
        true
    }

    fn edge(&self, index: usize, code: u8) -> Option<usize> {
        self.children(index)
            .find(|&child| self.nodes[child].letter == code)
    }

    fn children(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.nodes[index].child, |child| {
            self.nodes[child.get() as usize].sibling
        })
        .map(|child| child.get() as usize)
    }

    fn votes(&self, index: usize) -> &[u8] {
        let node = &self.nodes[index];
        let start = node.weight_offset as usize;
        &self.weights[start..start + usize::from(node.weight_count)]
    }

    /// Add an exception word written with its hyphens (`ni-gdy`; no hyphen: never broken).
    pub fn insert_exception(&mut self, word: &str) {
        let mut plain = String::with_capacity(word.len());
        let mut points = Vec::new();
        let mut n = 0usize;
        for c in word.chars() {
            if c == '-' {
                if n > 0 && points.last() != Some(&n) {
                    points.push(n);
                }
            } else {
                plain.extend(c.to_lowercase());
                n += 1;
            }
        }
        points.retain(|&i| i < n);
        if n > 0 {
            self.exceptions.insert(plain, points.into_boxed_slice());
        }
    }

    /// Break points of an exception word (lowercase), if it is one.
    pub fn exception(&self, word: &str) -> Option<&[usize]> {
        self.exceptions.get(word).map(|b| &b[..])
    }

    /// Number of exception words.
    pub fn exception_count(&self) -> usize {
        self.exceptions.len()
    }

    /// Make `letters` part of the alphabet even if no pattern mentions them.
    pub fn add_letters(&mut self, letters: impl IntoIterator<Item = char>) {
        for c in letters {
            let _ = self.alpha.intern(c);
        }
    }

    pub fn len(&self) -> usize {
        self.pattern_count
    }

    pub(crate) fn pattern_storage_bytes(&self) -> u64 {
        (self.nodes.capacity() * std::mem::size_of::<Node>() + self.weights.capacity()) as u64
    }

    pub fn is_empty(&self) -> bool {
        self.pattern_count == 0
    }

    /// Inter-letter values for a lowercase word: `out[i]` is the value *before* char `i`
    /// (`out.len() == word.len() + 1`). None if a char is outside the pattern alphabet.
    pub fn values(&self, word: &[char]) -> Option<Vec<u8>> {
        let mut codes = Vec::with_capacity(word.len() + 2);
        codes.push(1);
        for &c in word {
            let k = self.alpha.code(c);
            if k == 0 {
                return None;
            }
            codes.push(k);
        }
        codes.push(1);
        let mut weights = vec![0; word.len() + 1];
        if !self.nodes.is_empty() {
            for start in 0..codes.len() {
                let mut node = 0;
                for &code in &codes[start..] {
                    let Some(child) = self.edge(node, code) else {
                        break;
                    };
                    node = child;
                    // A gap outside either boundary marker is outside the result.
                    for (out, vote) in weights[start.saturating_sub(1)..]
                        .iter_mut()
                        .zip(self.votes(node).iter().skip(usize::from(start == 0)))
                    {
                        *out = (*out).max(*vote);
                    }
                }
            }
        }
        Some(weights)
    }

    /// Break points (char indices, a hyphen goes before that char) of a lowercase word.
    pub fn points(&self, word: &[char]) -> Option<Vec<usize>> {
        let v = self.values(word)?;
        Some(
            (1..word.len())
                .filter(|&i| v.get(i).is_some_and(|x| x % 2 == 1))
                .collect(),
        )
    }

    /// The patterns in TeX notation (sorted by letters), e.g. for writing a pattern file.
    pub fn to_tex(&self) -> Vec<String> {
        let mut rev: Vec<(u8, char)> = (0u8..128)
            .filter(|&c| self.alpha.ascii[c as usize] != 0)
            .map(|c| (self.alpha.ascii[c as usize], c as char))
            .collect();
        rev.extend(self.alpha.other.iter().map(|&(c, k)| (k, c)));
        let mut alphabet = ['?'; 64];
        for (code, letter) in rev {
            alphabet[usize::from(code)] = letter;
        }
        let mut out = Vec::with_capacity(self.pattern_count);
        if !self.nodes.is_empty() {
            self.write_patterns(0, &mut Vec::new(), &alphabet, &mut out);
        }
        out.sort();
        out.into_iter().map(|x| x.1).collect()
    }

    fn write_patterns(
        &self,
        index: usize,
        letters: &mut Vec<char>,
        alphabet: &[char; 64],
        out: &mut Vec<(String, String)>,
    ) {
        let weights = self.votes(index);
        if !weights.is_empty() {
            let mut notation = String::new();
            for (i, &weight) in weights.iter().enumerate() {
                if weight != 0 {
                    notation.push(char::from(b'0' + weight));
                }
                if let Some(&letter) = letters.get(i) {
                    notation.push(letter);
                }
            }
            out.push((letters.iter().collect(), notation));
        }
        for child in self.children(index) {
            letters.push(alphabet[usize::from(self.nodes[child].letter)]);
            self.write_patterns(child, letters, alphabet, out);
            letters.pop();
        }
    }
}

/// Apply `map` to padded `codes`, raising `vals` (len = codes.len() + 1; shorter is tolerated).
#[inline]
pub fn apply(map: &FxMap<u128, Box<[u8]>>, max_len: usize, codes: &[u8], vals: &mut [u8]) {
    let n = codes.len();
    for s in 0..n {
        let mut k = 0u128;
        for (len, &c) in codes
            .iter()
            .skip(s)
            .take(max_len.min(MAX_PATTERN_LEN))
            .enumerate()
        {
            k = (k << 6) | c as u128;
            if let Some(v) = map.get(&(k | (((len + 1) as u128) << 120))) {
                for (j, &x) in v.iter().enumerate() {
                    if let Some(slot) = vals.get_mut(s + j)
                        && x > *slot
                    {
                        *slot = x;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liang_example() {
        // The classic example from Liang's thesis: "hy-phen-ation".
        let p = Patterns::parse("hy3ph he2n hena4 hen5at 1na n2at 1tio 2io o2n");
        let w: Vec<char> = "hyphenation".chars().collect();
        assert_eq!(p.points(&w).unwrap(), vec![2, 6]);
        let mut tex = p.to_tex();
        tex.sort();
        assert!(tex.contains(&"hen5at".to_string()));
        assert!(p.values(&['h', 'é']).is_none());
    }

    #[test]
    fn tex_files_parse_with_blocks_comments_and_exceptions() {
        let p = Patterns::parse(
            "% a comment\n\\patterns{\n1na n2at % trailing comment\n.ab4cdefghijklm5n\n}\n\\hyphenation{\nni-gdy\nna-dal\nzawsze\n}\nhy-phen-ation\n",
        );
        assert_eq!(p.len(), 3);
        assert_eq!(p.exception("nigdy"), Some(&[2][..]));
        assert_eq!(p.exception("nadal"), Some(&[2][..]));
        assert_eq!(p.exception("zawsze"), Some(&[][..]));
        assert_eq!(p.exception("hyphenation"), Some(&[2, 6][..]));
        assert_eq!(p.exception_count(), 4);
        // A pattern longer than the old 9-letter limit applies.
        let w: Vec<char> = "abcdefghijklmn".chars().collect();
        assert_eq!(p.values(&w).unwrap()[13], 5);
        assert!(p.to_tex().contains(&".ab4cdefghijklm5n".to_string()));
    }

    #[test]
    fn other_commands_arguments_are_skipped() {
        let p = Patterns::parse(
            "\\message{Some Patterns `x-y' 2024-02-28}\n\\patterns{%\na1b\n}\n\\endinput\n",
        );
        assert_eq!((p.len(), p.exception_count()), (1, 0));
    }

    #[test]
    fn trie_matches_all_substrings_for_five_language_tables() {
        let custom = Patterns::parse("1a a2b a5b a2b3c .a6bc1d 2de. de7a");
        for set in [
            &custom,
            Patterns::en_us(),
            Patterns::de(),
            Patterns::it(),
            Patterns::pl(),
            Patterns::pt(),
        ] {
            let patterns = set.to_tex();
            let legacy: FxMap<_, _> = patterns
                .iter()
                .map(|pattern| {
                    let mut codes = Vec::new();
                    let mut weights = vec![0];
                    for ch in pattern.chars() {
                        if let Some(digit) = ch.to_digit(10) {
                            let gap = weights.len() - 1;
                            weights[gap] = digit as u8;
                        } else {
                            codes.push(set.alpha.code(ch));
                            weights.push(0);
                        }
                    }
                    (pack(&codes), weights.into_boxed_slice())
                })
                .collect();
            // Include patterns throughout each alphabet and patterns longer than their prefixes.
            for pattern in patterns
                .iter()
                .step_by((patterns.len() / 256).max(1))
                .take(256)
            {
                let word: Vec<char> = pattern.chars().filter(|c| c.is_alphabetic()).collect();
                let codes: Vec<u8> = std::iter::once(1)
                    .chain(word.iter().map(|&c| set.alpha.code(c)))
                    .chain(std::iter::once(1))
                    .collect();
                let mut reference = vec![0; codes.len() + 1];
                apply(&legacy, MAX_PATTERN_LEN, &codes, &mut reference);
                assert_eq!(
                    set.values(&word).as_deref(),
                    Some(&reference[1..codes.len()]),
                    "{pattern}"
                );
            }
        }
        assert!(custom.values(&['λ']).is_none());
    }

    #[test]
    fn apply_tolerates_short_value_buffers() {
        let mut patterns = FxMap::default();
        patterns.insert(pack(&[2, 3]), vec![0, 1, 0].into_boxed_slice());
        let mut vals = [0u8; 1];
        apply(&patterns, MAX_PATTERN_LEN, &[1, 2, 3, 1], &mut vals);
    }
}

/// One input character of a prefix or suffix condition. Common ASCII classes
/// need two words and a bit test. Other classes preserve exact Unicode scalars.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    Any,
    Letter(char),
    Ascii { bits: [u64; 2], negated: bool },
    Unicode { members: Box<[char]>, negated: bool },
}

impl Condition {
    pub fn matches(&self, letter: char) -> bool {
        match self {
            Self::Any => true,
            Self::Letter(c) => *c == letter,
            Self::Ascii { bits, negated } => {
                let bit = letter as usize;
                (bit < 128 && bits[bit / 64] & (1 << (bit % 64)) != 0) != *negated
            }
            Self::Unicode { members, negated } => {
                members.binary_search(&letter).is_ok() != *negated
            }
        }
    }

    fn class(text: &str) -> Self {
        let negated = text.starts_with('^');
        let members = text.strip_prefix('^').unwrap_or(text);
        if members.is_ascii() {
            let mut bits = [0u64; 2];
            for byte in members.bytes().map(usize::from) {
                bits[byte / 64] |= 1 << (byte % 64);
            }
            Self::Ascii { bits, negated }
        } else {
            let mut chars: Vec<_> = members.chars().collect();
            chars.sort_unstable();
            chars.dedup();
            Self::Unicode {
                members: chars.into(),
                negated,
            }
        }
    }
}

pub fn condition(text: &str) -> Result<Box<[Condition]>, String> {
    if text == "." {
        // An unconditional affix also applies to an empty remaining stem.
        return Ok(Box::default());
    }
    let mut tail = text;
    let mut atoms = Vec::new();
    while let Some(c) = tail.chars().next() {
        tail = &tail[c.len_utf8()..];
        atoms.push(match c {
            '.' => Condition::Any,
            ']' => return Err(format!("stray `]` in condition `{text}`")),
            '[' => {
                let (class, rest) = tail
                    .split_once(']')
                    .ok_or_else(|| format!("unclosed `[` in condition `{text}`"))?;
                tail = rest;
                Condition::class(class)
            }
            letter => Condition::Letter(letter),
        });
    }
    Ok(atoms.into())
}

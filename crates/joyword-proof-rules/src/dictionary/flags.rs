use std::fmt::Write;

pub const MAX_FLAGS_PER_ENTRY: usize = 8192;
pub(super) const VARIATION_SELECTOR: char = '\u{fe0f}';

/// The compact dictionary API also exposes these three flag notations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlagMode {
    #[default]
    Char,
    Long,
    Num,
}

impl FlagMode {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "" | "UTF-8" | "utf-8" | "UTF8" | "DEFAULT" | "default" => Some(Self::Char),
            "long" | "LONG" => Some(Self::Long),
            "num" | "NUM" => Some(Self::Num),
            _ => None,
        }
    }

    pub fn name(self) -> Option<&'static str> {
        match self {
            Self::Char => None,
            Self::Long => Some("long"),
            Self::Num => Some("num"),
        }
    }

    // .dic accepts an incomplete last pair or a bad numerical entry. Rules
    // and saved data use strict decoding and keep errors in input order.
    fn visit(
        self,
        field: &str,
        limit: usize,
        lenient: bool,
        mut emit: impl FnMut(u64),
    ) -> Result<usize, String> {
        let mut count = 0;
        let mut add = |value| {
            if count == limit {
                Err(format!(
                    "more than {limit} flags in `{}`",
                    field.chars().take(40).collect::<String>()
                ))
            } else {
                emit(value);
                count += 1;
                Ok(())
            }
        };
        match self {
            Self::Char => {
                for c in field.chars().filter(|c| *c != VARIATION_SELECTOR) {
                    add(c.into())?;
                }
            }
            Self::Long => {
                let mut letters = field.chars();
                while let Some(first) = letters.next() {
                    match letters.next() {
                        Some(second) => add((u64::from(first) << 32) | u64::from(second))?,
                        None if lenient => break,
                        None => {
                            return Err(format!(
                                "long flags `{field}` have an odd number of characters"
                            ));
                        }
                    }
                }
            }
            Self::Num if !field.is_empty() => {
                for item in field.split(',') {
                    // Some released dictionaries append morphology to a flag.
                    // Only the leading unsigned decimal field names the flag.
                    let mut digits = item.bytes().take_while(u8::is_ascii_digit);
                    let value = digits.next().and_then(|first| {
                        digits.try_fold(u16::from(first - b'0'), |n, digit| {
                            n.checked_mul(10)?.checked_add(u16::from(digit - b'0'))
                        })
                    });
                    match value {
                        Some(n) => add(n.into())?,
                        None if lenient => add(0)?,
                        None => {
                            return Err(format!(
                                "`{}` is not a flag number (0 to 65535)",
                                item.chars().take(20).collect::<String>()
                            ));
                        }
                    }
                }
            }
            Self::Num => {}
        }
        Ok(count)
    }

    pub fn parse(self, field: &str) -> Result<Vec<u64>, String> {
        self.parse_at_most(field, MAX_FLAGS_PER_ENTRY, false)
    }

    pub fn parse_at_most(
        self,
        field: &str,
        limit: usize,
        lenient: bool,
    ) -> Result<Vec<u64>, String> {
        let mut values = Vec::new();
        self.visit(field, limit, lenient, |value| values.push(value))?;
        Ok(values)
    }

    pub fn parse_one(self, field: &str) -> Result<u64, String> {
        let mut value = 0;
        if self.visit(field, MAX_FLAGS_PER_ENTRY, false, |n| value = n)? == 1 {
            Ok(value)
        } else {
            Err(format!("`{field}` is not one flag"))
        }
    }

    pub fn write(self, keys: impl Iterator<Item = u64>, out: &mut String) {
        for (i, key) in keys.enumerate() {
            let scalar = |n| u32::try_from(n).ok().and_then(char::from_u32);
            match self {
                Self::Char => out.extend(scalar(key)),
                Self::Long => {
                    out.extend(scalar(key >> 32));
                    out.extend(scalar(key & u64::from(u32::MAX)));
                }
                Self::Num => {
                    if i > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{key}");
                }
            }
        }
    }
}

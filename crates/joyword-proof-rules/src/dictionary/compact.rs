//! WCAFF2 header framing and directives. The installed byte format separates
//! UTF-8 metadata from byte-prefixed roots with an exact LF-terminated WORDS line.

pub const SIGNATURE: &str = "WCAFF2";

pub struct Header<'a> {
    lines: std::str::Lines<'a>,
    pub roots: &'a [u8],
}

pub enum Setting {
    Flags,
    Try,
    Keyboard,
    Ignore,
    Map,
    CompoundRule,
}

pub enum WordFlag {
    Forbidden,
    NoSuggest,
    Substandard,
    KeepCase,
    CompoundOnly,
    NeedAffix,
    Circumfix,
}

pub struct Affix<'a> {
    pub suffix: bool,
    pub flag: &'a str,
    pub cross: bool,
    pub strip: &'a str,
    pub add: &'a str,
    pub continuation: &'a str,
    pub condition: &'a str,
}

pub enum Directive<'a> {
    Comment(&'a str),
    Setting(Setting, &'a str),
    WordFlag(WordFlag, &'a str),
    FullStrip,
    CheckSharps,
    Conversion {
        output: bool,
        from: &'a str,
        to: &'a str,
    },
    Replace(&'a str, &'a str),
    CompoundMin(usize),
    Affix(Affix<'a>),
    /// The product's compound grammar checks directive names and its own limits.
    Compound(&'a str, Vec<&'a str>),
}

impl<'a> Header<'a> {
    pub fn read(raw: &'a [u8]) -> Result<Self, String> {
        const DELIMITER: &[u8] = b"\nWORDS\n";
        let boundary = raw
            .windows(DELIMITER.len())
            .position(|p| p == DELIMITER)
            .ok_or("affix dictionary: no WORDS section")?;
        let header = std::str::from_utf8(&raw[..boundary])
            .map_err(|_| "affix dictionary: bad utf-8 in the rules")?;
        let mut lines = header.lines();
        if lines.next() != Some(SIGNATURE) {
            return Err("affix dictionary: bad header".into());
        }
        Ok(Self {
            lines,
            roots: &raw[boundary + DELIMITER.len()..],
        })
    }

    pub fn visit(
        self,
        mut accept: impl FnMut(&'a str, Directive<'a>) -> Result<(), String>,
    ) -> Result<(), String> {
        for line in self.lines {
            if let Some(comment) = line.strip_prefix("# ") {
                accept(line, Directive::Comment(comment))?;
                continue;
            }
            // Only ASCII space and tab are delimiters. Non-breaking spaces can be dictionary flags.
            let mut fields = line.split([' ', '\t']).filter(|s| !s.is_empty());
            let Some(key) = fields.next() else { continue };
            let first = fields.next().unwrap_or_default();
            let bad = || format!("affix dictionary: bad `{line}`");
            let record = match key {
                "FLAG" => Directive::Setting(Setting::Flags, first),
                "TRY" => Directive::Setting(Setting::Try, first),
                "KEY" => Directive::Setting(Setting::Keyboard, first),
                "IGNORE" => Directive::Setting(Setting::Ignore, first),
                "MAP" => Directive::Setting(Setting::Map, first),
                "COMPOUNDRULE" => Directive::Setting(Setting::CompoundRule, first),
                "FULLSTRIP" => Directive::FullStrip,
                "CHECKSHARPS" => Directive::CheckSharps,
                "FORBIDDENWORD" => Directive::WordFlag(WordFlag::Forbidden, first),
                "NOSUGGEST" => Directive::WordFlag(WordFlag::NoSuggest, first),
                "SUBSTANDARD" => Directive::WordFlag(WordFlag::Substandard, first),
                "KEEPCASE" => Directive::WordFlag(WordFlag::KeepCase, first),
                "ONLYINCOMPOUND" => Directive::WordFlag(WordFlag::CompoundOnly, first),
                "NEEDAFFIX" => Directive::WordFlag(WordFlag::NeedAffix, first),
                "CIRCUMFIX" => Directive::WordFlag(WordFlag::Circumfix, first),
                "COMPOUNDMIN" => Directive::CompoundMin(first.parse().map_err(|_| bad())?),
                "REP" => Directive::Replace(first, fields.next().unwrap_or_default()),
                "ICONV" | "OCONV" => {
                    let to = fields.next().ok_or_else(bad)?;
                    if fields.next().is_some() {
                        return Err(bad());
                    }
                    Directive::Conversion {
                        output: key == "OCONV",
                        from: first,
                        to,
                    }
                }
                "PFX" | "SFX" => {
                    let cross = fields.next().ok_or_else(bad)? == "Y";
                    let strip = fields.next().ok_or_else(bad)?;
                    let append = fields.next().ok_or_else(bad)?;
                    let condition = fields.next().ok_or_else(bad)?;
                    if fields.next().is_some() {
                        return Err(bad());
                    }
                    let (add, continuation) = append.split_once('/').unwrap_or((append, ""));
                    Directive::Affix(Affix {
                        suffix: key == "SFX",
                        flag: first,
                        cross,
                        strip: if strip == "0" { "" } else { strip },
                        add: if add == "0" { "" } else { add },
                        continuation,
                        condition,
                    })
                }
                _ => {
                    // Compound settings consume at most three values. A fourth preserves the
                    // distinction between an allowed trailing comment and an unexpected field.
                    let args = line
                        .split([' ', '\t'])
                        .filter(|s| !s.is_empty())
                        .take(5)
                        .collect();
                    Directive::Compound(key, args)
                }
            };
            accept(line, record)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_stop_before_arbitrary_binary_roots_and_keep_unicode_flags() {
        let header =
            Header::read("WCAFF2\n# keep this\nTRY\tè\nPFX \u{a0} Y 0 é/7 .\nWORDS\n".as_bytes())
                .unwrap();
        let mut seen = 0;
        header
            .visit(|_, item| {
                match item {
                    Directive::Comment("keep this") | Directive::Setting(Setting::Try, "è") => (),
                    Directive::Affix(rule) => {
                        assert!(rule.cross && !rule.suffix);
                        assert_eq!(
                            (
                                rule.flag,
                                rule.strip,
                                rule.add,
                                rule.continuation,
                                rule.condition
                            ),
                            ("\u{a0}", "", "é", "7", ".")
                        );
                    }
                    _ => panic!("unexpected directive"),
                }
                seen += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(seen, 3);
        assert_eq!(
            Header::read(b"WCAFF2\nWORDS\n\xff\x80").unwrap().roots,
            b"\xff\x80"
        );
        assert!(Header::read(b"WCAFF2\nWORDS").is_err());
        assert!(Header::read(b"WCAFF1\nWORDS\n").is_err());
        assert!(Header::read(b"WCAFF2\n\xff\nWORDS\n").is_err());
        for line in [
            "SFX X Y 0 end",
            "PFX A N 0 0 . extra",
            "ICONV too few extra",
            "OCONV one",
            "COMPOUNDMIN nan",
        ] {
            let raw = format!("WCAFF2\n{line}\nWORDS\n");
            assert!(
                Header::read(raw.as_bytes())
                    .unwrap()
                    .visit(|_, _| Ok(()))
                    .is_err()
            );
        }
    }
}

//! Encode the installed WCAFF2 metadata in canonical order. Comments and
//! language flags are part of the interchange format of installed dictionaries.
use super::{FlagMode, compact::SIGNATURE};
use std::fmt::Write as _;

pub const COMPOUND_FLAGS: [&str; 8] = [
    "COMPOUNDFLAG",
    "COMPOUNDBEGIN",
    "COMPOUNDMIDDLE",
    "COMPOUNDEND",
    "COMPOUNDPERMITFLAG",
    "COMPOUNDFORBIDFLAG",
    "COMPOUNDROOT",
    "FORCEUCASE",
];
pub const COMPOUND_SWITCHES: [&str; 5] = [
    "CHECKCOMPOUNDDUP",
    "CHECKCOMPOUNDREP",
    "CHECKCOMPOUNDCASE",
    "CHECKCOMPOUNDTRIPLE",
    "SIMPLIFIEDTRIPLE",
];

pub struct Metadata<'a> {
    pub mode: FlagMode,
    pub comments: &'a [String],
    pub try_chars: &'a str,
    pub keyboard: &'a str,
    pub map: &'a [Vec<char>],
    pub replacements: &'a [(String, String)],
    pub full_strip: bool,
    pub check_sharps: bool,
    pub word_flags: [Option<u64>; 6],
    pub input: &'a [(String, String)],
    pub output: &'a [(String, String)],
    pub ignore: &'a [char],
    pub language: Option<&'a str>,
    pub compound_min: Option<usize>,
    pub compound_only: Option<u64>,
    pub compound_rules: &'a [String],
    pub edge_flags: [Option<u64>; 2],
}

pub struct Compounds<'a> {
    pub flags: [Option<u64>; 8],
    pub switches: [bool; 5],
    pub hungarian: bool,
    pub word_max: Option<usize>,
    pub syllable: Option<(usize, &'a str)>,
    pub syllable_num: Option<&'a str>,
    pub no_suggestions: bool,
    pub no_split_suggestions: bool,
    pub patterns: &'a [(String, String)],
}

pub struct Writer {
    mode: FlagMode,
    header: String,
}

impl Writer {
    pub fn new(meta: &Metadata<'_>, compounds: Option<Compounds<'_>>) -> Self {
        let mut writer = Self {
            mode: meta.mode,
            header: String::from(SIGNATURE),
        };
        writer.header.push('\n');
        if let Some(name) = meta.mode.name() {
            writer.field("FLAG", name);
        }
        for comment in meta.comments {
            writer.field("#", comment.trim());
        }
        for (key, value) in [("TRY", meta.try_chars), ("KEY", meta.keyboard)] {
            if !value.is_empty() {
                writer.field(key, value);
            }
        }
        for letters in meta.map {
            writer.letters("MAP", letters);
        }
        for (from, to) in meta.replacements {
            let out = &mut writer.header;
            out.push_str("REP ");
            // Spaces inside replacement tokens are encoded as underscores.
            for (word, delimiter) in [(from, ' '), (to, '\n')] {
                out.extend(word.chars().map(|c| if c == ' ' { '_' } else { c }));
                out.push(delimiter);
            }
        }
        writer.switch("FULLSTRIP", meta.full_strip);
        writer.switch("CHECKSHARPS", meta.check_sharps);
        let flags = [
            "FORBIDDENWORD",
            "NOSUGGEST",
            "SUBSTANDARD",
            "KEEPCASE",
            "NEEDAFFIX",
            "CIRCUMFIX",
        ];
        for (name, flag) in flags.into_iter().zip(meta.word_flags) {
            writer.flag(name, flag);
        }
        for (name, pairs) in [("ICONV", meta.input), ("OCONV", meta.output)] {
            for (from, to) in pairs {
                let _ = writeln!(writer.header, "{name} {from} {to}");
            }
        }
        if !meta.ignore.is_empty() {
            writer.letters("IGNORE", meta.ignore);
        }
        if let Some(lang) = meta.language {
            writer.field("LANG", lang);
        }
        if let Some(min) = meta.compound_min {
            let _ = writeln!(writer.header, "COMPOUNDMIN {min}");
        }
        writer.flag("ONLYINCOMPOUND", meta.compound_only);
        for rule in meta.compound_rules {
            writer.field("COMPOUNDRULE", rule);
        }
        for (name, flag) in ["COMPOUNDPERMITFLAG", "COMPOUNDFORBIDFLAG"]
            .into_iter()
            .zip(meta.edge_flags)
        {
            writer.flag(name, flag);
        }
        if let Some(c) = compounds {
            writer.compounds(c);
        }
        writer
    }

    fn field(&mut self, name: &str, value: &str) {
        let _ = writeln!(self.header, "{name} {value}");
    }
    fn letters(&mut self, name: &str, chars: &[char]) {
        self.header.push_str(name);
        self.header.push(' ');
        self.header.extend(chars);
        self.header.push('\n');
    }
    fn flag(&mut self, key: &str, value: Option<u64>) {
        if let Some(flag) = value {
            self.header.push_str(key);
            self.header.push(' ');
            self.mode.write(std::iter::once(flag), &mut self.header);
            self.header.push('\n');
        }
    }
    fn switch(&mut self, key: &str, present: bool) {
        if present {
            self.header.push_str(key);
            self.header.push('\n');
        }
    }
    fn compounds(&mut self, c: Compounds<'_>) {
        if c.hungarian {
            self.field("LANG", "hu");
        }
        for (key, flag) in COMPOUND_FLAGS.into_iter().zip(c.flags) {
            self.flag(key, flag);
        }
        for (key, enabled) in COMPOUND_SWITCHES.into_iter().zip(c.switches) {
            self.switch(key, enabled);
        }
        if let Some(n) = c.word_max {
            let _ = writeln!(self.header, "COMPOUNDWORDMAX {n}");
        }
        if let Some((n, vowels)) = c.syllable {
            let _ = writeln!(self.header, "COMPOUNDSYLLABLE {n} {vowels}");
        }
        if let Some(flags) = c.syllable_num {
            self.field("SYLLABLENUM", flags);
        }
        if c.no_suggestions {
            self.field("MAXCPDSUGS", "0");
        }
        self.switch("NOSPLITSUGS", c.no_split_suggestions);
        for (end, start) in c.patterns {
            let _ = writeln!(self.header, "CHECKCOMPOUNDPATTERN {end} {start}");
        }
    }

    pub fn affix(&mut self, rule: &super::compact::Affix<'_>) {
        let out = &mut self.header;
        out.push_str(if rule.suffix { "SFX " } else { "PFX " });
        out.push_str(rule.flag);
        out.push_str(if rule.cross { " Y " } else { " N " });
        out.push_str(if rule.strip.is_empty() {
            "0"
        } else {
            rule.strip
        });
        out.push(' ');
        out.push_str(if rule.add.is_empty() { "0" } else { rule.add });
        if !rule.continuation.is_empty() {
            out.push('/');
            out.push_str(rule.continuation);
        }
        out.push(' ');
        out.push_str(rule.condition);
        out.push('\n');
    }

    pub fn into_root_buffer(mut self) -> Vec<u8> {
        self.header.push_str("WORDS\n");
        self.header.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::compact::{Affix, Directive, Header};

    #[test]
    fn compact_metadata_keeps_wire_order_and_round_trips_empty_affix_fields() {
        let metadata = Metadata {
            mode: FlagMode::Num,
            comments: &[" retained ".into()],
            try_chars: "",
            keyboard: "",
            map: &[],
            replacements: &[("two words".into(), "one".into())],
            full_strip: false,
            check_sharps: true,
            word_flags: [Some(7), None, None, None, None, None],
            input: &[],
            output: &[],
            ignore: &[],
            language: Some("tr"),
            compound_min: Some(4),
            compound_only: Some(12),
            compound_rules: &["(5)(12)".into()],
            edge_flags: [None, Some(9)],
        };
        let mut writer = Writer::new(&metadata, None);
        writer.affix(&Affix {
            suffix: true,
            flag: "2",
            cross: true,
            strip: "",
            add: "",
            continuation: "4,12",
            condition: ".",
        });
        let data = writer.into_root_buffer();
        assert_eq!(data, b"WCAFF2\nFLAG num\n# retained\nREP two_words one\nCHECKSHARPS\nFORBIDDENWORD 7\nLANG tr\nCOMPOUNDMIN 4\nONLYINCOMPOUND 12\nCOMPOUNDRULE (5)(12)\nCOMPOUNDFORBIDFLAG 9\nSFX 2 Y 0 0/4,12 .\nWORDS\n");
        let mut rules = 0;
        Header::read(&data)
            .unwrap()
            .visit(|_, record| {
                if let Directive::Affix(a) = record {
                    assert!(a.suffix && a.cross && a.strip.is_empty() && a.add.is_empty());
                    assert_eq!(a.continuation, "4,12");
                    rules += 1;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(rules, 1);
    }
}

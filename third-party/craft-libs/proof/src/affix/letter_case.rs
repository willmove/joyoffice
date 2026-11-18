//! Hunspell LANG selects dotted and dotless I for Turkish, Azeri and Crimean Tatar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum LetterCase {
    #[default]
    Common,
    Turkish,
    Azeri,
    CrimeanTatar,
}
impl LetterCase {
    pub(super) fn of(tag: &str) -> Self {
        match tag {
            "tr" | "tr_TR" => Self::Turkish,
            "az" | "az_AZ" => Self::Azeri,
            "crh" => Self::CrimeanTatar,
            _ => Self::Common,
        }
    }
    pub(super) fn tag(self) -> Option<&'static str> {
        match self {
            Self::Common => None,
            Self::Turkish => Some("tr"),
            Self::Azeri => Some("az"),
            Self::CrimeanTatar => Some("crh"),
        }
    }
    pub(super) fn lower(self, word: &str) -> String {
        if self == Self::Common || !word.contains(['I', 'İ']) {
            return word.to_lowercase();
        }
        word.replace('I', "ı").replace('İ', "i").to_lowercase()
    }
    /// An all-cap spelling with initial dotted I keeps that initial for common-language
    /// proper names. Turkic dictionaries try its lowercase form directly. In all-cap
    /// common-language words an interior dotted I maps to one dictionary letter.
    pub(super) fn capital_variants(self, word: &str) -> (Option<String>, Option<String>) {
        let simple = if self == Self::Common && word.contains('İ') {
            word.replace('İ', "i").to_lowercase()
        } else {
            self.lower(word)
        };
        if let Some(tail) = word.strip_prefix('İ') {
            if self == Self::Common {
                return (
                    Some(format!("İ{}", tail.replace('İ', "i").to_lowercase())),
                    None,
                );
            }
            return (None, Some(simple));
        }
        (Some(self.initial(&simple, true)), Some(simple))
    }

    pub(super) fn upper(self, word: &str) -> String {
        if self == Self::Common || !word.contains(['i', 'ı']) {
            return word.to_uppercase();
        }
        word.replace('i', "İ").replace('ı', "I").to_uppercase()
    }
    pub(super) fn initial(self, word: &str, upper: bool) -> String {
        let mut chars = word.chars();
        let Some(c) = chars.next() else {
            return String::new();
        };
        if self == Self::Common {
            return if upper {
                c.to_uppercase().chain(chars).collect()
            } else {
                c.to_lowercase().chain(chars).collect()
            };
        }
        let rest = chars.as_str();
        let mut result = if upper {
            self.upper(&word[..c.len_utf8()])
        } else {
            self.lower(&word[..c.len_utf8()])
        };
        result.push_str(rest);
        result
    }
}

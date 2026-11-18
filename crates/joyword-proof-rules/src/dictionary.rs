//! The Hunspell flag, affix-condition and compound-pattern syntax used by Word.
//! The adapter retains the dictionary text for compact-format interchange.
//! See the Hunspell 5 manual, under FLAG, PFX/SFX and COMPOUNDRULE.
mod compound;
mod condition;
mod flags;

pub use compound::{MAX_COMPOUND_TOKENS, Repeat, compound_pattern};
pub use condition::{Condition, condition};
pub use flags::{FlagMode, MAX_FLAGS_PER_ENTRY};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_and_dictionary_flags_keep_their_limits_and_round_trip() {
        for (mode, encoded, canonical) in [
            (FlagMode::Char, "☎️é", "☎é"),
            (FlagMode::Long, "AaéB", "AaéB"),
            (FlagMode::Num, "0,000017tail,65535", "0,17,65535"),
        ] {
            let flags = mode.parse(encoded).unwrap();
            let mut text = String::new();
            mode.write(flags.iter().copied(), &mut text);
            assert_eq!(text, canonical);
            assert_eq!(mode.parse(&text).unwrap(), flags);
            assert!(mode.parse_at_most(encoded, flags.len() - 1, true).is_err());
        }
        for v in ["65536", "x7", "-1", "99999999"] {
            assert!(FlagMode::Num.parse_one(v).is_err());
            assert_eq!(FlagMode::Num.parse_at_most(v, 1, true).unwrap(), [0]);
        }
        assert!(FlagMode::Long.parse_one("abc").is_err());
        assert_eq!(
            FlagMode::Long.parse_at_most("abc", 1, true).unwrap(),
            FlagMode::Long.parse("ab").unwrap()
        );
    }

    #[test]
    fn boundary_conditions_cover_ascii_and_multibyte_character_classes() {
        let cases = [
            ("[aa^]", "a^", "é中"),
            ("[^aa]", "é中", "a"),
            ("[é中é]", "中é", "a"),
            ("[^é中]", "a", "é中"),
            ("[]", "", "a中"),
            ("[^]", "a中", ""),
        ];
        for (raw, yes, no) in cases {
            let result = condition(raw).unwrap();
            assert_eq!(result.len(), 1);
            for c in yes.chars() {
                assert!(result[0].matches(c));
            }
            for c in no.chars() {
                assert!(!result[0].matches(c));
            }
        }
        assert!(condition(".").unwrap().is_empty());
        assert_eq!(condition(".a").unwrap().len(), 2);
        for malformed in ["a[", "a]", "[é", "[ab]]"] {
            assert!(condition(malformed).is_err());
        }
    }

    #[test]
    fn quantifiers_apply_to_one_flag_in_each_supported_encoding() {
        for (mode, text) in [
            (FlagMode::Char, "a?b*c"),
            (FlagMode::Long, "(aa)?(bb)*(cc)"),
            (FlagMode::Num, "(1)?(2)*(3)"),
        ] {
            let tokens = compound_pattern(text, mode).unwrap();
            assert_eq!(
                tokens.iter().map(|v| v.1).collect::<Vec<_>>(),
                [Repeat::Optional, Repeat::Any, Repeat::Once]
            );
        }
        assert_eq!(
            compound_pattern("(42)[x0,x1]*(3)", FlagMode::Num).unwrap(),
            [(42, Repeat::Once), (3, Repeat::Once)]
        );
        assert!(compound_pattern(&"é".repeat(MAX_COMPOUND_TOKENS + 1), FlagMode::Char).is_err());
        assert!(compound_pattern("a?*", FlagMode::Char).is_err());
        assert_eq!(
            compound_pattern(")k", FlagMode::Char).unwrap()[0].0,
            u64::from(')')
        );
    }
}

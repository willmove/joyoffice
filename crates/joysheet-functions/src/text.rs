//! First-party text function registry (`classic`, `format`, `modern`).

mod classic;
mod format;
mod modern;

use crate::util::{A, S};
use crate::{FnSpec, VAR};
use classic::{
    char_fn, clean, code, concat, concatenate, exact, find, identity, left, len, lower, mid, proper, replace, rept, right, search, substitute, t_fn,
    textjoin, trim, unichar, unicode, upper,
};
use format::{arraytotext, bahttext, dollar, fixed, numbervalue, value, valuetotext};
#[cfg(test)]
use modern::{expand_capped, template};
use modern::{regexextract, regexreplace, regextest, textafter, textbefore, textsplit};

const TJ: &[bool] = &[false, true, false];
const TB: &[bool] = &[true, false, true, true, true, false];
const TS: &[bool] = &[true, false, false, true, true, false];
const ATT: &[bool] = &[false, true];

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!("CONCATENATE", 1, VAR, Text, S, "CONCATENATE(text1, [text2], ...)", "Joins several text values into one.", concatenate),
        f!("CONCAT", 1, VAR, Text, A, "CONCAT(text1, [text2], ...)", "Joins text from values and every cell of ranges, without a separator.", concat),
        f!(
            "TEXTJOIN",
            3,
            VAR,
            Text,
            TJ,
            "TEXTJOIN(delimiter, ignore_empty, text1, [text2], ...)",
            "Joins text from values and ranges with a delimiter, optionally skipping empty items.",
            textjoin
        ),
        f!("LEFT", 1, 2, Text, S, "LEFT(text, [num_chars])", "Returns the first characters of a text.", left),
        f!("LEFTB", 1, 2, Text, S, "LEFTB(text, [num_bytes])", "Byte version of LEFT (same as LEFT for single-byte text).", left),
        f!("RIGHT", 1, 2, Text, S, "RIGHT(text, [num_chars])", "Returns the last characters of a text.", right),
        f!("RIGHTB", 1, 2, Text, S, "RIGHTB(text, [num_bytes])", "Byte version of RIGHT (same as RIGHT for single-byte text).", right),
        f!("MID", 3, 3, Text, S, "MID(text, start_num, num_chars)", "Returns characters from the middle of a text.", mid),
        f!("MIDB", 3, 3, Text, S, "MIDB(text, start_num, num_bytes)", "Byte version of MID (same as MID for single-byte text).", mid),
        f!("LEN", 1, 1, Text, S, "LEN(text)", "Counts the characters in a text.", len),
        f!("LENB", 1, 1, Text, S, "LENB(text)", "Byte version of LEN (same as LEN for single-byte text).", len),
        f!("LOWER", 1, 1, Text, S, "LOWER(text)", "Converts text to lower case.", lower),
        f!("UPPER", 1, 1, Text, S, "UPPER(text)", "Converts text to upper case.", upper),
        f!("PROPER", 1, 1, Text, S, "PROPER(text)", "Capitalises the first letter of each word and lower-cases the rest.", proper),
        f!("TRIM", 1, 1, Text, S, "TRIM(text)", "Removes leading and trailing spaces and collapses runs of spaces to one.", trim),
        f!("CLEAN", 1, 1, Text, S, "CLEAN(text)", "Removes non-printable control characters from text.", clean),
        f!(
            "SUBSTITUTE",
            3,
            4,
            Text,
            S,
            "SUBSTITUTE(text, old_text, new_text, [instance_num])",
            "Replaces occurrences of one text with another, optionally only a given occurrence.",
            substitute
        ),
        f!("REPLACE", 4, 4, Text, S, "REPLACE(old_text, start_num, num_chars, new_text)", "Replaces part of a text by position.", replace),
        f!("REPLACEB", 4, 4, Text, S, "REPLACEB(old_text, start_num, num_bytes, new_text)", "Byte version of REPLACE.", replace),
        f!("REPT", 2, 2, Text, S, "REPT(text, number_times)", "Repeats a text a given number of times.", rept),
        f!("FIND", 2, 3, Text, S, "FIND(find_text, within_text, [start_num])", "Finds the position of one text in another (case-sensitive).", find),
        f!("FINDB", 2, 3, Text, S, "FINDB(find_text, within_text, [start_num])", "Byte version of FIND.", find),
        f!(
            "SEARCH",
            2,
            3,
            Text,
            S,
            "SEARCH(find_text, within_text, [start_num])",
            "Finds the position of one text in another, ignoring case and allowing wildcards.",
            search
        ),
        f!("SEARCHB", 2, 3, Text, S, "SEARCHB(find_text, within_text, [start_num])", "Byte version of SEARCH.", search),
        f!("EXACT", 2, 2, Text, S, "EXACT(text1, text2)", "TRUE when two texts are identical, including case.", exact),
        f!("CHAR", 1, 1, Text, S, "CHAR(number)", "Returns the character for a code from the Windows-1252 character set.", char_fn),
        f!("CODE", 1, 1, Text, S, "CODE(text)", "Returns the Windows-1252 code of the first character of a text.", code),
        f!("UNICHAR", 1, 1, Text, S, "UNICHAR(number)", "Returns the character for a Unicode code point.", unichar),
        f!("UNICODE", 1, 1, Text, S, "UNICODE(text)", "Returns the Unicode code point of the first character of a text.", unicode),
        f!("VALUE", 1, 1, Text, S, "VALUE(text)", "Converts text that looks like a number, date or time into a number.", value),
        f!(
            "NUMBERVALUE",
            1,
            3,
            Text,
            S,
            "NUMBERVALUE(text, [decimal_separator], [group_separator])",
            "Converts text to a number using the given decimal and group separators.",
            numbervalue
        ),
        f!(
            "FIXED",
            1,
            3,
            Text,
            S,
            "FIXED(number, [decimals], [no_commas])",
            "Rounds a number and formats it as text with fixed decimals and optional thousands separators.",
            fixed
        ),
        f!("DOLLAR", 1, 2, Text, S, "DOLLAR(number, [decimals])", "Formats a number as currency text.", dollar),
        f!("USDOLLAR", 1, 2, Compatibility, S, "USDOLLAR(number, [decimals])", "Formats a number as currency text, like DOLLAR.", dollar),
        f!("BAHTTEXT", 1, 1, Text, S, "BAHTTEXT(number)", "Spells out a number in Thai words with Baht.", bahttext),
        f!("T", 1, 1, Text, S, "T(value)", "Returns the value if it is text, otherwise empty text.", t_fn),
        f!(
            "TEXTBEFORE",
            2,
            6,
            Text,
            TB,
            "TEXTBEFORE(text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found])",
            "Returns the text that comes before a delimiter.",
            textbefore
        ),
        f!(
            "TEXTAFTER",
            2,
            6,
            Text,
            TB,
            "TEXTAFTER(text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found])",
            "Returns the text that comes after a delimiter.",
            textafter
        ),
        f!(
            "TEXTSPLIT",
            2,
            6,
            Text,
            TS,
            "TEXTSPLIT(text, col_delimiter, [row_delimiter], [ignore_empty], [match_mode], [pad_with])",
            "Splits text into a grid of columns and rows at the given delimiters.",
            textsplit
        ),
        f!(
            "VALUETOTEXT",
            1,
            2,
            Text,
            S,
            "VALUETOTEXT(value, [format])",
            "Converts any value to text, optionally in strict (quoted) form.",
            valuetotext
        ),
        f!(
            "ARRAYTOTEXT",
            1,
            2,
            Text,
            ATT,
            "ARRAYTOTEXT(array, [format])",
            "Converts an array to text, either as a comma list or as a strict array constant.",
            arraytotext
        ),
        f!(
            "REGEXTEST",
            2,
            3,
            Text,
            S,
            "REGEXTEST(text, pattern, [case_sensitivity])",
            "TRUE when a regular expression matches somewhere in the text.",
            regextest
        ),
        f!(
            "REGEXEXTRACT",
            2,
            4,
            Text,
            S,
            "REGEXEXTRACT(text, pattern, [return_mode], [case_sensitivity])",
            "Extracts the first match, all matches or the capture groups of a regular expression.",
            regexextract
        ),
        f!(
            "REGEXREPLACE",
            3,
            5,
            Text,
            S,
            "REGEXREPLACE(text, pattern, replacement, [occurrence], [case_sensitivity])",
            "Replaces matches of a regular expression, all of them or a chosen occurrence.",
            regexreplace
        ),
        f!("ASC", 1, 1, Text, S, "ASC(text)", "Converts full-width characters to half-width (no change for single-byte text).", identity),
        f!("DBCS", 1, 1, Text, S, "DBCS(text)", "Converts half-width characters to full-width (no change in this locale).", identity),
        f!("JIS", 1, 1, Text, S, "JIS(text)", "Converts half-width characters to full-width (no change in this locale).", identity),
        f!("PHONETIC", 1, 1, Text, S, "PHONETIC(reference)", "Returns the phonetic (furigana) text; here the text itself.", identity),
    ]
}

#[cfg(test)]
#[path = "tests/text_tests.rs"]
mod tests;

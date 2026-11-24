//! Spelling suggestions from an affix dictionary, one engine for every language that reads one
//! (Czech, Croatian, Indonesian, Italian, Polish, Portuguese, Russian, Spanish). Candidates come
//! in rounds of rising cost, each checked against the dictionary:
//! 1. letter case (`warszawa` → `Warszawa`);
//! 2. missing or wrong diacritics and other MAP slips, a few letters changed (`zolw` → `żółw`):
//!    adding a missing accent is the commonest slip and ranks first, while taking away one the
//!    writer typed ranks after the known confusions;
//! 3. the REP table of common confusions (`żeka` → `rzeka`);
//! 4. a missing space (`napewno` → `na pewno`);
//! 5. keyboard neighbours from the KEY rows, then one typo: a letter left out, added, swapped with
//!    the next or mistyped (TRY letters);
//! 6. when that finds fewer than asked for: the stems that start like the word (MAP relatives
//!    folded) and share the most letter pairs with it, whose forms are ranked by an edit distance in
//!    which a MAP slip costs half (two typos at most).
//!
//! The tables come from the dictionary; a language's [`Tuning`] adds the ones its dictionaries may
//! lack and the slips no table can say. The input is bounded ([`MAX_LETTERS`]), and so is every
//! round, so a call costs a few thousand dictionary lookups at most.

mod edits;
mod space;
use edits::map_variants;
pub(crate) use space::Split;
mod ranking;
pub(crate) use ranking::weighted_distance;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::affix::{AffixDict, Casing, casing};

/// Longest word suggestions are looked for, in letters.
pub(crate) const MAX_LETTERS: usize = 40;
const MAX_MAP_VARIANTS: usize = 4096;
/// Most candidates kept waiting for the compound check in one call (the rest are dropped).
const MAX_DEFERRED: usize = 1 << 14;
/// Stems the scan of round 6 expands.
const SCAN_STEMS: usize = 48;
/// Most affix rules tried in all to make those stems' forms (Mongolian has 12,000 rules).
const SCAN_FORMS_WORK: usize = 1 << 16;

const COST_CASE: u32 = 0;
pub(crate) const COST_ADD_DIACRITIC: u32 = 4;
pub(crate) const COST_OTHER_MAP: u32 = 7;
const COST_DROP_DIACRITIC: u32 = 11;
const COST_REP: u32 = 10;
pub(crate) const COST_SPLIT: u32 = 12;
/// A missing space leaving a half shorter than three letters ([`Split::Anywhere`]).
const COST_SPLIT_SHORT: u32 = 17;
/// A missing space between two longer halves with no short word beside it ([`Split::Near`]).
pub(crate) const COST_SPLIT_LONG: u32 = 18;
const COST_KEY: u32 = 13;
const COST_SWAP: u32 = 14;
const COST_DROP_OR_ADD: u32 = 15;
pub(crate) const COST_MISTYPE: u32 = 16;
const COST_SCAN: u32 = 20;
/// A missing space ranked after every one-word candidate (English: [`Split::LongWords`]).
pub(crate) const COST_SPLIT_LAST: u32 = COST_SCAN + 4 * 3 + 1;

/// What a language adds to its dictionary's tables.
pub(crate) struct Tuning {
    /// Where a missing space may go (round 4).
    pub split: Split,
    /// TRY letters for a dictionary without a TRY line.
    pub alphabet: &'static str,
    /// MAP groups the language adds to its dictionary's (each string is one group: `"cčć"`).
    pub map: &'static [&'static str],
    /// REP confusions the language adds to its dictionary's.
    pub rep: &'static [(&'static str, &'static str)],
    /// The cost of a REP confusion.
    pub rep_cost: u32,
    /// Most letters a MAP variant changes.
    pub max_map_changes: usize,
    /// The cost of changing letter `from` to its MAP relative `to`.
    pub map_cost: fn(char, char) -> u32,
    /// The cost of typing letter `c` at `i` of the word (`chars`, lower case) instead.
    pub mistype_cost: fn(&[char], usize, char) -> u32,
    /// Offer a word only the dictionary capitalizes (a name, an abbreviation: `Col`, `Wis`) only
    /// when the word itself is capitalized, and never as half of a missing space (`Che Que`).
    pub strict_case: bool,
}

impl Tuning {
    /// The dictionary's tables alone (Czech, Indonesian, Spanish).
    pub(crate) const DEFAULT: Tuning = Tuning {
        split: Split::Anywhere,
        alphabet: "",
        map: &[],
        rep: &[],
        rep_cost: COST_REP,
        max_map_changes: 4,
        map_cost,
        mistype_cost,
        strict_case: false,
    };
}

/// The cost of changing letter `from` to its MAP relative `to`. Adding a missing accent is the
/// commonest slip (typing without diacritics); an accent the writer did type is likelier right,
/// so taking one away costs more than a known confusion from the REP table.
pub(crate) fn map_cost(from: char, to: char) -> u32 {
    match (from.is_ascii(), to.is_ascii()) {
        (true, false) => COST_ADD_DIACRITIC,
        (false, true) => COST_DROP_DIACRITIC,
        _ => COST_OTHER_MAP,
    }
}

/// Any mistyped letter costs the same.
fn mistype_cost(_: &[char], _: usize, _: char) -> u32 {
    COST_MISTYPE
}

struct Collector<'a> {
    d: &'a AffixDict,
    /// The word, lower case.
    input: &'a str,
    /// Candidate → the cheapest cost it was offered at (rounds don't run in rising cost).
    tested: HashMap<String, u32>,
    /// Suggestion → (cost, order found).
    found: HashMap<String, (u32, usize)>,
    seq: usize,
    /// May a word the dictionary only has capitalized be offered ([`Tuning::strict_case`])?
    names: bool,
    /// May half of a missing space be one ([`Tuning::strict_case`])?
    split_names: bool,
    /// Candidates no simple word, left for the compound check: (cost, order, candidate).
    deferred: Vec<(u32, usize, String)>,
    /// Steps left for the compound searches of this call (`MAX_SUGGEST_COMPOUND_WORK` at first).
    compound_work: usize,
    /// The pieces those searches analysed, shared between them.
    pieces: crate::affix::Pieces,
}

impl Collector<'_> {
    /// Check a lower-case candidate (a word, or two separated by a space) and keep it if every
    /// word is in the dictionary, as written or capitalized (names).
    fn offer(&mut self, cand: &str, cost: u32) {
        if cand.is_empty() || cand == self.input {
            return;
        }
        match self.tested.get_mut(cand) {
            Some(prev) if *prev <= cost => return,
            Some(prev) => *prev = cost,
            None => {
                self.tested.insert(cand.to_string(), cost);
            }
        }
        // A candidate is checked as simple words here; one of a single word that isn't one waits
        // for the compound check, which the cheapest candidates get first ([`Self::compounds`]).
        if !self.accept(cand, cost, false)
            && self.d.flag_compounds()
            && !cand.contains(' ')
            && self.deferred.len() < MAX_DEFERRED
        {
            self.seq += 1;
            self.deferred.push((cost, self.seq, cand.to_string()));
        }
    }

    /// Check the waiting candidates as compounds, cheapest (then earliest) first, in one budget
    /// of steps per call: compound checks cost far more than lookups, and a one-letter slip of a
    /// long compound is the likeliest intent.
    ///
    /// A candidate changed where the word stops being parts (`ziekenhusopname`: `zieken` and
    /// `opname` are, `hus` isn't) goes first.
    fn compounds(&mut self) {
        let mut waiting = std::mem::take(&mut self.deferred);
        if waiting.is_empty() {
            return;
        }
        let gap = self.d.compound_gap(
            self.input,
            self.names,
            &mut self.compound_work,
            &mut self.pieces,
        );
        let input: Vec<char> = self.input.chars().collect();
        let outside = |cand: &str| {
            let Some((from, to)) = gap else { return false };
            let at = cand
                .chars()
                .zip(&input)
                .position(|(a, b)| a != *b)
                .unwrap_or(input.len().min(cand.chars().count()));
            at + 1 < from || at > to + 1
        };
        let mut waiting: Vec<(bool, u32, usize, String)> = waiting
            .drain(..)
            .map(|(cost, seq, cand)| (outside(&cand), cost, seq, cand))
            .collect();
        waiting.sort_unstable();
        for (_, cost, _, cand) in waiting {
            if self.compound_work == 0 {
                break;
            }
            // Offered again at a lower cost meanwhile: that offer is the one to check.
            if self.tested.get(&cand).is_some_and(|&c| c < cost) || self.found.contains_key(&cand) {
                continue;
            }
            self.accept(&cand, cost, true);
        }
    }

    /// Keep `cand` if each of its words is in the dictionary (`compounds`: flag-based compounds
    /// count); whether it was.
    fn accept(&mut self, cand: &str, cost: u32, compounds: bool) -> bool {
        let names = if cand.contains(' ') {
            self.split_names
        } else {
            self.names
        };
        let d = self.d;
        let (work, pieces) = (&mut self.compound_work, &mut self.pieces);
        let mut valid = |piece: &str| {
            if compounds {
                d.flag_compound_spending_suggestible(piece, work, pieces)
            } else {
                d.contains_suggestible_with(piece, false)
            }
        };
        let mut rewritten = String::new();
        let (mut byte, mut copied) = (0, 0);
        let mut cost = cost;
        for part in cand.split(' ') {
            let end = byte + part.len();
            if !valid(part) {
                if !names {
                    return false;
                }
                let name = d.initial_upper(part);
                if !valid(&name) {
                    return false;
                }
                // Preserve original separators, including those in a dictionary REP
                // phrase. Only allocate a replacement when a name changes case.
                rewritten.push_str(&cand[copied..byte]);
                rewritten.push_str(&name);
                copied = end;
                cost += 1;
            }
            byte = end + 1;
        }
        if copied == 0 {
            self.keep(cand.to_owned(), cost);
        } else {
            rewritten.push_str(&cand[copied..]);
            self.keep(rewritten, cost);
        }
        true
    }

    /// Keep a suggestion known to be correct, unless the dictionary says never to offer it.
    fn keep(&mut self, text: String, cost: u32) {
        if !text.split(' ').all(|part| self.d.can_suggest(part)) {
            return;
        }
        self.seq += 1;
        let seq = self.seq;
        let e = self.found.entry(text).or_insert((cost, seq));
        if cost < e.0 {
            *e = (cost, seq);
        }
    }

    fn count_within(&self, cost: u32) -> usize {
        self.found.values().filter(|(c, _)| *c <= cost).count()
    }
}

/// The MAP groups: the dictionary's, then the language's.
fn groups<'a>(d: &'a AffixDict, t: &Tuning) -> Cow<'a, [Vec<char>]> {
    if t.map.is_empty() {
        return Cow::Borrowed(d.map());
    }
    Cow::Owned(
        d.map()
            .iter()
            .cloned()
            .chain(t.map.iter().map(|g| g.chars().collect()))
            .collect(),
    )
}

/// The other members of every MAP group of `c` (its likely confusions), each once.
fn alternatives(c: char, map: &[Vec<char>]) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    for x in map
        .iter()
        .filter(|g| g.contains(&c))
        .flat_map(|g| g.iter().copied())
    {
        if x != c && !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// A letter's MAP representative (the first of its first group), lower case: `ż` → `z`, `ó` → `o`.
pub(crate) fn fold(c: char, map: &[Vec<char>]) -> char {
    let l = c.to_lowercase().next().unwrap_or(c);
    map.iter()
        .find(|g| g.contains(&l))
        .and_then(|g| g.first().copied())
        .unwrap_or(l)
}

/// Spelling suggestions for `word` from `d`, best first, at most `max`, in the word's letter case.
pub(crate) fn suggest_in(d: &AffixDict, word: &str, max: usize, t: &Tuning) -> Vec<String> {
    let word = word.trim();
    let n = word.chars().count();
    if n == 0 || n > MAX_LETTERS || max == 0 || d.is_empty() || word.contains(char::is_whitespace) {
        return Vec::new();
    }
    // The simple-word checks of this call share one budget of affix rules.
    let _steps = crate::affix::SuggestSteps::start();
    let case = casing(word);
    let input = d.lower_word(word);
    let chars: Vec<char> = input.chars().collect();
    let names = !t.strict_case || case != Casing::Lower;
    let split_names = !t.strict_case;
    let mut col = Collector {
        d,
        input: &input,
        tested: HashMap::new(),
        found: HashMap::new(),
        seq: 0,
        names,
        split_names,
        deferred: Vec::new(),
        compound_work: crate::affix::MAX_SUGGEST_COMPOUND_WORK,
        pieces: Default::default(),
    };
    add_case(&mut col, word, &input);
    let map = groups(d, t);
    add_map_slips(&mut col, &chars, &map, t);
    let rep = d
        .rep()
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .chain(t.rep.iter().copied());
    let alphabet = if d.try_chars().is_empty() {
        t.alphabet
    } else {
        d.try_chars()
    };
    edits::visit(&input, &chars, rep, d.key(), alphabet, t, |word, cost| {
        col.offer(word, cost)
    });
    col.compounds();
    // 6. Further off: scan the stems that start like the word.
    if col.count_within(COST_MISTYPE) < max {
        scan(&mut col, &chars, &map);
        col.compounds();
    }
    let found = ranking::best_first(col.found, &chars, n);
    let mut out: Vec<String> = Vec::new();
    'found: for text in found {
        // A KEEPCASE word only exists in its dictionary case: `Km` would be flagged, so offer `km`.
        // Under CHECKSHARPS one with `ß` may still be capitalized, or written in capitals with SS
        // (`Fußball`, `FUSSBALL`), as hunspell 1.7.5 offers.
        let locked = text.split(' ').any(|p| d.case_locked(p));
        let sharp = d.check_sharps() && text.contains('ß');
        let recased: Vec<String> = match case {
            Casing::Initial if !locked || sharp => vec![d.initial_upper(&text)],
            // In capitals `ß` stays `ß` (`STRAßE` is right with or without CHECKSHARPS); under
            // CHECKSHARPS the SS spelling comes first, as Hunspell offers it.
            Casing::Upper if n > 1 => {
                let ss = d.upper_word(&text);
                let kept: String = text
                    .split('ß')
                    .map(|part| d.upper_word(part))
                    .collect::<Vec<_>>()
                    .join("ß");
                match (locked, sharp) {
                    (false, false) => vec![kept],
                    (false, true) => vec![ss, kept],
                    (true, true) => vec![ss, text.clone()],
                    (true, false) => vec![text.clone()],
                }
            }
            _ => vec![text.clone()],
        };
        for t in recased {
            // Shown as the dictionary's OCONV table writes it (`'` → `’` in French).
            let t = d.output(&t).into_owned();
            if t != word && !out.contains(&t) {
                out.push(t);
            }
            if out.len() >= max {
                break 'found;
            }
        }
    }
    out
}

/// Round 1: letter case.
fn add_case(col: &mut Collector, word: &str, input: &str) {
    if col.d.contains(input) && word != input {
        col.keep(input.to_string(), COST_CASE);
    } else if col.d.contains(&col.d.initial_upper(input)) && word != col.d.initial_upper(input) {
        col.keep(col.d.initial_upper(input), COST_CASE);
    }
}

/// Round 2: accents and other MAP slips.
fn add_map_slips(col: &mut Collector, chars: &[char], map: &[Vec<char>], t: &Tuning) {
    let alts: Vec<Vec<char>> = chars.iter().map(|&c| alternatives(c, map)).collect();
    let mut variants = Vec::new();
    map_variants(chars, &alts, t, &mut variants);
    variants.sort_by_key(|v| v.1);
    for (v, cost) in &variants {
        col.offer(v, *cost);
    }
}

/// Round 6: rank the stems starting with the word's first letter (or a MAP relative of it, either
/// case) by shared letter pairs, then keep the forms of the best within two typos.
fn scan(col: &mut Collector, chars: &[char], map: &[Vec<char>]) {
    let d = col.d;
    let Some(&first) = chars.first() else { return };
    let folded: Vec<char> = chars.iter().map(|&c| fold(c, map)).collect();
    let pairs: HashSet<(char, char)> = std::iter::once(' ')
        .chain(folded.iter().copied())
        .zip(folded.iter().copied())
        .collect();
    let mut firsts: Vec<char> = std::iter::once(first)
        .chain(alternatives(first, map))
        .collect();
    let upper: Vec<char> = firsts.iter().flat_map(|c| c.to_uppercase()).collect();
    firsts.extend(upper);
    firsts.sort_unstable();
    firsts.dedup();
    let n = folded.len();
    let stems = firsts
        .into_iter()
        .flat_map(|first| d.stems_with_prefix(&first.to_string()));
    let scored = stems.map(|stem| {
        let mut previous = ' ';
        let mut length = 0usize;
        let mut shared = 0usize;
        for next in stem.chars().map(|c| fold(c, map)) {
            shared += usize::from(pairs.contains(&(previous, next)));
            length += 1;
            previous = next;
        }
        // Shared pairs, less the stem's letters the word doesn't explain, less a length gap
        // beyond what an ending explains.
        let extra = length.saturating_sub(shared);
        let gap = length.abs_diff(n).saturating_sub(4);
        let score = (shared * 4).saturating_sub(extra * 3 + gap * 2);
        (stem, if shared >= 2 { score } else { 0 })
    });
    let best = ranking::stem_shortlist(scored, SCAN_STEMS);
    let related = |a: char, b: char| fold(a, map) == fold(b, map);
    let limit = 4;
    let mut budget = SCAN_FORMS_WORK;
    for stem in best {
        for form in d.forms_spending(stem, &mut budget) {
            let lf: Vec<char> = col.d.lower_word(&form).chars().collect();
            if lf.len().abs_diff(chars.len()) > 2 {
                continue;
            }
            if !col.names && form.chars().any(char::is_uppercase) {
                continue;
            }
            let dist = weighted_distance(&lf, chars, &related, limit);
            if dist <= limit && lf.as_slice() != chars {
                col.keep(form, COST_SCAN + dist * 3);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// craft-libs#487 review: MAP variants are made by number of changes, so when a long word has
    /// more than [`MAX_MAP_VARIANTS`] of them the cap drops the most-changed ones, never a single
    /// slip (here the first letter's, which depth-first order reached last).
    #[test]
    fn the_map_cap_keeps_the_fewest_changes() {
        let chars: Vec<char> = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".chars().collect();
        let alts = vec!["áàâäãåā".chars().collect::<Vec<char>>(); chars.len()];
        let mut out = Vec::new();
        map_variants(&chars, &alts, &Tuning::DEFAULT, &mut out);
        assert_eq!(out.len(), MAX_MAP_VARIANTS);
        let changed = |v: &str| v.chars().filter(|&c| c != 'a').count();
        assert_eq!(
            out.iter().filter(|(v, _)| changed(v) == 1).count(),
            30 * 7,
            "every single slip is kept"
        );
        assert!(
            out.iter()
                .any(|(v, _)| v.starts_with('á') && changed(v) == 1)
        );
        assert!(
            out.windows(2).all(|w| changed(&w[0].0) <= changed(&w[1].0)),
            "fewest changes first"
        );
    }

    /// craft-libs#490 review: a suggestion recased to capitals keeps `ß` (`STRAßE`, right with or
    /// without CHECKSHARPS); under CHECKSHARPS the SS form comes first, and a KEEPCASE `ß` word may
    /// be capitalized or written in capitals with SS. Expected lists are the hunspell 1.7.5 CLI's
    /// on the same files; every suggestion is right.
    #[test]
    fn sharp_s_suggestions_match_hunspell() {
        let try_ = "TRY esianrtolcdugmphbyfvkwzß\n";
        let cases: &[(String, &str, &str, &[&str])] = &[
            (
                format!("SET UTF-8\n{try_}"),
                "1\nstraße\n",
                "STRAßEE",
                &["STRAßE"],
            ),
            (
                format!("SET UTF-8\nCHECKSHARPS\n{try_}"),
                "1\nstraße\n",
                "STRAßEE",
                &["STRASSE", "STRAßE"],
            ),
            (
                format!("SET UTF-8\nCHECKSHARPS\n{try_}"),
                "1\nstraße\n",
                "Straßee",
                &["Straße"],
            ),
            (
                format!("SET UTF-8\nCHECKSHARPS\nKEEPCASE k\n{try_}"),
                "2\nfußball/k\nGruß/k\n",
                "FUSBALL",
                &["FUSSBALL", "fußball"],
            ),
            (
                format!("SET UTF-8\nCHECKSHARPS\nKEEPCASE k\n{try_}"),
                "2\nfußball/k\nGruß/k\n",
                "Fusball",
                &["Fußball"],
            ),
            (
                format!("SET UTF-8\nCHECKSHARPS\nKEEPCASE k\n{try_}"),
                "2\nfußball/k\nGruß/k\n",
                "GRUS",
                &["GRUSS", "Gruß"],
            ),
        ];
        for (aff, dic, word, want) in cases {
            let d = AffixDict::from_hunspell(aff, dic, &[]).unwrap();
            let got = suggest_in(&d, word, 8, &Tuning::DEFAULT);
            assert_eq!(&got, want, "{word} with\n{aff}");
            for g in &got {
                assert!(d.check(g), "{word} → {g} is not right");
            }
        }
    }

    /// craft-libs#489 review repros: OCONV applies to a suggestion after its case is set, case
    /// sensitively, as the hunspell 1.7.5 CLI answers on the same files (`KBT` → `KAT`, not
    /// `K’T`), and every suggestion is right.
    #[test]
    fn oconv_suggestions_match_hunspell() {
        type Case = (
            &'static str,
            &'static str,
            &'static [(&'static str, &'static str)],
        );
        let cases: &[Case] = &[
            (
                "SET UTF-8\nTRY ab\nOCONV 1\nOCONV a ’\n",
                "1\nkat\n",
                &[
                    ("kbt", "k’t"),
                    ("Kbt", "K’t"),
                    ("KBT", "KAT"),
                    ("kot", "k’t"),
                ],
            ),
            (
                "SET UTF-8\nTRY ab\nOCONV 1\nOCONV k q\n",
                "1\nkat\n",
                &[
                    ("kbt", "qat"),
                    ("Kbt", "Kat"),
                    ("KBT", "KAT"),
                    ("kot", "qat"),
                ],
            ),
            (
                "SET UTF-8\nTRY esianrtolcdugmphbyfvkwz'\nICONV 1\nICONV ’ '\nOCONV 1\nOCONV ' ’\nSFX S Y 1\nSFX S 0 's .\n",
                "2\ncolour/S\ndon't\n",
                &[
                    ("dont", "don’t"),
                    ("DONT", "DON’T"),
                    ("colours", "colour’s"),
                ],
            ),
        ];
        for (aff, dic, words) in cases {
            let d = AffixDict::from_hunspell(aff, dic, &[]).unwrap();
            for (word, want) in *words {
                let got = suggest_in(&d, word, 8, &Tuning::DEFAULT);
                assert!(
                    got.iter().any(|g| g == want),
                    "{word}: {got:?}, want {want}"
                );
                for g in &got {
                    assert!(
                        d.check(g) || !aff.contains("ICONV"),
                        "{word} → {g} is not right"
                    );
                }
            }
        }
    }

    /// craft-libs#489 review: 65,536 ICONV pairs on one first character cost one hash lookup per
    /// prefix, not a scan of the table, per position of a long word and per suggestion.
    #[test]
    fn a_huge_conversion_table_on_one_character_stays_fast() {
        let mut aff = String::from("SET UTF-8\nTRY abc\nICONV 65536\n");
        let a60 = "a".repeat(60);
        for c in (0x100u32..).filter_map(char::from_u32).take(65_536) {
            aff.push_str(&format!("ICONV {a60}{c} x\n"));
        }
        let d = AffixDict::from_hunspell(&aff, "1\nabc\n", &[]).unwrap();
        let t = std::time::Instant::now();
        assert!(!d.check(&"a".repeat(470)));
        let _ = suggest_in(&d, "abd", 4, &Tuning::DEFAULT);
        let _ = suggest_in(&d, &"a".repeat(40), 4, &Tuning::DEFAULT);
        assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
    }

    #[test]
    fn an_extra_trailing_letter_is_dropped_before_a_short_word_split() {
        let d = AffixDict::from_hunspell("", "2\npočítač\nz\n", &[]).unwrap();
        let suggestions = suggest_in(&d, "počítačz", 8, &Tuning::DEFAULT);
        assert_eq!(
            suggestions.first().map(String::as_str),
            Some("počítač"),
            "{suggestions:?}"
        );
        assert!(suggestions.contains(&"počítač z".into()), "{suggestions:?}");
    }

    /// A candidate offered again at a lower cost (a REP confusion after a MAP diacritic drop)
    /// ranks at the lower one.
    #[test]
    fn a_cheaper_later_offer_wins() {
        let d = AffixDict::from_hunspell("", "1\ncafe\n", &[]).unwrap();
        let mut col = Collector {
            d: &d,
            input: "cafè",
            tested: HashMap::new(),
            found: HashMap::new(),
            seq: 0,
            names: true,
            split_names: true,
            deferred: Vec::new(),
            compound_work: crate::affix::MAX_SUGGEST_COMPOUND_WORK,
            pieces: Default::default(),
        };
        col.offer("cafe", COST_DROP_DIACRITIC);
        col.offer("cafe", COST_REP);
        col.offer("cafe", COST_SCAN);
        assert_eq!(col.found.get("cafe").map(|e| e.0), Some(COST_REP));
    }

    #[test]
    fn distance_weighs_map_slips_half() {
        let map = vec![vec!['a', 'ą'], vec!['o', 'ó', 'u']];
        let related = |a: char, b: char| fold(a, &map) == fold(b, &map);
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(weighted_distance(&c("ktury"), &c("który"), &related, 9), 1);
        assert_eq!(weighted_distance(&c("kot"), &c("kto"), &related, 9), 2);
        assert_eq!(weighted_distance(&c("kot"), &c("kotek"), &related, 9), 4);
        assert_eq!(weighted_distance(&c("kot"), &c("pies"), &related, 2), 3);
        assert_eq!(weighted_distance(&c(""), &c("ab"), &related, 9), 4);
    }

    /// A KEEPCASE word is offered in its dictionary case when the word's own case would be flagged
    /// (`Kmm` → `km`, never `Km`), and a capitalised KEEPCASE word gets its dictionary form.
    #[test]
    fn keepcase_suggestions_are_not_recased_into_errors() {
        let d = AffixDict::from_hunspell("KEEPCASE K\n", "2\nkm/K\npies\n", &[]).unwrap();
        assert!(!d.check("Km") && !d.check("KM"));
        for (word, want) in [("Kmm", "km"), ("KMM", "km"), ("Km", "km")] {
            let got = suggest_in(&d, word, 4, &Tuning::DEFAULT);
            assert!(
                got.iter().all(|s| s.split(' ').all(|p| d.check(p))),
                "{word}: {got:?}"
            );
            assert_eq!(
                got.first().map(String::as_str),
                Some(want),
                "{word}: {got:?}"
            );
        }
        assert_eq!(
            suggest_in(&d, "Pies", 4, &Tuning::DEFAULT),
            Vec::<String>::new()
        );
        assert_eq!(
            suggest_in(&d, "Piesx", 4, &Tuning::DEFAULT)
                .first()
                .map(String::as_str),
            Some("Pies")
        );
    }

    /// A language's tables come after its dictionary's, a TRY alphabet stands in for a missing TRY
    /// line, and a split rule decides which missing spaces are offered.
    #[test]
    fn tuning_adds_to_the_dictionary_tables() {
        let d = AffixDict::from_hunspell("", "3\nčaj\nkot\nna\n", &[]).unwrap();
        let tuned = Tuning {
            map: &["cč"],
            alphabet: "t",
            rep: &[("xy", "kot")],
            ..Tuning::DEFAULT
        };
        assert!(!suggest_in(&d, "caj", 4, &Tuning::DEFAULT).contains(&"čaj".into()));
        assert!(
            suggest_in(&d, "caj", 4, &tuned).contains(&"čaj".into()),
            "the language's MAP group"
        );
        assert!(
            suggest_in(&d, "xy", 4, &tuned).contains(&"kot".into()),
            "the language's REP pair"
        );
        assert!(
            suggest_in(&d, "ko", 4, &tuned).contains(&"kot".into()),
            "the language's TRY letters"
        );
        let near = Tuning {
            split: Split::Near {
                leading: &["na"],
                trailing: &[],
            },
            ..Tuning::DEFAULT
        };
        assert!(suggest_in(&d, "nakot", 4, &near).contains(&"na kot".into()));
        assert!(
            !suggest_in(&d, "kotna", 4, &near).contains(&"kot na".into()),
            "no stray short half"
        );
        assert!(suggest_in(&d, "kotna", 4, &Tuning::DEFAULT).contains(&"kot na".into()));
    }
}

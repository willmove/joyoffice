//! Enumerate a stem's licensed forms in the forward direction. The selected
//! rule and its continuation stay together until the whole derivation is checked.
use super::{AffixDict, MAX_FORMS, MAX_FORMS_WORK, MAX_WORD_BYTES, Rule, has};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::slice;

impl AffixDict {
    /// Forms of this stem, subject to the per-stem enumeration budget.
    pub fn forms(&self, stem: &str) -> Vec<String> {
        let mut budget = MAX_FORMS_WORK;
        self.forms_spending(stem, &mut budget)
    }

    /// Several stems in one suggestion request share the caller's work budget.
    pub(crate) fn forms_spending(&self, stem: &str, budget: &mut usize) -> Vec<String> {
        let Some((index, merged)) = self.stem_lookup(stem) else {
            return Vec::new();
        };
        if self.excluded(false).hits(merged)
            || self.only_in_compound.is_some_and(|f| has(merged, f))
            || *budget == 0
        {
            return Vec::new();
        }
        let cap = (*budget).min(MAX_FORMS_WORK);
        let mut expansion = Expansion {
            dict: self,
            stem,
            root_flags: merged,
            root_index: index,
            remaining: cap,
            words: Vec::new(),
        };
        expansion.generate();
        *budget -= cap - expansion.remaining;
        expansion.words
    }
}

struct Expansion<'a> {
    dict: &'a AffixDict,
    stem: &'a str,
    root_flags: &'a [u16],
    root_index: u32,
    remaining: usize,
    words: Vec<String>,
}

/// Merge only the rules granted by these flags, in source order. The common
/// one-class case borrows the dictionary's already ordered list.
enum OrderedRules<'a> {
    Single(slice::Iter<'a, u32>),
    Merge {
        heads: BinaryHeap<Reverse<(u32, usize)>>,
        tails: Vec<slice::Iter<'a, u32>>,
    },
}

impl Iterator for OrderedRules<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        match self {
            Self::Single(rules) => rules.next().copied(),
            Self::Merge { heads, tails } => {
                let Reverse((rule, class)) = heads.pop()?;
                if let Some(next) = tails.get_mut(class).and_then(Iterator::next) {
                    heads.push(Reverse((*next, class)));
                }
                Some(rule)
            }
        }
    }
}

fn ordered<'a>(
    by_flag: &'a [Vec<u32>],
    flags: &[u16],
    budget: &mut usize,
) -> Option<OrderedRules<'a>> {
    *budget = budget.checked_sub(flags.len())?;
    let mut groups = flags
        .iter()
        .map(|&f| by_flag[usize::from(f)].as_slice())
        .filter(|s| !s.is_empty());
    let Some(first) = groups.next() else {
        return Some(OrderedRules::Single([].iter()));
    };
    let Some(second) = groups.next() else {
        return Some(OrderedRules::Single(first.iter()));
    };
    let mut heads = Vec::new();
    let mut tails = Vec::new();
    for group in [first, second].into_iter().chain(groups) {
        *budget = budget.checked_sub(group.len())?;
        let mut rules = group.iter();
        heads.push(Reverse((*rules.next()?, tails.len())));
        tails.push(rules);
    }
    Some(OrderedRules::Merge {
        heads: heads.into(),
        tails,
    })
}

impl Expansion<'_> {
    /// Count both rule applications and flag-list visits, even for rejected forms.
    fn step(&mut self) -> bool {
        if self.remaining == 0 || self.words.len() >= MAX_FORMS {
            return false;
        }
        self.remaining -= 1;
        true
    }

    fn emit(&mut self, word: &str) {
        if !word.is_empty()
            && word.len() <= MAX_WORD_BYTES
            && !self.dict.is_forbidden(word)
            && self.words.len() < MAX_FORMS
        {
            self.words.push(word.to_owned());
        }
    }

    fn generate(&mut self) {
        let d = self.dict;
        if !d.need_affix.is_some_and(|flag| has(self.root_flags, flag)) {
            self.emit(self.stem);
        }
        // Rule application and building ordered flag lists are independent costs. A
        // sparse continuation cannot repeatedly scan a large list for free.
        let mut index_budget = self.remaining.saturating_mul(4);
        let mut first = Vec::new();
        let mut prefix_flags =
            std::collections::BTreeSet::from_iter(self.root_flags.iter().copied());
        let mut continuation_sets = std::collections::HashSet::new();
        let Some(suffixes) = ordered(&d.sfx_by_flag, self.root_flags, &mut index_budget) else {
            return;
        };
        for index in suffixes {
            if !self.step() {
                return;
            }
            let rule = &d.suffixes[index as usize];
            let Some(word) = self.suffix(self.stem, rule) else {
                continue;
            };
            if !d.cont_holds(rule, d.circumfix) && !d.cont_holds(rule, d.need_affix) {
                self.emit(&word);
            }
            if rule.cross && continuation_sets.insert(rule.cont) {
                let flags = d.set(rule.cont);
                let Some(left) = index_budget.checked_sub(flags.len()) else {
                    return;
                };
                index_budget = left;
                prefix_flags.extend(flags);
            }
            first.push((word, rule));
        }
        // Preserve the existing priority for capped or very productive stems:
        // single suffixes, unprefixed pairs, then standalone and singly crossed
        // prefixes. Newly supported three-affix forms use the remaining budget.
        let mut visited = std::collections::HashSet::new();
        for (word, inner) in &first {
            if inner.cont == 0
                || d.cont_holds(inner, d.circumfix)
                || !visited.insert((word.as_str(), inner.cont))
            {
                continue;
            }
            let Some(suffixes) = ordered(&d.sfx_by_flag, d.set(inner.cont), &mut index_budget)
            else {
                return;
            };
            for id in suffixes {
                if !self.step() {
                    return;
                }
                if let Some(next) = self.suffix(word, &d.suffixes[id as usize]) {
                    self.emit(&next);
                }
            }
        }
        let prefix_flags = Vec::from_iter(prefix_flags);
        let Some(prefixes) = ordered(&d.pfx_by_flag, &prefix_flags, &mut index_budget) else {
            return;
        };
        let crossable: Vec<_> = first.iter().filter(|(_, s)| s.cross).collect();
        for id in prefixes {
            if !self.step() {
                return;
            }
            let prefix = &d.prefixes[id as usize];
            if d.cont_holds(prefix, d.only_in_compound) {
                continue;
            }
            let paired = d.cont_holds(prefix, d.circumfix);
            let needs = d.cont_holds(prefix, d.need_affix);
            if !paired && !needs && has(self.root_flags, prefix.flag) {
                self.prefix(self.stem, prefix);
            }
            if !prefix.cross {
                continue;
            }
            for &(word, inner) in &crossable {
                if !self.step() {
                    return;
                }
                if paired == d.cont_holds(inner, d.circumfix)
                    && !(needs && d.cont_holds(inner, d.need_affix))
                    && (d.cont_holds(inner, Some(prefix.flag))
                        || self.same_entry(inner.flag, prefix.flag))
                {
                    self.prefix(word, prefix);
                }
            }
        }
        for (word, inner) in &first {
            if inner.cont == 0 {
                continue;
            }
            let paired = d.cont_holds(inner, d.circumfix);
            let Some(suffixes) = ordered(&d.sfx_by_flag, d.set(inner.cont), &mut index_budget)
            else {
                return;
            };
            for id in suffixes {
                if !self.step() {
                    return;
                }
                let outer = &d.suffixes[id as usize];
                if outer.cross
                    && let Some(next) = self.suffix(word, outer)
                {
                    self.cross(&next, inner, outer, paired);
                }
            }
        }
    }

    fn suffix(&self, word: &str, rule: &Rule) -> Option<String> {
        if self.dict.cont_holds(rule, self.dict.only_in_compound) || !rule.cond_matches(word, true)
        {
            return None;
        }
        let base = word.strip_suffix(rule.strip.as_ref())?;
        if base.is_empty() && !self.dict.full_strip {
            return None;
        }
        Some(format!("{base}{}", rule.add))
    }

    fn prefix(&mut self, word: &str, rule: &Rule) {
        if self.dict.cont_holds(rule, self.dict.only_in_compound) || !rule.cond_matches(word, false)
        {
            return;
        }
        if let Some(base) = word.strip_prefix(rule.strip.as_ref())
            && (!base.is_empty() || self.dict.full_strip)
        {
            self.emit(&format!("{}{base}", rule.add));
        }
    }

    /// A prefix licensed by the outer suffix consumes that continuation. Otherwise
    /// a crossing inner suffix must pair its circumfix and license the prefix
    /// through its own continuation or the same dictionary homonym.
    fn cross(&mut self, word: &str, inner: &Rule, outer: &Rule, paired: bool) {
        let d = self.dict;
        let last = d.set(outer.cont);
        let sets = [self.root_flags, d.set(inner.cont), last];
        for (level, flags) in sets.iter().enumerate() {
            for &flag in *flags {
                if !self.step() {
                    return;
                }
                if sets[..level].iter().any(|s| has(s, flag)) {
                    continue;
                }
                if d.pfx_by_flag[usize::from(flag)].is_empty() {
                    continue;
                }
                let outer_grants = has(last, flag);
                let from_outer = outer_grants && !paired;
                let from_inner = !outer_grants
                    && inner.cross
                    && (has(sets[1], flag) || self.same_entry(inner.flag, flag));
                if !from_outer && !from_inner {
                    continue;
                }
                for &id in &d.pfx_by_flag[usize::from(flag)] {
                    if !self.step() {
                        return;
                    }
                    let p = &d.prefixes[id as usize];
                    let matched = from_inner && paired == d.cont_holds(p, d.circumfix);
                    if p.cross && (from_outer || matched) {
                        self.prefix(word, p);
                    }
                }
            }
        }
    }

    fn same_entry(&self, suffix: u16, prefix: u16) -> bool {
        has(self.root_flags, prefix)
            && self.dict.homonyms.get(&self.root_index).is_none_or(|sets| {
                sets.iter()
                    .map(|&id| self.dict.set(id))
                    .any(|entry| has(entry, suffix) && has(entry, prefix))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_three_affix_form_uses_the_outer_grant_first() -> Result<(), String> {
        let aff = "SET UTF-8\nCIRCUMFIX c\nPFX P Y 1\nPFX P 0 ne/c .\nSFX A Y 1\nSFX A 0 able/BcP .\nSFX B Y 1\nSFX B 0 s/P .\n";
        let d = AffixDict::from_hunspell(aff, "1\nwork/AP\n", &[])?;
        let generated = d.forms("work");
        assert!(generated.iter().any(|w| w == "neworkable"));
        assert!(!d.contains("neworkables") && !generated.iter().any(|w| w == "neworkables"));
        Ok(())
    }

    #[test]
    fn forward_crosses_do_not_combine_separately_listed_homonyms() -> Result<(), String> {
        let aff = "SET UTF-8\nPFX P Y 1\nPFX P 0 pre .\nSFX A Y 1\nSFX A 0 ed/B .\nSFX B Y 1\nSFX B 0 s .\n";
        let d = AffixDict::from_hunspell(aff, "2\nwork/P\nwork/A\n", &[])?;
        let mut forms = d.forms("work");
        forms.sort();
        assert_eq!(forms, ["prework", "work", "worked", "workeds"]);
        assert!(forms.iter().all(|w| d.contains(w)));
        Ok(())
    }
}

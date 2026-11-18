//! Match Hunspell COMPOUNDRULE token sequences over dictionary stem boundaries.
//! Bit i means that token i can accept the next part. Optional and repeated tokens
//! may be skipped; repeated tokens also keep their position after consuming a part.
use super::{MAX_COMPOUND_BYTES, MAX_COMPOUND_TOKENS, Repeat, has};

#[derive(Clone, Debug)]
pub(super) struct RuleProgram {
    flags: Vec<(u16, u64)>,
    repeats: u64,
    // Skip runs of 1, 2, 4, 8, 16 or 32 consecutive nullable tokens.
    jumps: [u64; 6],
    accept: u64,
}

impl RuleProgram {
    pub(super) fn new(tokens: impl IntoIterator<Item = (Option<u16>, Repeat)>) -> Self {
        let mut positions = std::collections::BTreeMap::<u16, u64>::new();
        let mut nullable = 0;
        let mut repeats = 0;
        let mut count = 0;
        for (id, op) in tokens {
            debug_assert!(count < MAX_COMPOUND_TOKENS);
            let bit = 1u64 << count;
            if let Some(flag) = id {
                *positions.entry(flag).or_default() |= bit;
            }
            if op != Repeat::Once {
                nullable |= bit;
            }
            if op == Repeat::Any {
                repeats |= bit;
            }
            count += 1;
        }
        let mut jumps = [0; 6];
        let mut reach = nullable;
        for (i, jump) in jumps.iter_mut().enumerate() {
            *jump = reach;
            reach &= reach >> (1 << i);
        }
        Self {
            flags: positions.into_iter().collect(),
            repeats,
            jumps,
            accept: 1 << count,
        }
    }

    pub(super) fn flags(&self) -> impl Iterator<Item = u16> + '_ {
        self.flags.iter().map(|(flag, _)| *flag)
    }

    fn closure(&self, mut states: u64) -> u64 {
        for (i, mask) in self.jumps.iter().enumerate() {
            if *mask == 0 {
                break;
            }
            states |= (states & mask) << (1 << i);
        }
        states
    }

    pub(super) fn positions(&self, active: u64, flags: &[u16]) -> u64 {
        let mut matched = 0;
        for &(flag, positions) in &self.flags {
            if active & positions != 0 && has(flags, flag) {
                matched |= active & positions;
            }
        }
        matched
    }

    pub(super) fn accepts(
        &self,
        cuts: usize,
        minimum: usize,
        mut matched_at: impl FnMut(usize, usize, u64) -> u64,
    ) -> bool {
        // The caller bounds the UTF-8 word length and every rule to 32 tokens.
        let mut states = [0u64; MAX_COMPOUND_BYTES + 1];
        let live = &mut states[..cuts];
        live[0] = self.closure(1);
        let last = cuts - 1;
        for start in 0..last {
            let active = live[start] & (self.accept - 1);
            if active == 0 {
                continue;
            }
            for (end, next) in live
                .iter_mut()
                .enumerate()
                .skip(start.saturating_add(minimum))
            {
                let matched = matched_at(start, end, active);
                if matched == 0 {
                    continue;
                }
                let reached =
                    self.closure((matched & self.repeats) | ((matched & !self.repeats) << 1));
                // A complete word must contain at least two nonempty dictionary parts.
                if end == last && start != 0 && reached & self.accept != 0 {
                    return true;
                }
                *next |= reached;
            }
        }
        false
    }
}

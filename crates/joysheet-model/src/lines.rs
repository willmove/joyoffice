//! Copy-on-write line metadata and immutable prefix indexes, shared across workbook snapshots.
use crate::{LineIndex, LineInfo};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Lines {
    entries: Arc<BTreeMap<u32, LineInfo>>,
    #[serde(skip)]
    index: Arc<OnceLock<(u32, u32, Arc<LineIndex>)>>,
}
impl PartialEq for Lines {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.entries, &other.entries) || self.entries == other.entries
    }
}
impl Deref for Lines {
    type Target = BTreeMap<u32, LineInfo>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
impl DerefMut for Lines {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Every mutation route, including get_mut/entry/retain and borrowed map helpers,
        // detaches the index before exposing the map. Old snapshots keep their own index.
        self.index = Arc::default();
        Arc::make_mut(&mut self.entries)
    }
}
impl<'a> IntoIterator for &'a Lines {
    type Item = (&'a u32, &'a LineInfo);
    type IntoIter = std::collections::btree_map::Iter<'a, u32, LineInfo>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}
impl IntoIterator for Lines {
    type Item = (u32, LineInfo);
    type IntoIter = std::collections::btree_map::IntoIter<u32, LineInfo>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::unwrap_or_clone(self.entries).into_iter()
    }
}
impl Lines {
    pub fn index(&self, default: f32, max: u32) -> Arc<LineIndex> {
        let (bits, limit, index) = self.index.get_or_init(|| (default.to_bits(), max, Arc::new(LineIndex::new(self, default, max))));
        if *bits == default.to_bits() && *limit == max { index.clone() } else { Arc::new(LineIndex::new(self, default, max)) }
    }
}

#[cfg(test)]
#[path = "tests/lines_tests.rs"]
mod tests;

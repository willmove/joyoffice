//! Serde helpers for maps keyed by `CellRef`. JSON object keys must be strings, so the keys are
//! written as `A1`-style text.

use std::collections::BTreeMap;

use joysheet_core::CellRef;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S: Serializer, V: Serialize>(map: &BTreeMap<CellRef, V>, s: S) -> Result<S::Ok, S::Error> {
    s.collect_map(map.iter().map(|(k, v)| (k.a1(), v)))
}

pub fn deserialize<'de, D: Deserializer<'de>, V: Deserialize<'de>>(d: D) -> Result<BTreeMap<CellRef, V>, D::Error> {
    let raw = BTreeMap::<String, V>::deserialize(d)?;
    let mut out = BTreeMap::new();
    for (k, v) in raw {
        let cell = CellRef::parse(&k).ok_or_else(|| D::Error::custom(format!("invalid cell reference `{k}`")))?;
        out.insert(cell, v);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "tests/cellmap_tests.rs"]
mod tests;

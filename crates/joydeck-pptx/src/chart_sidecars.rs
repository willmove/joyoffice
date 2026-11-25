//! Relationships of unchanged chart parts and relocation of their owned payloads.

use std::collections::{HashMap, HashSet, hash_map::Entry};

use joydeck_model::Chart;
use joydeck_model::chart::{ChartKept, ChartPart, ChartRel};

use crate::opc::{self, Package, PackageOut, RelsOut};
use crate::read::Imp;

const MAX_MEMBERS: usize = 256;
const MAX_DEPTH: usize = 8;
const MAX_BYTES: usize = 32 << 20;

pub(super) fn capture(imp: &mut Imp, chart_path: &str) -> ChartKept {
    let key = opc::norm(chart_path);
    if let Some(known) = imp.kept_charts.get(&key) {
        return known.clone();
    }
    let mut collection = Collection {
        package: imp.pkg,
        bytes_used: &mut imp.kept_sidecar_bytes,
        present: HashSet::from([key.clone()]),
        members: Vec::new(),
    };
    let mut relations = Vec::new();
    for rel in imp.pkg.rels(chart_path).items {
        if ![&rel.id, &rel.ty, &rel.target_raw]
            .into_iter()
            .all(|value| {
                !value.is_empty()
                    && value.len() <= 4096
                    && !value
                        .chars()
                        .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"' | '\''))
            })
        {
            log::warn!("pptx: skipping chart relationship {}", rel.id);
            continue;
        }
        if rel.external || collection.add(&rel.target, 0) {
            relations.push(ChartRel {
                id: rel.id,
                rel_type: rel.ty,
                target: rel.target_raw,
                part: if rel.external {
                    String::new()
                } else {
                    rel.target
                },
                external: rel.external,
            });
        }
    }
    let kept = ChartKept {
        rels: relations,
        parts: collection.members,
    };
    imp.kept_charts.insert(key, kept.clone());
    kept
}

struct Collection<'p, 'b> {
    package: &'p Package,
    bytes_used: &'b mut usize,
    present: HashSet<String>,
    members: Vec<ChartPart>,
}

impl Collection<'_, '_> {
    fn add(&mut self, path: &str, depth: usize) -> bool {
        if depth > MAX_DEPTH || self.members.len() >= MAX_MEMBERS {
            log::warn!("pptx: chart part closure stopped at {path}");
            return false;
        }
        if self.present.contains(&opc::norm(path)) {
            return true;
        }
        let content_type = self.package.content_type(path).unwrap_or_default();
        if !self.store(path, content_type) {
            return false;
        }
        let relationships = opc::rels_path(path);
        if !self.present.contains(&opc::norm(&relationships))
            && self.store(
                &relationships,
                "application/vnd.openxmlformats-package.relationships+xml".into(),
            )
        {
            for rel in self.package.rels(path).items {
                if !rel.external {
                    self.add(&rel.target, depth + 1);
                }
            }
        }
        true
    }

    fn store(&mut self, path: &str, content_type: String) -> bool {
        if self.members.len() >= MAX_MEMBERS || !opc::sidecar_part(path) {
            return false;
        }
        let Some(bytes) = self.package.get(path) else {
            return false;
        };
        let name = self
            .package
            .original_name(path)
            .unwrap_or(path)
            .trim_start_matches('/');
        if !opc::sidecar_part(name) {
            return false;
        }
        let data = if bytes.len() <= MAX_BYTES.saturating_sub(*self.bytes_used) {
            *self.bytes_used += bytes.len();
            bytes.to_vec()
        } else {
            log::warn!("pptx: chart sidecar retained byte budget exceeded for {path}");
            Vec::new()
        };
        self.members.push(ChartPart {
            name: name.into(),
            content_type,
            bytes: data,
            native_entry: None,
        });
        self.present.insert(opc::norm(path));
        true
    }
}

fn payload(part: &ChartPart) -> bool {
    !opc::norm(&part.name).ends_with(".rels") && opc::sidecar_part(&part.name)
}

pub(super) fn relationship_ids(chart: &Chart) -> HashSet<&str> {
    let present: HashSet<_> = chart
        .kept
        .parts
        .iter()
        .filter(|part| payload(part))
        .map(|part| opc::norm(&part.name))
        .collect();
    chart
        .kept
        .rels
        .iter()
        .filter(|edge| edge.external || present.contains(&opc::norm(&edge.part)))
        .map(|edge| edge.id.as_str())
        .collect()
}

pub(super) fn write(package: &mut PackageOut, chart_path: &str, chart: &Chart, next: &mut u32) {
    if chart.kept.rels.is_empty() {
        return;
    }
    let mut renamed = HashMap::new();
    let mut moved = Vec::new();
    for part in chart.kept.parts.iter().filter(|part| payload(part)) {
        if let Entry::Vacant(entry) = renamed.entry(opc::norm(&part.name)) {
            *next += 1;
            let destination = destination(entry.key(), *next);
            let content_type = if part.content_type.is_empty() {
                crate::mime_for(&part.name)
            } else {
                &part.content_type
            };
            package.xml_part(&destination, content_type, part.bytes.clone());
            moved.push((part, destination.clone()));
            entry.insert(destination);
        }
    }
    let mut chart_rels = RelsOut::default();
    for rel in &chart.kept.rels {
        let target = relocate(chart_path, &renamed, rel.external, &rel.target, &rel.part);
        if let Some(target) = target {
            chart_rels
                .items
                .push((rel.id.clone(), rel.rel_type.clone(), target, rel.external));
        }
    }
    package.rels(chart_path, &chart_rels);

    let mut originals = HashMap::new();
    for part in &chart.kept.parts {
        originals
            .entry(opc::norm(&part.name))
            .or_insert(&part.bytes);
    }
    for (old, new) in moved {
        if let Some(bytes) = originals.get(&opc::norm(&opc::rels_path(&old.name))) {
            let mut relations = RelsOut::default();
            for rel in opc::rels_from_bytes(bytes, &old.name).items {
                if let Some(target) =
                    relocate(&new, &renamed, rel.external, &rel.target_raw, &rel.target)
                {
                    relations.items.push((rel.id, rel.ty, target, rel.external));
                }
            }
            package.rels(&new, &relations);
        }
    }
}

fn relocate(
    owner: &str,
    renamed: &HashMap<String, String>,
    external: bool,
    uri: &str,
    part: &str,
) -> Option<String> {
    if external {
        Some(uri.into())
    } else {
        renamed
            .get(&opc::norm(part))
            .map(|path| opc::relative(owner, path))
    }
}

fn destination(old: &str, index: u32) -> String {
    let area = old.split('/').nth(1).unwrap_or("embeddings");
    let suffix = opc::file_of(old)
        .rsplit_once('.')
        .map(|(_, suffix)| suffix)
        .filter(|suffix| {
            (1..=8).contains(&suffix.len()) && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .unwrap_or("bin");
    format!("ppt/{area}/kept{index}.{suffix}")
}

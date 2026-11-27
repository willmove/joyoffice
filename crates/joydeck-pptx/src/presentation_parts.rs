//! Ordered slide and master references in a PresentationML document part.

use std::collections::{HashMap, HashSet, hash_map::Entry};

use crate::opc::{self, Package, Rels};
use crate::read::{load, max_cnvpr};
use crate::xml::{Doc, El};

const SLIDE_LIMIT: usize = 10_000;

pub(super) struct Inventory {
    pub masters: Vec<String>,
    pub slides: Vec<(u32, String)>,
    pub documents: HashMap<String, Doc>,
    pub max_shape_id: u32,
}

impl Inventory {
    pub(super) fn read(package: &Package, root: &El, rels: &Rels) -> Self {
        let masters = master_paths(root, rels);
        let mut distinct = HashSet::new();
        let slides: Vec<_> = root
            .child("sldIdLst")
            .into_iter()
            .flat_map(|list| list.children_named("sldId"))
            .take(SLIDE_LIMIT)
            .filter_map(|item| {
                let path = target(item, rels, "slide")?;
                distinct
                    .insert(opc::norm(path))
                    .then(|| (item.u32("id").unwrap_or_default(), path.to_owned()))
            })
            .collect();
        let mut inventory = Self {
            masters,
            slides,
            documents: HashMap::new(),
            max_shape_id: 0,
        };
        inventory.load_documents(package);
        inventory
    }

    fn load_documents(&mut self, package: &Package) {
        let Self {
            masters,
            slides,
            documents,
            max_shape_id,
        } = self;
        let mut insert = |path: &str| {
            if let Entry::Vacant(slot) = documents.entry(opc::norm(path))
                && let Some(doc) = load(package, path)
            {
                max_cnvpr(&doc.root, max_shape_id);
                slot.insert(doc);
            }
        };
        for master in masters {
            insert(master);
            for layout in package
                .rels(master)
                .all_of("slideLayout")
                .filter(|rel| !rel.external)
            {
                insert(&layout.target);
            }
        }
        for (_, slide) in slides {
            insert(slide);
        }
    }
}

pub(super) fn master_paths(root: &El, rels: &Rels) -> Vec<String> {
    let mut masters: Vec<_> = root
        .child("sldMasterIdLst")
        .into_iter()
        .flat_map(|list| list.children_named("sldMasterId"))
        .filter_map(|item| target(item, rels, "slideMaster").map(str::to_owned))
        .collect();
    if masters.is_empty() {
        masters.extend(
            rels.all_of("slideMaster")
                .filter(|rel| !rel.external)
                .map(|rel| rel.target.clone()),
        );
    }
    let mut distinct = HashSet::new();
    masters.retain(|path| distinct.insert(opc::norm(path)));
    masters
}

pub(super) fn target<'a>(item: &El, rels: &'a Rels, kind: &str) -> Option<&'a str> {
    let rel = rels.get(item.attr("r:id")?)?;
    (rel.kind == kind && !rel.external).then_some(rel.target.as_str())
}

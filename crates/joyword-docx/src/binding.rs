//! Verified export binding retained after replacing an original package.
//!
//! Re-importing edited bytes allocates fresh model IDs and new XML locators.
//! The live engine must keep its stable IDs and undo history, while subsequent
//! exports must use the new locators. `ExportBinding` joins those views: the
//! baseline keeps the live IDs, paragraphs receive the verified new locators,
//! and later snapshots are projected through the same immutable map.

use std::collections::BTreeMap;
use std::sync::Arc;

use joyoffice_opc::Budget;
use joyword_model::{Block, BlockSeq, Document, DocumentSnapshot, NodeId, PreservedId, SourceRef};

use crate::export::plan_export_with_original;
use crate::{Editability, ExportError, plan_export};
use crate::{ExportMode, ExportPlan, PackageBacking, import};

#[derive(Clone, Debug)]
pub struct ExportBinding {
    backing: PackageBacking,
    baseline: DocumentSnapshot,
    sources: BTreeMap<NodeId, Option<Arc<SourceRef>>>,
    editability: Editability,
    handles: BTreeMap<PreservedId, PreservedId>,
}

impl ExportBinding {
    /// Verify `backing` against the exact live snapshot that was just saved.
    /// No file IO occurs here. A mismatch refuses the binding before a caller
    /// can publish the package as the next save's source baseline.
    pub fn rebind(
        backing: PackageBacking,
        saved: &DocumentSnapshot,
        budget: &Budget,
    ) -> Result<Self, ExportError> {
        let previous = backing.clone();
        Self::rebind_from_previous(backing, saved, &previous, budget)
    }

    /// First original save: verify preserved fragments against the package
    /// that owns the live model's handles before mapping the new locators.
    pub fn rebind_from_previous(
        backing: PackageBacking,
        saved: &DocumentSnapshot,
        previous: &PackageBacking,
        budget: &Budget,
    ) -> Result<Self, ExportError> {
        Self::rebind_checked(backing, saved, saved, previous, None, budget)
    }

    /// Subsequent original save with stable live IDs and original handles.
    pub fn rebind_next(
        &self,
        backing: PackageBacking,
        saved: &DocumentSnapshot,
        budget: &Budget,
    ) -> Result<Self, ExportError> {
        let projected = rebase(saved, &self.sources, &self.handles)?;
        Self::rebind_checked(
            backing,
            saved,
            &projected,
            &self.backing,
            Some(&self.handles),
            budget,
        )
    }

    fn rebind_checked(
        backing: PackageBacking,
        saved: &DocumentSnapshot,
        projected: &DocumentSnapshot,
        previous: &PackageBacking,
        prior_handles: Option<&BTreeMap<PreservedId, PreservedId>>,
        budget: &Budget,
    ) -> Result<Self, ExportError> {
        let imported = import(backing.original_bytes(), budget)?;
        if !crate::export::compare::equivalent(projected, previous, &imported.document, &backing) {
            return Err(ExportError::RoundTripMismatch);
        }
        let next_handles =
            crate::export::compare::handle_map(projected, previous, &imported.document, &backing)
                .ok_or(ExportError::RoundTripMismatch)?;
        let handles = match prior_handles {
            Some(prior) => prior
                .iter()
                .map(|(old, current)| Some((*old, *next_handles.get(current)?)))
                .collect::<Option<BTreeMap<_, _>>>()
                .ok_or(ExportError::RoundTripMismatch)?,
            None => next_handles,
        };
        let mapped = source_map(saved, &imported.document)?;
        let editability = imported
            .editability
            .remap_locked(&mapped.ids)
            .ok_or(ExportError::RoundTripMismatch)?;
        let sources = mapped.sources;
        let baseline = rebase(saved, &sources, &handles)?;
        let plan = plan_export_with_original(&backing, &baseline, &baseline, budget, &editability)?;
        if plan.mode() != ExportMode::OriginalPackage {
            return Err(ExportError::RoundTripMismatch);
        }
        Ok(Self {
            backing,
            baseline,
            sources,
            editability,
            handles,
        })
    }

    pub fn backing(&self) -> &PackageBacking {
        &self.backing
    }

    /// Restrictions expressed using the stable live block IDs.
    pub fn editability(&self) -> &Editability {
        &self.editability
    }

    /// Plan against the saved baseline, refreshing source locators on every
    /// surviving stable paragraph while leaving post-save node IDs untouched.
    pub fn plan(
        &self,
        current: &DocumentSnapshot,
        budget: &Budget,
    ) -> Result<ExportPlan, ExportError> {
        let current = rebase(current, &self.sources, &self.handles)?;
        plan_export_with_original(
            &self.backing,
            &self.baseline,
            &current,
            budget,
            &self.editability,
        )
    }

    /// Ordinary first-save path, provided so callers can use one code path for
    /// an initial import and all subsequent rebound saves.
    pub fn plan_initial(
        backing: &PackageBacking,
        current: &DocumentSnapshot,
        budget: &Budget,
    ) -> Result<ExportPlan, ExportError> {
        plan_export(backing, current, budget)
    }
}

struct SourceMap {
    sources: BTreeMap<NodeId, Option<Arc<SourceRef>>>,
    ids: BTreeMap<NodeId, NodeId>,
}

fn source_map(saved: &DocumentSnapshot, imported: &Document) -> Result<SourceMap, ExportError> {
    fn walk(live: &BlockSeq, fresh: &BlockSeq, map: &mut SourceMap) -> Result<(), ExportError> {
        if live.len() != fresh.len() {
            return Err(ExportError::RoundTripMismatch);
        }
        for (a, b) in live.iter().zip(fresh.iter()) {
            map.ids.insert(b.id(), a.id());
            let source = match (a, b) {
                (Block::Paragraph(_), Block::Paragraph(b)) => b.source.clone(),
                (Block::Table(a), Block::Table(b)) => {
                    if a.rows.len() != b.rows.len() {
                        return Err(ExportError::RoundTripMismatch);
                    }
                    for (a, b) in a.rows.iter().zip(&b.rows) {
                        if a.cells.len() != b.cells.len() {
                            return Err(ExportError::RoundTripMismatch);
                        }
                        for (a, b) in a.cells.iter().zip(&b.cells) {
                            walk(&a.blocks, &b.blocks, map)?;
                        }
                    }
                    b.source.clone()
                }
                (Block::Sdt(a), Block::Sdt(b)) => {
                    walk(&a.blocks, &b.blocks, map)?;
                    b.source.clone()
                }
                (Block::Opaque(_), Block::Opaque(b)) => b.source.clone(),
                _ => return Err(ExportError::RoundTripMismatch),
            };
            map.sources.insert(a.id(), source);
        }
        Ok(())
    }
    let live: Vec<_> = saved.stories().collect();
    let fresh: Vec<_> = imported.stories().collect();
    if live.len() != fresh.len() {
        return Err(ExportError::RoundTripMismatch);
    }
    let mut map = SourceMap {
        sources: BTreeMap::new(),
        ids: BTreeMap::new(),
    };
    for (a, b) in live.into_iter().zip(fresh) {
        if a.kind != b.kind {
            return Err(ExportError::RoundTripMismatch);
        }
        walk(&a.blocks, &b.blocks, &mut map)?;
    }
    Ok(map)
}

fn rebase(
    current: &DocumentSnapshot,
    sources: &BTreeMap<NodeId, Option<Arc<SourceRef>>>,
    handles: &BTreeMap<PreservedId, PreservedId>,
) -> Result<DocumentSnapshot, ExportError> {
    fn walk(blocks: &mut BlockSeq, sources: &BTreeMap<NodeId, Option<Arc<SourceRef>>>) {
        for i in 0..blocks.len() {
            blocks.update(i, |b| {
                let source = sources.get(&b.id());
                match b {
                    Block::Paragraph(p) => {
                        if let Some(s) = source {
                            Arc::make_mut(p).source = s.clone();
                        }
                    }
                    Block::Table(t) => {
                        let t = Arc::make_mut(t);
                        if let Some(s) = source {
                            t.source = s.clone();
                        }
                        for r in &mut t.rows {
                            for c in &mut r.cells {
                                walk(&mut c.blocks, sources);
                            }
                        }
                    }
                    Block::Sdt(s) => {
                        let s = Arc::make_mut(s);
                        if let Some(source) = source {
                            s.source = source.clone();
                        }
                        walk(&mut s.blocks, sources);
                    }
                    Block::Opaque(o) => {
                        if let Some(s) = source {
                            Arc::make_mut(o).source = s.clone();
                        }
                    }
                }
            });
        }
    }
    let mut document = crate::export::compare::normalize(current, &|id| handles.get(&id).copied())
        .ok_or(ExportError::RoundTripMismatch)?;
    let ids: Vec<_> = document.stories().map(|s| s.id).collect();
    for id in ids {
        walk(
            &mut document
                .story_mut(id)
                .ok_or(ExportError::RoundTripMismatch)?
                .blocks,
            sources,
        );
    }
    Ok(document.snapshot())
}

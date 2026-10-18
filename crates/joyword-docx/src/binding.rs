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
use joyword_model::{Block, BlockSeq, Document, DocumentSnapshot, NodeId, SourceRef};

use crate::export::plan_export_with_original;
use crate::{ExportError, plan_export};
use crate::{ExportMode, ExportPlan, PackageBacking, import};

#[derive(Clone, Debug)]
pub struct ExportBinding {
    backing: PackageBacking,
    baseline: DocumentSnapshot,
    sources: BTreeMap<NodeId, Option<Arc<SourceRef>>>,
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
        let imported = import(backing.original_bytes(), budget)?;
        if imported.document.semantic_difference(saved).is_some() {
            return Err(ExportError::RoundTripMismatch);
        }
        let sources = source_map(saved, &imported.document)?;
        let baseline = rebase(saved, &sources)?;
        let plan = plan_export_with_original(&backing, &baseline, &baseline, budget)?;
        if plan.mode() != ExportMode::OriginalPackage {
            return Err(ExportError::RoundTripMismatch);
        }
        Ok(Self {
            backing,
            baseline,
            sources,
        })
    }

    pub fn backing(&self) -> &PackageBacking {
        &self.backing
    }

    /// Plan against the saved baseline, refreshing source locators on every
    /// surviving stable paragraph while leaving post-save node IDs untouched.
    pub fn plan(
        &self,
        current: &DocumentSnapshot,
        budget: &Budget,
    ) -> Result<ExportPlan, ExportError> {
        let current = rebase(current, &self.sources)?;
        plan_export_with_original(&self.backing, &self.baseline, &current, budget)
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

fn source_map(
    saved: &DocumentSnapshot,
    imported: &Document,
) -> Result<BTreeMap<NodeId, Option<Arc<SourceRef>>>, ExportError> {
    if saved.body().blocks.len() != imported.body().blocks.len() {
        return Err(ExportError::RoundTripMismatch);
    }
    let mut sources = BTreeMap::new();
    for (live, fresh) in saved
        .body()
        .blocks
        .iter()
        .zip(imported.body().blocks.iter())
    {
        if let (Block::Paragraph(live), Block::Paragraph(fresh)) = (live, fresh) {
            sources.insert(live.id(), fresh.source.clone());
        }
    }
    Ok(sources)
}

fn rebase(
    current: &DocumentSnapshot,
    sources: &BTreeMap<NodeId, Option<Arc<SourceRef>>>,
) -> Result<DocumentSnapshot, ExportError> {
    let mut document: Document = (**current).clone();
    let blocks = document
        .body()
        .blocks
        .iter()
        .cloned()
        .map(|mut block| {
            if let Block::Paragraph(paragraph) = &mut block
                && let Some(source) = sources.get(&paragraph.id())
            {
                Arc::make_mut(paragraph).source = source.clone();
            }
            block
        })
        .collect::<Vec<_>>();
    let story = document.body_id();
    document
        .story_mut(story)
        .ok_or(ExportError::RoundTripMismatch)?
        .blocks = BlockSeq::from_vec(blocks);
    Ok(document.snapshot())
}

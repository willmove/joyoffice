//! Offline comparison and one-object acceptance, guarded by both document digests.
use crate::{Cancellation, Document, editor::Editor};
use joydeck_engine::{Command, transfer};
use joydeck_model::{Presentation, ShapeId};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    pub page: usize,
    pub object: Option<ShapeId>,
    pub kind: &'static str,
    pub acceptable: bool,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub base_sha256: String,
    pub reference_sha256: String,
    pub differences: Vec<Difference>,
    pub truncated: bool,
}
pub struct Comparison {
    pub report: Report,
    reference: Document,
    reference_pages: Vec<usize>,
}
pub fn digest(p: &Presentation) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(p).map_err(|e| e.to_string())?);
    for item in &p.media {
        hash.update(item.id.0.to_le_bytes());
        hash.update(Sha256::digest(item.data.as_slice()));
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl Comparison {
    pub fn open(base: &Editor, path: &Path) -> Result<Self, String> {
        let reference = Document::open(path, &Cancellation::default())?;
        Self::new(base, reference)
    }
    pub fn new(base: &Editor, reference: Document) -> Result<Self, String> {
        Self::from_model(base.session.model(), reference)
    }
    pub fn from_model(p: &Presentation, reference: Document) -> Result<Self, String> {
        let q = &reference.presentation;
        let reference_by_id: BTreeMap<_, _> = q
            .slides
            .iter()
            .enumerate()
            .map(|(page, slide)| (slide.id, page))
            .collect();
        // Only treat the two sequences as a permutation when both contain the
        // same unique IDs. Otherwise retain positional comparison.
        let paired: Option<Vec<_>> = p
            .slides
            .iter()
            .map(|slide| reference_by_id.get(&slide.id).copied())
            .collect();
        let reference_pages = paired
            .filter(|pages| {
                pages.len() == q.slides.len()
                    && pages.iter().collect::<BTreeSet<_>>().len() == pages.len()
            })
            .unwrap_or_else(|| (0..p.slides.len().max(q.slides.len())).collect());
        let mut differences = vec![];
        if reference_pages.iter().copied().ne(0..reference_pages.len()) {
            differences.push(Difference {
                page: 0,
                object: None,
                kind: "slide-order",
                acceptable: false,
                reason: Some("slide order is report-only".into()),
            });
        }
        for (page, &reference_page) in reference_pages.iter().enumerate() {
            let (Some(left), Some(right)) = (p.slides.get(page), q.slides.get(reference_page))
            else {
                differences.push(Difference {
                    page: page + 1,
                    object: None,
                    kind: "slide-added-or-deleted",
                    acceptable: false,
                    reason: Some("accept complete slide structure separately".into()),
                });
                continue;
            };
            let mut left_properties = left.as_ref().clone();
            let mut right_properties = right.as_ref().clone();
            left_properties.shapes.clear();
            right_properties.shapes.clear();
            if left_properties != right_properties
                || left.shapes.iter().map(|s| s.id).collect::<Vec<_>>()
                    != right.shapes.iter().map(|s| s.id).collect::<Vec<_>>()
            {
                differences.push(Difference {
                    page: page + 1,
                    object: None,
                    kind: "slide-properties",
                    acceptable: false,
                    reason: Some("page properties are report-only".into()),
                });
            }
            let ids: BTreeSet<_> = left
                .shapes
                .iter()
                .chain(&right.shapes)
                .map(|s| s.id)
                .collect();
            for id in ids {
                if differences.len() >= 4096 {
                    break;
                }
                let a = left.shape(id);
                let b = right.shape(id);
                let same = a == b
                    && transfer::capture(p, page, &[id])
                        .ok()
                        .zip(transfer::capture(q, reference_page, &[id]).ok())
                        .is_none_or(|(a, b)| {
                            serde_json::to_vec(&a).ok() == serde_json::to_vec(&b).ok()
                        });
                if same {
                    continue;
                }
                let kind = match (a, b) {
                    (None, _) => "added",
                    (_, None) => "deleted",
                    _ => "changed",
                };
                let reason = if a.is_some_and(|s| s.locked) {
                    Some("object is locked".into())
                } else if b.is_some() {
                    transfer::capture(q, reference_page, &[id]).err()
                } else {
                    None
                };
                differences.push(Difference {
                    page: page + 1,
                    object: Some(id),
                    kind,
                    acceptable: reason.is_none(),
                    reason,
                });
            }
        }
        let mut properties = p.clone();
        let mut reference_properties = q.clone();
        for doc in [&mut properties, &mut reference_properties] {
            doc.slides.clear();
            doc.media.clear();
            doc.next_id = 0;
        }
        if properties != reference_properties {
            differences.push(Difference {
                page: 0,
                object: None,
                kind: "document-properties",
                acceptable: false,
                reason: Some("document settings/themes are report-only".into()),
            });
        }
        if p.media != q.media {
            differences.push(Difference {
                page: 0,
                object: None,
                kind: "document-resources",
                acceptable: false,
                reason: Some(
                    "resource changes are report-only; inspect object differences separately"
                        .into(),
                ),
            });
        }
        let truncated = differences.len() >= 4096;
        differences.truncate(4096);
        Ok(Self {
            report: Report {
                base_sha256: digest(p)?,
                reference_sha256: digest(q)?,
                differences,
                truncated,
            },
            reference,
            reference_pages,
        })
    }
    pub fn command(
        &self,
        editor: &Editor,
        base: &str,
        reference: &str,
        page: usize,
        object: ShapeId,
    ) -> Result<Command, String> {
        if base != self.report.base_sha256
            || reference != self.report.reference_sha256
            || digest(editor.session.model())? != base
        {
            return Err("comparison is stale; compare again".into());
        }
        let diff = self
            .report
            .differences
            .iter()
            .find(|d| d.page == page && d.object == Some(object) && d.acceptable)
            .ok_or("object difference is not acceptable")?;
        let target = (diff.kind != "added").then_some(object);
        let replacement = if diff.kind == "deleted" {
            None
        } else {
            Some(Box::new(transfer::capture(
                &self.reference.presentation,
                self.reference_pages[page - 1],
                &[object],
            )?))
        };
        Ok(Command::AcceptObject {
            page,
            target,
            replacement,
        })
    }
}

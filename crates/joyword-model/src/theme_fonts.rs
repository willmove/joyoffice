//! Refresh layout's cached Latin theme font without detaching the imported binding.
use crate::{Blocks, Document, props::CharProps};
use std::sync::Arc;

fn transform(props: &mut CharProps, edit: &dyn Fn(&mut CharProps), depth: usize) {
    edit(props);
    if depth < 64
        && let Some(change) = props.fmt_change.as_mut()
    {
        transform(&mut change.old, edit, depth + 1);
    }
}

fn blocks(blocks: &mut Blocks, edit: &dyn Fn(&mut CharProps)) {
    let relevant = |props: &CharProps| props.theme_font.is_some() || props.fmt_change.is_some();
    for block in blocks.iter_mut() {
        let mut any = false;
        crate::edit::each_para(block, 0, &mut |p| {
            any |= relevant(&p.mark) || p.runs.iter().any(|r| relevant(&r.props));
        });
        if !any {
            continue;
        }
        crate::edit::each_para_mut(Arc::make_mut(block), 0, &mut |para| {
            if relevant(&para.mark) || para.runs.iter().any(|r| relevant(&r.props)) {
                transform(&mut para.mark, edit, 0);
                for run in &mut para.runs {
                    transform(&mut run.props, edit, 0);
                }
                para.touch();
            }
        });
    }
}

impl Document {
    /// Refresh live stories, style regions and list definitions after changing theme fonts.
    /// Kept glossary entries are not rewritten just to refresh a layout cache.
    pub fn refresh_theme_fonts(&mut self, major: &str, minor: &str) {
        self.transform_theme_fonts(&|props| {
            props.refresh_theme_font(major, minor);
        });
    }

    /// Freeze source formatting when moving content to a document with another theme.
    pub(crate) fn detach_theme_fonts(&mut self) {
        self.transform_theme_fonts(&|props| {
            props.theme_font = None;
        });
    }

    fn transform_theme_fonts(&mut self, edit: &dyn Fn(&mut CharProps)) {
        transform(&mut self.styles.default_chr, edit, 0);
        for style in &mut self.styles.styles {
            transform(&mut style.chr, edit, 0);
            if let Some(table) = &mut style.table {
                for chr in [
                    &mut table.header_chr,
                    &mut table.band_chr,
                    &mut table.first_col_chr,
                    &mut table.total_chr,
                    &mut table.last_col_chr,
                    &mut table.col_band_chr,
                ] {
                    transform(chr, edit, 0);
                }
                for region in table.regions.values_mut() {
                    transform(&mut region.chr, edit, 0);
                }
            }
        }
        for list in &mut self.numbering.abstracts {
            for level in &mut list.levels {
                transform(&mut level.chr, edit, 0);
            }
        }
        for list in &mut self.numbering.nums {
            for (_, level) in &mut list.level_overrides {
                transform(&mut level.chr, edit, 0);
            }
        }
        blocks(&mut self.body, edit);
        for part in self.parts.values_mut() {
            blocks(&mut part.blocks, edit);
        }
        for notes in [&mut self.footnote_separators, &mut self.endnote_separators] {
            for story in [
                &mut notes.separator,
                &mut notes.continuation_separator,
                &mut notes.continuation_notice,
            ]
            .into_iter()
            .flatten()
            {
                blocks(story, edit);
            }
        }
    }
}

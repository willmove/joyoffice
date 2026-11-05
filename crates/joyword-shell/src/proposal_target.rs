//! Selection and transaction boundary for reviewed replacements.
use crate::reader::Reader;
use joyword_engine::{
    Selection,
    doc::{StoryRef, para::OBJ},
};

#[derive(Clone)]
pub struct ProposalTarget(Selection);

impl Reader {
    pub fn proposal_target(&self) -> ProposalTarget {
        ProposalTarget(self.session.sel.clone())
    }
}

impl ProposalTarget {
    pub fn text<'a>(&self, reader: &'a Reader) -> Option<&'a str> {
        eligible_text(reader, &self.0)
    }

    pub fn apply(&self, reader: &mut Reader, replacement: &str) -> Result<(), String> {
        self.text(reader).ok_or("The proposal selection changed")?;
        reader
            .session
            .apply_agent_proposal(replacement)
            .map_err(|e| e.to_string())?;
        reader.refresh();
        Ok(())
    }
}

fn eligible_text<'a>(reader: &'a Reader, selection: &Selection) -> Option<&'a str> {
    let s = reader.engine();
    let (a, b) = selection.ordered();
    if s.sel != *selection
        || selection.is_collapsed()
        || a.story != StoryRef::Body
        || a.path != b.path
        || b.story != a.story
        || reader.is_composing()
        || s.doc.settings.protection.is_some()
        || s.recording.is_some()
        || s.column.is_some()
    {
        return None;
    }
    let p = s.doc.para_at(&a)?;
    if !p.objects.is_empty() {
        return None;
    }
    let text = p.text.get(a.off..b.off)?;
    if text.len() > 24 * 1024
        || text
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, OBJ | '\u{fdd0}' | '\u{2028}' | '\u{2029}'))
        || a.path.depth() > 0
    {
        return None;
    }
    let style = s.doc.styles.resolve_para(&p.props);
    if p.run_ranges().any(|(range, run)| {
        if range.end <= a.off || range.start >= b.off {
            return false;
        }
        let resolved = s.doc.styles.resolve_char(Some(&style.style), run);
        resolved.hidden || resolved.ins.is_some() || resolved.del.is_some()
    }) {
        return None;
    }
    Some(text)
}

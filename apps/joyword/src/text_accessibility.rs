//! TextPattern data for the same bounded input projection the editor uses.
//! Build only when GPUI requests an accessibility subtree, not on every paint.
use gpui::accesskit::{Node, NodeId, Role, TextPosition, TextSelection};
use joyword_layout::LayoutSnapshot;
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::input::InputView;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

const MAX_VISUAL_RUNS: usize = 20_000;

#[derive(Clone, Debug)]
pub(crate) struct TextRange {
    pub node: NodeId,
    pub utf16: Range<usize>,
    bytes: Range<usize>,
}

impl TextRange {
    pub fn offset(&self, text: &str, character: usize) -> Option<usize> {
        let value = text.get(self.bytes.clone())?;
        let mut offset = self.utf16.start;
        let mut count = 0;
        for ch in value.chars().take(character) {
            offset += ch.len_utf16();
            count += 1;
        }
        (count == character).then_some(offset)
    }
}

pub(crate) struct TextTree {
    pub nodes: Vec<(NodeId, Node)>,
    pub ranges: Vec<TextRange>,
    pub selection: Option<TextSelection>,
}

/// UTF-16 offsets are only used at the input boundary. AccessKit character
/// indices refer to scalar boundaries, which TextInput's selection API admits;
/// keyboard movement continues to use the editor's grapheme/word rules.
pub(crate) fn text_tree(
    view: &InputView,
    layout: Option<&LayoutSnapshot>,
    mut id: impl FnMut((u64, usize)) -> NodeId,
    mut bounds: impl FnMut(usize) -> Option<gpui::accesskit::Rect>,
) -> TextTree {
    let index = view.index();
    let text = index.text();
    let mut stops = BTreeSet::from([index.len_utf16()]);
    let mut offset = 0;
    let mut before_newline = BTreeSet::new();
    for ch in text.chars() {
        if ch == '\n' {
            before_newline.insert(offset);
            stops.insert(offset + 1);
        }
        offset += ch.len_utf16();
    }
    let mut lines = BTreeMap::new();
    if let Some(layout) = layout.filter(|layout| layout.lines().len() <= MAX_VISUAL_RUNS) {
        for (i, line) in layout.lines().iter().enumerate() {
            let at = |scalar| {
                index
                    .offset(Position {
                        story: view.selection().head.story,
                        paragraph: line.paragraph,
                        offset: ScalarOffset(scalar),
                        bias: Bias::After,
                    })
                    .ok()
            };
            if let (Some(start), Some(end)) =
                (at(line.source.scalars.start), at(line.source.scalars.end))
            {
                lines.entry(start).or_insert(i);
                // A hard newline belongs to the preceding line's last run.
                if end > start && !before_newline.contains(&end) {
                    stops.insert(end);
                }
            }
        }
    }
    let mut nodes = Vec::new();
    let mut ranges = Vec::new();
    let mut chars = text.char_indices().peekable();
    let (mut utf16, mut byte) = (0, 0);
    for end in stops {
        let (start, start_byte) = (utf16, byte);
        let mut lengths = Vec::new();
        while utf16 < end {
            let Some((at, ch)) = chars.next() else { break };
            lengths.push(ch.len_utf8() as u8);
            utf16 += ch.len_utf16();
            byte = at + ch.len_utf8();
        }
        let Ok(position) = index.position(start, Bias::After) else {
            continue;
        };
        let node_id = id((position.paragraph.get(), position.offset.0));
        let mut node = Node::new(Role::TextRun);
        node.set_value(&text[start_byte..byte]);
        node.set_character_lengths(lengths);
        if let Some(rect) = lines.get(&start).and_then(|line| bounds(*line)) {
            node.set_bounds(rect);
        }
        nodes.push((node_id, node));
        ranges.push(TextRange {
            node: node_id,
            utf16: start..utf16,
            bytes: start_byte..byte,
        });
    }
    let endpoint = |at: Position| {
        let offset = index.offset(at).ok()?;
        let run = ranges
            .iter()
            .find(|run| offset < run.utf16.end)
            .or_else(|| ranges.last())?;
        let character_index = text
            .get(run.bytes.clone())?
            .chars()
            .scan(run.utf16.start, |next, ch| {
                let start = *next;
                *next += ch.len_utf16();
                Some(start)
            })
            .take_while(|start| *start < offset)
            .count();
        Some(TextPosition {
            node: run.node,
            character_index,
        })
    };
    let selection = endpoint(view.selection().anchor)
        .zip(endpoint(view.selection().head))
        .map(|(anchor, focus)| TextSelection { anchor, focus });
    TextTree {
        nodes,
        ranges,
        selection,
    }
}

pub(crate) fn selection_offsets(
    ranges: &[TextRange],
    text: &str,
    selection: &TextSelection,
) -> Option<(usize, usize)> {
    let offset = |at: &TextPosition| {
        ranges
            .iter()
            .find(|run| run.node == at.node)
            .and_then(|run| run.offset(text, at.character_index))
    };
    Some((offset(&selection.anchor)?, offset(&selection.focus)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyword_engine::Session;
    use joyword_model::{BlockSeq, Document, Paragraph};
    use joyword_shell::input::{InputLimits, TextInput};

    #[test]
    fn native_text_nodes_keep_unicode_newlines_selection_direction_and_reject_bad_ranges()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = Document::new_blank();
        let mut blocks = Vec::new();
        for text in ["甲🙂e\u{301}", "乙𐐀"] {
            let mut p = Paragraph::new(doc.ids().node());
            p.push_text(text, None, Default::default())?;
            blocks.push(p.into());
        }
        let body = doc.body_id();
        doc.story_mut(body).ok_or("missing body")?.blocks = BlockSeq::from_vec(blocks);
        let mut input = TextInput::new(Session::new(doc), InputLimits::default())?;
        input.select(1..9, true)?;
        let mut next = 0;
        let tree = text_tree(
            input.view(),
            None,
            |_| {
                next += 1;
                NodeId(next)
            },
            |_| None,
        );
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].1.value(), Some("甲🙂e\u{301}\n"));
        assert_eq!(tree.nodes[0].1.character_lengths(), &[3, 4, 1, 2, 1]);
        assert_eq!(tree.nodes[1].1.value(), Some("乙𐐀"));
        let selection = tree.selection.ok_or("missing selection")?;
        assert_eq!(
            selection_offsets(&tree.ranges, input.view().index().text(), &selection),
            Some((9, 1))
        );
        let mut invalid = selection;
        invalid.focus.character_index = usize::MAX;
        assert_eq!(
            selection_offsets(&tree.ranges, input.view().index().text(), &invalid),
            None
        );
        invalid.focus.node = NodeId(999);
        invalid.focus.character_index = 0;
        assert_eq!(
            selection_offsets(&tree.ranges, input.view().index().text(), &invalid),
            None
        );
        Ok(())
    }

    #[test]
    fn empty_document_has_a_valid_native_insertion_point() -> Result<(), Box<dyn std::error::Error>>
    {
        let input = TextInput::new(Session::new(Document::new_blank()), InputLimits::default())?;
        let tree = text_tree(input.view(), None, |_| NodeId(1), |_| None);
        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(tree.nodes[0].1.value(), Some(""));
        let selection = tree.selection.ok_or("missing empty insertion point")?;
        assert_eq!(
            selection_offsets(&tree.ranges, "", &selection),
            Some((0, 0))
        );
        Ok(())
    }
}

//! Paragraph and run content, using the existing JoyDeck document representation.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::text::{BodyProps, Bullet, ParaProps, RunProps};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RunKind {
    Text,
    /// A line break inside the paragraph (Shift+Enter).
    Break,
    /// A field: `slidenum`, `datetime`, `datetime1`…`datetime13`, or other; `text` holds the last value.
    Field {
        field: String,
    },
    /// An inline equation (OMML kept as XML; `text` is its linear form).
    Math {
        omml: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "is_default_props")]
    pub props: RunProps,
    #[serde(default = "text_kind", skip_serializing_if = "is_text_kind")]
    pub kind: RunKind,
}

fn is_default_props(p: &RunProps) -> bool {
    *p == RunProps::default()
}
fn text_kind() -> RunKind {
    RunKind::Text
}
fn is_text_kind(k: &RunKind) -> bool {
    *k == RunKind::Text
}

impl Run {
    pub fn new(text: impl Into<String>) -> Self {
        Run {
            text: text.into(),
            props: RunProps::default(),
            kind: RunKind::Text,
        }
    }
    pub fn with(text: impl Into<String>, props: RunProps) -> Self {
        Run {
            text: text.into(),
            props,
            kind: RunKind::Text,
        }
    }
    /// The characters this run contributes to the paragraph (a break is one `\u{b}`).
    pub fn content(&self) -> &str {
        match self.kind {
            RunKind::Break => "\u{b}",
            _ => &self.text,
        }
    }
    pub fn char_len(&self) -> usize {
        self.content().chars().count()
    }

    /// Keep the prefix and return the remaining text with the same explicit run style.
    pub(crate) fn split_text_at(&mut self, offset: usize) -> Option<Self> {
        if self.kind != RunKind::Text {
            return None;
        }
        let (byte, _) = self.text.char_indices().nth(offset)?;
        Some(Self::with(self.text.split_off(byte), self.props.clone()))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Paragraph {
    /// Outline level 0–8.
    pub level: u8,
    #[serde(skip_serializing_if = "is_default_para")]
    pub props: ParaProps,
    pub runs: Vec<Run>,
    /// `a:defRPr`: defaults for runs in this paragraph that omit a property. Sits above the
    /// placeholder's list style, so a layout can still supply what this paragraph leaves unset.
    #[serde(skip_serializing_if = "is_default_props")]
    pub def_props: RunProps,
    /// Properties of the paragraph end mark (used for an empty paragraph's caret).
    #[serde(skip_serializing_if = "is_default_props")]
    pub end_props: RunProps,
}

fn is_default_para(p: &ParaProps) -> bool {
    *p == ParaProps::default()
}

impl Paragraph {
    pub fn new(text: &str) -> Self {
        Paragraph {
            runs: if text.is_empty() {
                vec![]
            } else {
                vec![Run::new(text)]
            },
            ..Default::default()
        }
    }
    /// Leading tabs are the outline level (at most 8), the same rule as a typed indent.
    /// Returns whether any tab was consumed.
    pub fn promote_leading_tabs(&mut self) -> bool {
        let Some(r) = self.runs.first_mut() else {
            return false;
        };
        if r.kind != RunKind::Text {
            return false;
        }
        let tabs = r.text.chars().take_while(|c| *c == '\t').count();
        if tabs == 0 {
            return false;
        }
        r.text = r.text.trim_start_matches('\t').to_string();
        self.level = tabs.min(8) as u8;
        true
    }
    pub fn text(&self) -> String {
        self.runs.iter().map(Run::content).collect()
    }
    pub fn char_len(&self) -> usize {
        self.runs.iter().map(Run::char_len).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.char_len() == 0
    }
    /// Merge adjacent text runs with identical properties.
    pub fn normalize(&mut self) {
        let runs = std::mem::take(&mut self.runs);
        self.runs.reserve(runs.len());
        for run in runs {
            if run.kind == RunKind::Text && run.text.is_empty() {
                continue;
            }
            match self.runs.last_mut() {
                Some(previous)
                    if previous.kind == RunKind::Text
                        && run.kind == RunKind::Text
                        && previous.props == run.props =>
                {
                    previous.text.push_str(&run.text);
                }
                _ => self.runs.push(run),
            }
        }
    }
}

/// Per-level paragraph + run defaults (`lstStyle`, master text styles, `defaultTextStyle`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LevelStyle {
    pub para: ParaProps,
    pub run: RunProps,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListStyle {
    /// Levels 1–9 (index 0–8); `None` = not specified.
    pub levels: Vec<Option<LevelStyle>>,
}

impl ListStyle {
    pub fn level(&self, l: u8) -> Option<&LevelStyle> {
        self.levels.get(l as usize).and_then(Option::as_ref)
    }
    pub fn set(&mut self, l: u8, s: LevelStyle) {
        let i = (l as usize).min(8);
        if self.levels.len() <= i {
            self.levels.resize(i + 1, None);
        }
        if let Some(slot) = self.levels.get_mut(i) {
            *slot = Some(s);
        }
    }
    pub fn is_empty(&self) -> bool {
        self.levels.iter().all(Option::is_none)
    }
}

/// Per-level numbering for a text body, including empty paragraph previews and restarts.
#[derive(Default)]
pub struct ListNumbering {
    levels: [ListSequence; 9],
}

#[derive(Clone, Copy, Default)]
struct ListSequence {
    start_at: u32,
    used: u32,
}

impl ListNumbering {
    /// Returns the one-based ordinal before applying the numbering scheme and `startAt`.
    pub fn advance(&mut self, level: u8, bullet: Option<&Bullet>, empty: bool) -> Option<u32> {
        let level = usize::from(level.min(8));
        if let Some(Bullet::AutoNum { start_at, .. }) = bullet {
            let current = &mut self.levels[level];
            let ordinal = if current.start_at == *start_at {
                current.used.saturating_add(1)
            } else {
                1
            };
            if !empty {
                *current = ListSequence {
                    start_at: *start_at,
                    used: ordinal,
                };
                self.clear_from(level + 1);
            }
            Some(ordinal)
        } else {
            if !empty {
                self.clear_from(level);
            }
            None
        }
    }

    fn clear_from(&mut self, index: usize) {
        for level in &mut self.levels[index..] {
            level.used = 0;
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextBody {
    pub body: BodyProps,
    #[serde(skip_serializing_if = "ListStyle::is_empty")]
    pub list_style: ListStyle,
    pub paragraphs: Vec<Paragraph>,
}

impl TextBody {
    pub fn from_text(text: &str) -> Self {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        TextBody {
            paragraphs: text.split('\n').map(Paragraph::new).collect(),
            ..Default::default()
        }
    }
    /// Plain text with `\n` between paragraphs.
    pub fn text(&self) -> String {
        let mut result = String::with_capacity(self.byte_len());
        for (i, paragraph) in self.paragraphs.iter().enumerate() {
            if i != 0 {
                result.push('\n');
            }
            for run in &paragraph.runs {
                result.push_str(run.content());
            }
        }
        result
    }

    pub fn byte_len(&self) -> usize {
        self.paragraphs
            .iter()
            .flat_map(|p| &p.runs)
            .map(|r| r.content().len())
            .sum::<usize>()
            + self.paragraphs.len().saturating_sub(1)
    }

    /// Byte ranges in the same coordinate stream as `text`, including paragraph separators.
    pub fn run_spans(&self) -> impl Iterator<Item = (Range<usize>, &Run)> {
        self.paragraphs
            .iter()
            .enumerate()
            .flat_map(|(index, paragraph)| paragraph.runs.iter().map(move |run| (index, run)))
            .scan((0, 0), |(last_index, previous_end), (index, run)| {
                let start = *previous_end + (index - *last_index);
                let span = start..start + run.content().len();
                *last_index = index;
                *previous_end = span.end;
                Some((span, run))
            })
    }
    pub fn is_empty(&self) -> bool {
        self.paragraphs.iter().all(Paragraph::is_empty)
    }
    /// Total characters, counting one separator between paragraphs.
    pub fn char_len(&self) -> usize {
        let n: usize = self.paragraphs.iter().map(Paragraph::char_len).sum();
        n + self.paragraphs.len().saturating_sub(1)
    }
}

//! Structured loss reports for bidirectional text/markdown/html conversions (FORMAT-006).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossKind {
    FormattingDropped,
    TableFlattened,
    ImageOmitted,
    ActiveContentStripped,
    UnsupportedElement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossItem {
    pub kind: LossKind,
    pub description: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LossReport {
    pub items: Vec<LossItem>,
}

impl LossReport {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn record(&mut self, kind: LossKind, description: impl Into<String>) {
        self.items.push(LossItem {
            kind,
            description: description.into(),
        });
    }

    pub fn has_loss(&self) -> bool {
        !self.items.is_empty()
    }
}

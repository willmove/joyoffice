//! Immutable show snapshot. Editing continues in the source document independently.
use crate::document::{Deck, Result};
use std::sync::Arc;
#[derive(Clone)]
pub struct ShowSnapshot {
    pub deck: Arc<Deck>,
    pub pages: Vec<usize>,
    pub cursor: usize,
}
impl ShowSnapshot {
    pub fn new(deck: Arc<Deck>, start: usize) -> Result<Self> {
        if start >= deck.slides.len() {
            return Err("放映起始页不存在".into());
        }
        deck.validate()?;
        let mut pages: Vec<_> = deck
            .slides
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.hidden)
            .map(|(i, _)| i)
            .collect();
        if !deck.presentation.order.is_empty() {
            pages = deck
                .presentation
                .order
                .iter()
                .filter_map(|id| deck.slides.iter().position(|s| &s.id == id && !s.hidden))
                .collect();
        }
        let cursor = pages
            .iter()
            .position(|p| *p == start)
            .or_else(|| pages.iter().position(|p| *p >= start))
            .ok_or("起始页之后没有可见页面")?;
        Ok(Self {
            deck,
            pages,
            cursor,
        })
    }
    pub fn page(&self) -> usize {
        self.pages[self.cursor]
    }
    pub fn next_page(&mut self) {
        self.cursor = (self.cursor + 1).min(self.pages.len() - 1);
    }
    pub fn previous(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }
    pub fn first(&mut self) {
        self.cursor = 0;
    }
    pub fn last(&mut self) {
        self.cursor = self.pages.len() - 1;
    }
}

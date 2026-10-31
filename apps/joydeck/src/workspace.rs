//! A document has one writer/history. Views keep their own selection and viewport.
use crate::{
    document::{Deck, Result},
    edit::{Command, Session},
};
use std::sync::{Arc, Mutex};
pub struct SaveLease(Arc<Mutex<bool>>);
impl Drop for SaveLease {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = false;
    }
}
#[derive(Clone)]
pub struct SharedSession {
    core: Arc<Mutex<Session>>,
    pub id: u64,
    pub deck: Arc<Deck>,
    pub revision: u64,
    dirty_seen: bool,
    saving: Arc<Mutex<bool>>,
    file: Arc<Mutex<Option<(std::path::PathBuf, crate::storage::Fingerprint)>>>,
}
impl Default for SharedSession {
    fn default() -> Self {
        Self::new()
    }
}
impl SharedSession {
    pub fn new() -> Self {
        Self::from_session(Session::new())
    }
    pub fn from_deck(deck: Deck, clean: bool) -> Self {
        Self::from_session(Session::from_deck(deck, clean))
    }
    fn from_session(s: Session) -> Self {
        Self {
            id: s.id,
            deck: s.deck.clone(),
            revision: s.revision,
            dirty_seen: s.dirty(),
            core: Arc::new(Mutex::new(s)),
            saving: Arc::new(Mutex::new(false)),
            file: Arc::new(Mutex::new(None)),
        }
    }
    pub fn sync(&mut self) -> bool {
        let s = self.core.lock().unwrap();
        let changed = self.revision != s.revision || self.dirty_seen != s.dirty();
        self.deck = s.deck.clone();
        self.revision = s.revision;
        self.dirty_seen = s.dirty();
        changed
    }
    pub fn try_save(&self) -> Option<SaveLease> {
        if self.begin_save() {
            Some(SaveLease(self.saving.clone()))
        } else {
            None
        }
    }
    pub fn begin_save(&self) -> bool {
        let mut busy = self.saving.lock().unwrap();
        if *busy {
            false
        } else {
            *busy = true;
            true
        }
    }
    pub fn end_save(&self) {
        *self.saving.lock().unwrap() = false;
    }
    pub fn is_saving(&self) -> bool {
        *self.saving.lock().unwrap()
    }
    pub fn file(&self) -> Option<(std::path::PathBuf, crate::storage::Fingerprint)> {
        self.file.lock().unwrap().clone()
    }
    pub fn set_file(&self, file: Option<(std::path::PathBuf, crate::storage::Fingerprint)>) {
        *self.file.lock().unwrap() = file;
    }
    pub fn dirty(&self) -> bool {
        self.core.lock().unwrap().dirty()
    }
    pub fn can_undo(&self) -> bool {
        self.core.lock().unwrap().can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.core.lock().unwrap().can_redo()
    }
    pub fn enabled(&self, c: &Command) -> Result<()> {
        self.core.lock().unwrap().enabled(c)
    }
    pub fn execute(&mut self, id: u64, c: Command) -> Result<()> {
        {
            let mut s = self.core.lock().unwrap();
            if self.revision != s.revision {
                return Err("其他窗口已修改文稿，请同步后重试".into());
            }
            s.execute(id, c)?;
        }
        self.sync();
        Ok(())
    }
    pub fn undo(&mut self) {
        self.core.lock().unwrap().undo();
        self.sync();
    }
    pub fn redo(&mut self) {
        self.core.lock().unwrap().redo();
        self.sync();
    }
    pub fn mark_saved(&mut self, snapshot: Arc<Deck>) {
        self.core.lock().unwrap().mark_saved(snapshot);
        self.sync();
    }
}

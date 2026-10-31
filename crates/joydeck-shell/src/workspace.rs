//! A document has one writer/history. Views keep their own selection and viewport.
use crate::{
    document::{Deck, Result},
    edit::{Command, Session},
};
use std::sync::{Arc, Mutex};
pub struct SaveLease(Arc<Mutex<bool>>);
impl Drop for SaveLease {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.0.lock() {
            *busy = false;
        }
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
    /// Poisoned session state is never recovered as a success. Readers retain
    /// their last snapshot; editing/saving is disabled and dirty stays true.
    pub fn failure(&self) -> Option<&'static str> {
        (self.core.is_poisoned() || self.saving.is_poisoned() || self.file.is_poisoned())
            .then_some("文稿会话已损坏，请保留恢复副本后重新打开")
    }
    fn healthy(&self) -> Result<()> {
        self.failure().map_or(Ok(()), |e| Err(e.into()))
    }
    pub fn sync(&mut self) -> bool {
        if self.healthy().is_err() {
            return false;
        }
        let Ok(s) = self.core.lock() else {
            return false;
        };
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
        if self.healthy().is_err() {
            return false;
        }
        let Ok(mut busy) = self.saving.lock() else {
            return false;
        };
        if *busy {
            false
        } else {
            *busy = true;
            true
        }
    }
    pub fn end_save(&self) {
        if let Ok(mut busy) = self.saving.lock() {
            *busy = false;
        }
    }
    pub fn is_saving(&self) -> bool {
        self.saving.lock().map_or(true, |busy| *busy)
    }
    pub fn file(&self) -> Option<(std::path::PathBuf, crate::storage::Fingerprint)> {
        self.file.lock().ok().and_then(|file| file.clone())
    }
    pub fn set_file(&self, file: Option<(std::path::PathBuf, crate::storage::Fingerprint)>) {
        if let Ok(mut target) = self.file.lock() {
            *target = file;
        }
    }
    pub fn dirty(&self) -> bool {
        self.failure().is_some() || self.core.lock().map_or(true, |s| s.dirty())
    }
    pub fn can_undo(&self) -> bool {
        self.healthy().is_ok() && self.core.lock().is_ok_and(|s| s.can_undo())
    }
    pub fn can_redo(&self) -> bool {
        self.healthy().is_ok() && self.core.lock().is_ok_and(|s| s.can_redo())
    }
    pub fn enabled(&self, c: &Command) -> Result<()> {
        self.healthy()?;
        let s = self.core.lock().map_err(|_| "文稿会话锁已损坏")?;
        if self.revision != s.revision {
            return Err("其他窗口已修改文稿，请同步后重试".into());
        }
        s.enabled(c)
    }
    pub fn execute(&mut self, id: u64, c: Command) -> Result<()> {
        self.healthy()?;
        {
            let mut s = self.core.lock().map_err(|_| "文稿会话锁已损坏")?;
            if self.revision != s.revision {
                return Err("其他窗口已修改文稿，请同步后重试".into());
            }
            s.execute(id, c)?;
        }
        self.sync();
        Ok(())
    }
    pub fn undo(&mut self) {
        if self.healthy().is_err() {
            return;
        }
        if let Ok(mut s) = self.core.lock() {
            s.undo();
        }
        self.sync();
    }
    pub fn redo(&mut self) {
        if self.healthy().is_err() {
            return;
        }
        if let Ok(mut s) = self.core.lock() {
            s.redo();
        }
        self.sync();
    }
    pub fn mark_saved(&mut self, snapshot: Arc<Deck>) {
        if self.healthy().is_err() {
            return;
        }
        if let Ok(mut s) = self.core.lock() {
            s.mark_saved(snapshot);
        }
        self.sync();
    }
}

#[cfg(test)]
#[path = "workspace/tests.rs"]
mod tests;

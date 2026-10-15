//! Logical paragraph ordinal + Unicode scalar positions, never persisted IDs.
//! Every event binds both versions and complete source/text pre/post hashes.
use crate::{
    Error,
    journal::{AppendLog, DurableWrite, Record},
};
use joyoffice_opc::Budget;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_ACTIONS: usize = 128;
pub const MAX_EVENTS: usize = 4095;
pub const MAX_TEXT: usize = 4 << 20;
pub const MAX_PARAGRAPHS: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub paragraph: usize,
    pub scalar: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Insert {
        at: Point,
        text: String,
    },
    Delete {
        start: Point,
        end: Point,
    },
    Split(Point),
    Undo,
    Redo,
    /// A receipt for an already saved version; it is not a file-save command.
    Saved(u64),
}
#[derive(Clone, Debug)]
pub struct Event {
    pub before: u64,
    pub after: u64,
    pub prior: [u8; 32],
    pub next: [u8; 32],
    pub actions: Vec<Action>,
}
pub struct Recovery {
    original: Vec<u8>,
    pub session: [u8; 16],
    events: Vec<Vec<u8>>,
    docx: DocxSession,
    saved: u64,
    closed: bool,
}
pub fn budget() -> Budget {
    Budget {
        max_entries: 512,
        max_part_bytes: 8 << 20,
        max_total_bytes: 16 << 20,
        max_ratio: 100,
        ratio_floor_bytes: 4096,
        max_xml_depth: 64,
        max_xml_nodes: 100_000,
        max_relationships: 1024,
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn validate(docx: &DocxSession) -> Result<(), Error> {
    let doc = docx.session().document();
    if doc.stories().count() != 1 || doc.body().blocks.len() > MAX_PARAGRAPHS {
        return Err(Error::Limit("body paragraphs/stories"));
    }
    let mut bytes = 0;
    for block in doc.body().blocks.iter() {
        let p = block
            .as_paragraph()
            .ok_or(Error::Invalid("non-text body"))?;
        if !p.inlines().is_empty()
            || p.style.is_some()
            || !p.props.is_empty()
            || !p.mark_props.is_empty()
            || p.runs()
                .iter()
                .any(|r| r.style.is_some() || !r.props.is_empty())
        {
            return Err(Error::Invalid(
                "only default-property plain text is recoverable",
            ));
        }
        let size = p.text().summary().bytes;
        if size > 65_536 {
            return Err(Error::Limit("paragraph bytes"));
        }
        bytes += size;
        if bytes > MAX_TEXT {
            return Err(Error::Limit("text bytes"));
        }
    }
    Ok(())
}
impl Recovery {
    pub fn open(original: &[u8], session: [u8; 16]) -> Result<Self, Error> {
        if original.len() > (8 << 20) - 32 {
            return Err(Error::Limit("baseline bytes"));
        }
        let docx =
            DocxSession::open(original, &budget()).map_err(|e| Error::Engine(e.to_string()))?;
        validate(&docx)?;
        Ok(Self {
            original: original.to_vec(),
            session,
            events: Vec::new(),
            docx,
            saved: 0,
            closed: false,
        })
    }
    pub fn docx(&self) -> &DocxSession {
        &self.docx
    }
    pub fn version(&self) -> u64 {
        self.docx.session().version().get()
    }
    pub fn saved_version(&self) -> u64 {
        self.saved
    }
    pub fn dirty(&self) -> bool {
        self.version() != self.saved
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    pub fn texts(&self) -> Vec<String> {
        self.docx
            .session()
            .document()
            .body()
            .blocks
            .iter()
            .map(|b| b.as_paragraph().unwrap().plain_text(|_| unreachable!()))
            .collect()
    }
    fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(hash(&self.original));
        h.update(self.version().to_le_bytes());
        h.update(self.saved.to_le_bytes());
        for text in self.texts() {
            h.update((text.len() as u64).to_le_bytes());
            h.update(text.as_bytes());
        }
        h.finalize().into()
    }
    pub fn baseline_payload(&self) -> Vec<u8> {
        let mut bytes = hash(&self.original).to_vec();
        bytes.extend_from_slice(&self.original);
        bytes
    }
    fn actions(&mut self, actions: &[Action]) -> Result<(), Error> {
        if actions.is_empty() || actions.len() > MAX_ACTIONS {
            return Err(Error::Limit("empty/excessive actions"));
        }
        if actions
            .iter()
            .any(|a| matches!(a, Action::Undo | Action::Redo | Action::Saved(_)))
        {
            if actions.len() != 1 {
                return Err(Error::Invalid("history/receipt must be a separate event"));
            }
            match actions[0] {
                Action::Undo => {
                    self.docx
                        .undo()
                        .map_err(|e| Error::Engine(format!("{e:?}")))?;
                }
                Action::Redo => {
                    self.docx
                        .redo()
                        .map_err(|e| Error::Engine(format!("{e:?}")))?;
                }
                Action::Saved(version) => {
                    if version > self.version() || version < self.saved {
                        return Err(Error::Invalid("stale/future save receipt"));
                    }
                    self.saved = version;
                }
                _ => unreachable!(),
            }
        } else {
            // Logical positions within one transaction are resolved against a
            // temporary engine using earlier commands, then submitted once.
            let mut probe = joyword_engine::Session::new(self.docx.session().document().clone());
            let mut commands = Vec::new();
            for action in actions {
                let locate = |at: Point| -> Result<Position, Error> {
                    let doc = probe.document();
                    let p = doc
                        .body()
                        .blocks
                        .get(at.paragraph)
                        .and_then(|b| b.as_paragraph())
                        .ok_or(Error::Invalid("paragraph position"))?;
                    if at.scalar > p.len() {
                        return Err(Error::Invalid("scalar position"));
                    }
                    Ok(Position {
                        story: doc.body_id(),
                        paragraph: p.id(),
                        offset: ScalarOffset(at.scalar),
                        bias: Bias::After,
                    })
                };
                let command = match action {
                    Action::Insert { at, text } => {
                        if text.len() > 65_536 {
                            return Err(Error::Limit("insert bytes"));
                        }
                        Command::InsertText {
                            at: locate(*at)?,
                            text: text.clone(),
                        }
                    }
                    Action::Delete { start, end } => Command::DeleteRange {
                        start: locate(*start)?,
                        end: locate(*end)?,
                    },
                    Action::Split(at) => Command::SplitParagraph { at: locate(*at)? },
                    _ => unreachable!(),
                };
                probe
                    .apply(Transaction::new(probe.version(), vec![command.clone()]))
                    .map_err(|e| Error::Engine(e.to_string()))?;
                commands.push(command);
            }
            self.docx
                .apply(Transaction::new(self.docx.session().version(), commands))
                .map_err(|e| Error::Engine(e.to_string()))?;
        }
        validate(&self.docx)
    }
    /// Replay to a candidate, durably append, then publish it. Failed writes
    /// never publish the candidate. The log is poisoned on uncertain IO.
    /// Overlay/preedit has no API here: callers submit only committed actions.
    pub fn commit<W: DurableWrite>(
        &mut self,
        log: &mut AppendLog<W>,
        base: u64,
        actions: Vec<Action>,
        cancel: &AtomicBool,
    ) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Invalid("closed recovery session"));
        }
        if self.session != log.session()
            || self.version() != base
            || log.records_written() != (self.events.len() + 1) as u64
            || log.baseline_source() != Some(source_hash(&self.original))
        {
            return Err(Error::Invalid("source/session/version mismatch"));
        }
        if self.events.len() >= MAX_EVENTS {
            return Err(Error::Limit("events"));
        }
        let mut candidate = Self::open(&self.original, self.session)?;
        for payload in &self.events {
            check_cancel(cancel)?;
            candidate.replay_payload(payload)?;
        }
        check_cancel(cancel)?;
        let prior = candidate.digest();
        candidate.actions(&actions)?;
        let event = Event {
            before: self.version(),
            after: candidate.version(),
            prior,
            next: candidate.digest(),
            actions,
        };
        let payload = encode(&event)?;
        check_cancel(cancel)?;
        log.append(&payload)?;
        candidate.events = self.events.clone();
        candidate.events.push(payload);
        *self = candidate;
        Ok(())
    }
    fn replay_payload(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let event = decode(bytes)?;
        if event.before != self.version() || event.prior != self.digest() {
            return Err(Error::Invalid("event precondition/source/version"));
        }
        self.actions(&event.actions)?;
        if event.after != self.version() || event.next != self.digest() {
            return Err(Error::Invalid("event postcondition/version"));
        }
        Ok(())
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), Error> {
    if cancel.load(Ordering::Acquire) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
pub fn recover(
    records: &[Record],
    expected_session: [u8; 16],
    expected_source: [u8; 32],
    cancel: &AtomicBool,
) -> Result<Recovery, Error> {
    let first = records.first().ok_or(Error::Invalid("missing baseline"))?;
    if records.len() > MAX_EVENTS + 1
        || first.kind != 1
        || first.sequence != 0
        || first.payload.len() < 32
    {
        return Err(Error::Invalid("baseline envelope"));
    }
    let source = &first.payload[32..];
    if hash(source) != expected_source
        || first.payload[..32] != expected_source
        || first.session != expected_session
    {
        return Err(Error::Invalid("wrong baseline source/session"));
    }
    check_cancel(cancel)?;
    let mut result = Recovery::open(source, expected_session)?;
    for (index, record) in records.iter().enumerate().skip(1) {
        check_cancel(cancel)?;
        if record.kind != 2 || record.sequence != index as u64 || record.session != expected_session
        {
            return Err(Error::Invalid("event order/session"));
        }
        result.replay_payload(&record.payload)?;
        result.events.push(record.payload.clone());
    }
    Ok(result)
}
pub fn source_hash(bytes: &[u8]) -> [u8; 32] {
    hash(bytes)
}

pub fn encode(event: &Event) -> Result<Vec<u8>, Error> {
    if event.actions.is_empty() || event.actions.len() > MAX_ACTIONS {
        return Err(Error::Limit("actions"));
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&event.before.to_le_bytes());
    bytes.extend_from_slice(&event.after.to_le_bytes());
    bytes.extend_from_slice(&event.prior);
    bytes.extend_from_slice(&event.next);
    bytes.extend_from_slice(&(event.actions.len() as u32).to_le_bytes());
    let point = |bytes: &mut Vec<u8>, at: Point| -> Result<(), Error> {
        for n in [at.paragraph, at.scalar] {
            bytes.extend_from_slice(
                &u32::try_from(n)
                    .map_err(|_| Error::Limit("point"))?
                    .to_le_bytes(),
            );
        }
        Ok(())
    };
    for action in &event.actions {
        match action {
            Action::Insert { at, text } => {
                if text.len() > 65_536 {
                    return Err(Error::Limit("insert bytes"));
                }
                bytes.push(1);
                point(&mut bytes, *at)?;
                bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
            }
            Action::Delete { start, end } => {
                bytes.push(2);
                point(&mut bytes, *start)?;
                point(&mut bytes, *end)?;
            }
            Action::Split(at) => {
                bytes.push(3);
                point(&mut bytes, *at)?;
            }
            Action::Undo => bytes.push(4),
            Action::Redo => bytes.push(5),
            Action::Saved(v) => {
                bytes.push(6);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    if bytes.len() > 8 << 20 {
        return Err(Error::Limit("event bytes"));
    }
    Ok(bytes)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(Error::Invalid("truncated event"))?;
        let part = &self.bytes[self.at..end];
        self.at = end;
        Ok(part)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn point(&mut self) -> Result<Point, Error> {
        Ok(Point {
            paragraph: self.u32()? as usize,
            scalar: self.u32()? as usize,
        })
    }
}
pub fn decode(bytes: &[u8]) -> Result<Event, Error> {
    if bytes.len() > 8 << 20 {
        return Err(Error::Limit("event bytes"));
    }
    let mut c = Cursor { bytes, at: 0 };
    let before = c.u64()?;
    let after = c.u64()?;
    let prior = c.take(32)?.try_into().unwrap();
    let next = c.take(32)?.try_into().unwrap();
    let count = c.u32()? as usize;
    if count == 0 || count > MAX_ACTIONS {
        return Err(Error::Limit("actions"));
    }
    let mut actions = Vec::new();
    for _ in 0..count {
        actions.push(match c.take(1)?[0] {
            1 => {
                let at = c.point()?;
                let n = c.u32()? as usize;
                if n > 65_536 {
                    return Err(Error::Limit("insert bytes"));
                }
                Action::Insert {
                    at,
                    text: std::str::from_utf8(c.take(n)?)
                        .map_err(|_| Error::Invalid("event UTF8"))?
                        .into(),
                }
            }
            2 => Action::Delete {
                start: c.point()?,
                end: c.point()?,
            },
            3 => Action::Split(c.point()?),
            4 => Action::Undo,
            5 => Action::Redo,
            6 => Action::Saved(c.u64()?),
            _ => return Err(Error::Invalid("unsupported action")),
        });
    }
    if c.at != bytes.len() {
        return Err(Error::Invalid("trailing event bytes"));
    }
    Ok(Event {
        before,
        after,
        prior,
        next,
        actions,
    })
}

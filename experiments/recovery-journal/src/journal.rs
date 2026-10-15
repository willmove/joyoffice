//! Version 1, little endian. Every record has a 44-byte header:
//! magic[8], schema u16, kind u8, reserved u8, sequence u64, session[16],
//! payload length u32, CRC32 u32 over header[0..40] plus payload.
//! A torn/corrupt record ends the prefix; no resynchronization is attempted.
use crate::{Error, io as at};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

pub const HEADER: usize = 44;
pub const MAGIC: &[u8; 8] = b"JWJRNL01";
pub const SCHEMA: u16 = 1;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub record_bytes: usize,
    pub file_bytes: usize,
    pub records: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            record_bytes: 8 << 20,
            file_bytes: 32 << 20,
            records: 4096,
        }
    }
}
impl Limits {
    pub fn capped(self) -> Self {
        let h = Self::default();
        Self {
            record_bytes: self.record_bytes.min(h.record_bytes),
            file_bytes: self.file_bytes.min(h.file_bytes),
            records: self.records.min(h.records),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub kind: u8,
    pub sequence: u64,
    pub session: [u8; 16],
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tail {
    Truncated,
    Magic,
    Length,
    Checksum,
    Order,
    Session,
    Kind,
}
#[derive(Debug)]
pub struct Scan {
    pub records: Vec<Record>,
    pub valid_bytes: usize,
    pub tail: Option<(usize, Tail)>,
}

pub fn scan(bytes: &[u8], limits: Limits) -> Result<Scan, Error> {
    let limits = limits.capped();
    if bytes.len() > limits.file_bytes {
        return Err(Error::Limit("file bytes"));
    }
    let mut result = Scan {
        records: Vec::new(),
        valid_bytes: 0,
        tail: None,
    };
    let mut offset = 0;
    while offset < bytes.len() {
        if result.records.len() >= limits.records {
            return Err(Error::Limit("records"));
        }
        let rest = &bytes[offset..];
        let stop = if rest.len() < HEADER {
            Some(Tail::Truncated)
        } else {
            None
        };
        if let Some(reason) = stop {
            result.tail = Some((offset, reason));
            break;
        }
        if &rest[..8] != MAGIC {
            result.tail = Some((offset, Tail::Magic));
            break;
        }
        let schema = u16::from_le_bytes(rest[8..10].try_into().unwrap());
        if schema != SCHEMA {
            return Err(Error::Schema(schema));
        }
        let length = u32::from_le_bytes(rest[36..40].try_into().unwrap()) as usize;
        let end = HEADER
            .checked_add(length)
            .ok_or(Error::Limit("length overflow"))?;
        let sequence = u64::from_le_bytes(rest[12..20].try_into().unwrap());
        let session: [u8; 16] = rest[20..36].try_into().unwrap();
        let crc = u32::from_le_bytes(rest[40..44].try_into().unwrap());
        let reason = if length > limits.record_bytes {
            Some(Tail::Length)
        } else if end > rest.len() {
            Some(Tail::Truncated)
        } else if sequence != result.records.len() as u64 {
            Some(Tail::Order)
        } else if result.records.first().is_some_and(|r| r.session != session) {
            Some(Tail::Session)
        } else if rest[11] != 0 || rest[10] != if sequence == 0 { 1 } else { 2 } {
            Some(Tail::Kind)
        } else {
            let mut hash = crc32fast::Hasher::new();
            hash.update(&rest[..40]);
            hash.update(&rest[HEADER..end]);
            (hash.finalize() != crc).then_some(Tail::Checksum)
        };
        if let Some(reason) = reason {
            result.tail = Some((offset, reason));
            break;
        }
        result.records.push(Record {
            kind: rest[10],
            sequence,
            session,
            payload: rest[HEADER..end].to_vec(),
        });
        offset += end;
        result.valid_bytes = offset;
    }
    Ok(result)
}
pub fn read(path: &Path, limits: Limits) -> Result<Vec<u8>, Error> {
    let max = limits.capped().file_bytes;
    let file = File::open(path).map_err(|e| at("open log", e))?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| at("read log", e))?;
    if bytes.len() > max {
        return Err(Error::Limit("file bytes"));
    }
    Ok(bytes)
}

pub trait DurableWrite: Write {
    fn sync(&mut self) -> io::Result<()>;
}
impl DurableWrite for File {
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}
/// A fresh, exclusively owned sink. On any write/flush/sync error, poison it;
/// a record may already be partly or fully visible, so retry is unsafe.
pub struct AppendLog<W> {
    sink: W,
    session: [u8; 16],
    next: u64,
    bytes: usize,
    limits: Limits,
    poisoned: bool,
    baseline_source: Option<[u8; 32]>,
}
impl AppendLog<File> {
    pub fn create(path: &Path, session: [u8; 16], limits: Limits) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| at("create log", e))?;
        Ok(Self::fresh(file, session, limits))
    }
}
impl<W: DurableWrite> AppendLog<W> {
    pub fn fresh(sink: W, session: [u8; 16], limits: Limits) -> Self {
        Self {
            sink,
            session,
            next: 0,
            bytes: 0,
            limits: limits.capped(),
            poisoned: false,
            baseline_source: None,
        }
    }
    pub fn append(&mut self, payload: &[u8]) -> Result<(), Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        if payload.len() > self.limits.record_bytes {
            return Err(Error::Limit("record bytes"));
        }
        if self.next >= self.limits.records as u64 {
            return Err(Error::Limit("records"));
        }
        let total = self
            .bytes
            .checked_add(HEADER)
            .and_then(|n| n.checked_add(payload.len()))
            .ok_or(Error::Limit("file bytes"))?;
        if total > self.limits.file_bytes {
            return Err(Error::Limit("file bytes"));
        }
        let mut header = Vec::with_capacity(HEADER);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&SCHEMA.to_le_bytes());
        header.extend_from_slice(&[if self.next == 0 { 1 } else { 2 }, 0]);
        header.extend_from_slice(&self.next.to_le_bytes());
        header.extend_from_slice(&self.session);
        header.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        let mut hash = crc32fast::Hasher::new();
        hash.update(&header);
        hash.update(payload);
        header.extend_from_slice(&hash.finalize().to_le_bytes());
        let write = self
            .sink
            .write_all(&header)
            .and_then(|()| self.sink.write_all(payload));
        let written = write
            .map_err(|e| at("append", e))
            .and_then(|()| self.sink.flush().map_err(|e| at("flush", e)))
            .and_then(|()| self.sink.sync().map_err(|e| at("sync", e)));
        if let Err(error) = written {
            self.poisoned = true;
            return Err(error);
        }
        if self.next == 0 && payload.len() >= 32 {
            let source: [u8; 32] = Sha256::digest(&payload[32..]).into();
            if payload[..32] == source {
                self.baseline_source = Some(source);
            }
        }
        self.next += 1;
        self.bytes = total;
        Ok(())
    }
    pub fn into_inner(self) -> W {
        self.sink
    }
    pub fn records_written(&self) -> u64 {
        self.next
    }
    pub fn baseline_source(&self) -> Option<[u8; 32]> {
        self.baseline_source
    }
    pub fn session(&self) -> [u8; 16] {
        self.session
    }
}

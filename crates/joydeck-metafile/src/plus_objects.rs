//! Reassemble bounded EmfPlusObject payloads across GDI comments.

use std::borrow::Cow;

use crate::bytes::Bytes;

const MAX_OBJECT_BYTES: usize = 64 << 20;

pub(crate) struct Object<'a> {
    pub slot: usize,
    pub kind: u16,
    pub payload: Cow<'a, [u8]>,
}

struct Pieces {
    slot: usize,
    kind: u16,
    size: usize,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub(crate) struct Assembly(Option<Pieces>);

impl Assembly {
    pub fn push<'a>(&mut self, flags: u16, record: Bytes<'a>) -> Option<Object<'a>> {
        let slot = usize::from(flags & 0xff);
        let kind = (flags >> 8) & 0x7f;
        if slot >= 64 {
            return None;
        }
        let previous = self
            .0
            .take()
            .filter(|part| part.slot == slot && part.kind == kind);
        let (size, segment, mut bytes) = if flags & 0x8000 != 0 {
            let size = record.u32(0)? as usize;
            if size > MAX_OBJECT_BYTES {
                return None;
            }
            let data = previous
                .filter(|part| part.size == size)
                .map_or_else(Vec::new, |part| part.bytes);
            (size, record.tail(4)?.0, data)
        } else if let Some(part) = previous {
            (part.size, record.0, part.bytes)
        } else {
            return (record.0.len() <= MAX_OBJECT_BYTES).then_some(Object {
                slot,
                kind,
                payload: Cow::Borrowed(record.0),
            });
        };
        bytes.extend_from_slice(&segment[..segment.len().min(size.saturating_sub(bytes.len()))]);
        if bytes.len() == size {
            return Some(Object {
                slot,
                kind,
                payload: Cow::Owned(bytes),
            });
        }
        if flags & 0x8000 != 0 {
            self.0 = Some(Pieces {
                slot,
                kind,
                size,
                bytes,
            });
        }
        None
    }
}

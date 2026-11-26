//! Wire framing for [MS-WMF], [MS-EMF], and [MS-EMFPLUS]. Operands stay inside their record.

use crate::bytes::Bytes;

#[derive(Clone, Copy)]
pub(crate) enum Format {
    Wmf,
    Emf,
    EmfPlus,
}

pub(crate) struct Record<'a> {
    pub kind: u32,
    pub flags: u16,
    pub data: Bytes<'a>,
}

pub(crate) fn read<'a>(
    input: Bytes<'a>,
    offset: usize,
    format: Format,
) -> impl Iterator<Item = Record<'a>> {
    let mut remaining = input.0.get(offset..).unwrap_or_default();
    std::iter::from_fn(move || {
        let bytes = Bytes(remaining);
        let (kind, flags, size) = match format {
            Format::Wmf => (
                u32::from(bytes.u16(4)?),
                0,
                usize::try_from(bytes.u32(0)?).ok()?.checked_mul(2)?,
            ),
            Format::Emf => (bytes.u32(0)?, 0, usize::try_from(bytes.u32(4)?).ok()?),
            Format::EmfPlus => (
                u32::from(bytes.u16(0)?),
                bytes.u16(2)?,
                usize::try_from(bytes.u32(4)?).ok()?,
            ),
        };
        let data = match format {
            Format::Wmf if size >= 6 => bytes.slice(0, size)?,
            Format::Emf if size >= 8 && size.is_multiple_of(4) => bytes.slice(0, size)?,
            Format::EmfPlus if size >= 12 => {
                // The last comment record can be missing only its trailing alignment bytes.
                let body = usize::try_from(bytes.u32(8)?).ok()?;
                if body > size - 12 {
                    return None;
                }
                bytes.slice(12, body)?
            }
            _ => return None,
        };
        remaining = remaining.get(size..).unwrap_or_default();
        Some(Record {
            kind,
            flags,
            data: Bytes(data),
        })
    })
    .fuse()
}

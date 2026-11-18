use std::io::{self, Read};

/// Output consumed from a decoded ZIP member, including bytes produced before a CRC or I/O error.
pub enum MemberRead {
    Complete(Vec<u8>),
    Limit(u64),
    Failed { produced: u64, cause: io::Error },
}

impl MemberRead {
    pub fn produced(&self) -> u64 {
        match self {
            Self::Complete(bytes) => bytes.len() as u64,
            Self::Limit(bytes)
            | Self::Failed {
                produced: bytes, ..
            } => *bytes,
        }
    }
}

/// Read at most one byte beyond the allowance to distinguish an exact fit from an overrun.
/// A size hint controls initial allocation only. Formats own their cumulative budgets and
/// decide whether a malformed optional member is fatal to the document.
pub fn read_bounded_member(reader: impl Read, allowance: u64, size_hint: u64) -> MemberRead {
    let capacity = size_hint.min(allowance).min(16 << 20) as usize;
    let mut bytes = Vec::with_capacity(capacity);
    let result = reader
        .take(allowance.saturating_add(1))
        .read_to_end(&mut bytes);
    let produced = bytes.len() as u64;
    if produced > allowance {
        MemberRead::Limit(produced)
    } else {
        match result {
            Ok(_) => MemberRead::Complete(bytes),
            Err(cause) => MemberRead::Failed { produced, cause },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_for_exact_limits_empty_entries_and_failure_after_partial_output() {
        let input = b"12345";
        assert!(
            matches!(read_bounded_member(&input[..], 5, u64::MAX), MemberRead::Complete(v) if v == input)
        );
        assert!(matches!(
            read_bounded_member(&input[..], 2, 0),
            MemberRead::Limit(3)
        ));
        assert!(
            matches!(read_bounded_member(io::empty(), 0, 0), MemberRead::Complete(v) if v.is_empty())
        );
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("bad checksum"))
            }
        }
        assert!(matches!(
            read_bounded_member(input.as_slice().chain(Broken), 10, 0),
            MemberRead::Failed { produced: 5, .. }
        ));
    }
}

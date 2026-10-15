use joyword_recovery_journal::{Error, journal::*};
use std::io::{self, Write};
#[derive(Default)]
struct Sink {
    data: Vec<u8>,
    stop: Option<usize>,
    flush_fail: bool,
    sync_fail: bool,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = bytes.len().min(3).min(
            self.stop
                .unwrap_or(usize::MAX)
                .saturating_sub(self.data.len()),
        );
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "injected disk full",
            ));
        }
        self.data.extend_from_slice(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_fail {
            Err(io::Error::other("injected flush"))
        } else {
            Ok(())
        }
    }
}
impl DurableWrite for Sink {
    fn sync(&mut self) -> io::Result<()> {
        if self.sync_fail {
            Err(io::Error::other("injected sync"))
        } else {
            Ok(())
        }
    }
}
fn sample() -> Vec<u8> {
    let mut log = AppendLog::fresh(Sink::default(), [7; 16], Limits::default());
    log.append(b"baseline").unwrap();
    log.append("中文👩‍💻".as_bytes()).unwrap();
    log.append(b"third").unwrap();
    log.into_inner().data
}
#[test]
fn every_byte_truncation_only_exposes_complete_prefix() {
    let bytes = sample();
    let complete = scan(&bytes, Limits::default()).unwrap();
    assert_eq!(complete.records.len(), 3);
    let ends = [
        0,
        HEADER + 8,
        HEADER + 8 + HEADER + "中文👩‍💻".len(),
        bytes.len(),
    ];
    for n in 0..=bytes.len() {
        let result = scan(&bytes[..n], Limits::default()).unwrap();
        let valid = *ends.iter().rev().find(|&&end| end <= n).unwrap();
        assert_eq!(result.valid_bytes, valid, "cut {n}");
        assert_eq!(result.tail.is_some(), valid != n);
    }
}
#[test]
fn header_payload_corruption_and_huge_lengths_stop_without_resync() {
    let bytes = sample();
    let start = HEADER + 8;
    for offset in [
        start,
        start + 10,
        start + 12,
        start + 20,
        start + 40,
        start + HEADER,
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        let result = scan(&bad, Limits::default()).unwrap();
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.valid_bytes, start);
    }
    let mut bad = bytes.clone();
    bad[start + 36..start + 40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        scan(&bad, Limits::default()).unwrap().tail,
        Some((start, Tail::Length))
    );
    let mut schema = bytes;
    schema[8..10].copy_from_slice(&2u16.to_le_bytes());
    assert!(matches!(
        scan(&schema, Limits::default()),
        Err(Error::Schema(2))
    ));
}
#[test]
fn short_writes_flush_sync_failure_and_poison_are_explicit() {
    for sink in [
        Sink {
            stop: Some(HEADER + 2),
            ..Sink::default()
        },
        Sink {
            flush_fail: true,
            ..Sink::default()
        },
        Sink {
            sync_fail: true,
            ..Sink::default()
        },
    ] {
        let mut log = AppendLog::fresh(sink, [0; 16], Limits::default());
        assert!(matches!(log.append(b"payload"), Err(Error::Io { .. })));
        assert!(matches!(log.append(b"retry"), Err(Error::Poisoned)));
    }
    assert_eq!(scan(&sample(), Limits::default()).unwrap().records.len(), 3);
}
#[test]
fn bounded_limits_and_empty_scan_are_explicit() {
    assert!(scan(&[], Limits::default()).unwrap().records.is_empty());
    let mut log = AppendLog::fresh(
        Sink::default(),
        [0; 16],
        Limits {
            records: 1,
            record_bytes: 2,
            file_bytes: HEADER + 2,
        },
    );
    assert!(matches!(log.append(b"123"), Err(Error::Limit(_))));
    log.append(b"12").unwrap();
    assert!(matches!(log.append(b""), Err(Error::Limit(_))));
    assert!(matches!(
        scan(
            &sample(),
            Limits {
                file_bytes: 1,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn real_file_create_new_read_and_no_source_rewrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("中文 path.jwj");
    let mut log = AppendLog::create(&path, [9; 16], Limits::default()).unwrap();
    log.append(b"one").unwrap();
    drop(log);
    assert!(AppendLog::create(&path, [9; 16], Limits::default()).is_err());
    let original = read(&path, Limits::default()).unwrap();
    assert_eq!(scan(&original, Limits::default()).unwrap().records.len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

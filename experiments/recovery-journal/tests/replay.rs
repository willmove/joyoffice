use joyword_recovery_journal::{Error, journal::*, replay::*};
use std::io::{self, Write};
use std::sync::atomic::AtomicBool;
const SOURCE: &[u8] = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
#[derive(Default)]
struct Memory(Vec<u8>);
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl DurableWrite for Memory {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn setup() -> (Recovery, AppendLog<Memory>) {
    let r = Recovery::open(SOURCE, [3; 16]).unwrap();
    let mut log = AppendLog::fresh(Memory::default(), [3; 16], Limits::default());
    log.append(&r.baseline_payload()).unwrap();
    (r, log)
}
fn commit(r: &mut Recovery, l: &mut AppendLog<Memory>, actions: Vec<Action>) {
    r.commit(l, r.version(), actions, &AtomicBool::new(false))
        .unwrap();
}
fn at(p: usize, s: usize) -> Point {
    Point {
        paragraph: p,
        scalar: s,
    }
}
#[test]
fn real_engine_unicode_split_delete_undo_redo_and_receipts() {
    let (mut r, mut l) = setup();
    let original = r.texts();
    commit(
        &mut r,
        &mut l,
        vec![Action::Insert {
            at: at(0, 0),
            text: "甲👩‍💻e\u{301}".into(),
        }],
    );
    commit(&mut r, &mut l, vec![Action::Split(at(0, 6))]);
    assert!(
        r.docx().export_plan().is_err(),
        "C07 must still refuse structural export"
    );
    commit(
        &mut r,
        &mut l,
        vec![Action::Delete {
            start: at(0, 6),
            end: at(1, 0),
        }],
    );
    let changed = r.texts();
    commit(&mut r, &mut l, vec![Action::Undo]);
    commit(&mut r, &mut l, vec![Action::Redo]);
    assert_eq!(r.texts(), changed);
    let saved = r.version();
    commit(&mut r, &mut l, vec![Action::Saved(saved)]);
    assert!(!r.dirty());
    commit(
        &mut r,
        &mut l,
        vec![Action::Insert {
            at: at(0, 0),
            text: "later".into(),
        }],
    );
    assert!(r.dirty());
    commit(&mut r, &mut l, vec![Action::Saved(saved)]);
    assert!(r.dirty());
    let data = l.into_inner().0;
    let records = scan(&data, Limits::default()).unwrap().records;
    let mut recovered = recover(
        &records,
        [3; 16],
        source_hash(SOURCE),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(recovered.texts(), r.texts());
    assert_eq!(recovered.version(), r.version());
    assert_eq!(recovered.saved_version(), saved);
    recovered
        .docx()
        .backing()
        .original_bytes()
        .eq(SOURCE)
        .then_some(())
        .unwrap();
    assert_ne!(recovered.texts(), original);
    recovered.close();
    let mut wrong = AppendLog::fresh(Memory::default(), [3; 16], Limits::default());
    assert!(
        recovered
            .commit(
                &mut wrong,
                saved,
                vec![Action::Undo],
                &AtomicBool::new(false)
            )
            .is_err()
    );
}
#[test]
fn multi_command_transaction_replays_once_with_logical_positions() {
    let (mut r, mut l) = setup();
    let initial = r.texts();
    commit(
        &mut r,
        &mut l,
        vec![
            Action::Split(at(0, 0)),
            Action::Insert {
                at: at(1, 0),
                text: "inserted".into(),
            },
        ],
    );
    assert_eq!(r.version(), 1);
    assert_eq!(r.texts()[0], "");
    commit(&mut r, &mut l, vec![Action::Undo]);
    assert_eq!(r.texts(), initial);
    let records = scan(&l.into_inner().0, Limits::default()).unwrap().records;
    let recovered = recover(
        &records,
        [3; 16],
        source_hash(SOURCE),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(recovered.texts(), initial);
}
#[test]
fn invalid_events_sources_order_cancel_and_budget_never_publish() {
    let (mut r, mut l) = setup();
    let before = r.texts();
    for actions in [
        vec![],
        vec![Action::Insert {
            at: at(99, 0),
            text: "x".into(),
        }],
        vec![Action::Split(at(0, usize::MAX))],
        vec![Action::Insert {
            at: at(0, 0),
            text: "x".repeat(65_537),
        }],
        vec![Action::Undo],
        vec![Action::Saved(100)],
    ] {
        assert!(
            r.commit(&mut l, 0, actions, &AtomicBool::new(false))
                .is_err()
        );
        assert_eq!(r.texts(), before);
        assert_eq!(r.version(), 0);
        assert_eq!(l.records_written(), 1);
    }
    assert!(matches!(
        r.commit(
            &mut l,
            0,
            vec![Action::Split(at(0, 0))],
            &AtomicBool::new(true)
        ),
        Err(Error::Cancelled)
    ));
    let fresh = AppendLog::fresh(Memory::default(), [3; 16], Limits::default());
    assert_eq!(fresh.records_written(), 0);
    commit(
        &mut r,
        &mut l,
        vec![Action::Insert {
            at: at(0, 0),
            text: "x".into(),
        }],
    );
    let records = scan(&l.into_inner().0, Limits::default()).unwrap().records;
    assert!(
        recover(
            &records,
            [4; 16],
            source_hash(SOURCE),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    assert!(recover(&records, [3; 16], [0; 32], &AtomicBool::new(false)).is_err());
    let mut repeated = records.clone();
    repeated.push(records[1].clone());
    assert!(
        recover(
            &repeated,
            [3; 16],
            source_hash(SOURCE),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let mut corrupt = records;
    corrupt[1].payload[16] ^= 1;
    assert!(
        recover(
            &corrupt,
            [3; 16],
            source_hash(SOURCE),
            &AtomicBool::new(false)
        )
        .is_err()
    );
}
#[test]
fn restricted_baseline_does_not_bypass_docx_policy() {
    let bytes = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-005.docx");
    if let Ok(mut r) = Recovery::open(bytes, [3; 16]) {
        let mut l = AppendLog::fresh(Memory::default(), [3; 16], Limits::default());
        l.append(&r.baseline_payload()).unwrap();
        assert!(
            r.commit(
                &mut l,
                0,
                vec![Action::Insert {
                    at: at(0, 0),
                    text: "x".into()
                }],
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert_eq!(r.version(), 0);
    }
}
#[test]
fn every_event_byte_truncation_and_unknown_action_are_refused() {
    let (mut r, mut l) = setup();
    commit(
        &mut r,
        &mut l,
        vec![Action::Insert {
            at: at(0, 0),
            text: "abc".into(),
        }],
    );
    let records = scan(&l.into_inner().0, Limits::default()).unwrap().records;
    let payload = &records[1].payload;
    for cut in 0..payload.len() {
        assert!(decode(&payload[..cut]).is_err());
    }
    let mut unknown = payload.clone();
    unknown[84] = 99;
    assert!(decode(&unknown).is_err());
}

#[test]
fn uncertain_durable_append_does_not_publish_the_candidate() {
    struct Failing {
        bytes: Vec<u8>,
        syncs: usize,
    }
    impl Write for Failing {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl DurableWrite for Failing {
        fn sync(&mut self) -> io::Result<()> {
            self.syncs += 1;
            if self.syncs == 2 {
                Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "injected final sync",
                ))
            } else {
                Ok(())
            }
        }
    }
    let mut recovery = Recovery::open(SOURCE, [5; 16]).unwrap();
    let mut log = AppendLog::fresh(
        Failing {
            bytes: Vec::new(),
            syncs: 0,
        },
        [5; 16],
        Limits::default(),
    );
    log.append(&recovery.baseline_payload()).unwrap();
    let before = recovery.texts();
    assert!(
        recovery
            .commit(
                &mut log,
                0,
                vec![Action::Insert {
                    at: at(0, 0),
                    text: "uncertain".into()
                }],
                &AtomicBool::new(false)
            )
            .is_err()
    );
    assert_eq!(recovery.version(), 0);
    assert_eq!(recovery.texts(), before);
    assert!(matches!(log.append(b"retry"), Err(Error::Poisoned)));
    // The failed sync does not prove bytes are absent: acknowledge this exact
    // uncertainty; a later scan may see the complete candidate record.
    assert_eq!(
        scan(&log.into_inner().bytes, Limits::default())
            .unwrap()
            .records
            .len(),
        2
    );
}

#[test]
fn cancellation_is_observed_between_replayed_events() {
    let (mut r, mut log) = setup();
    commit(&mut r, &mut log, vec![Action::Saved(0)]);
    let records = scan(&log.into_inner().0, Limits::default())
        .unwrap()
        .records;
    let mut long = vec![records[0].clone()];
    for i in 1..=MAX_EVENTS {
        let mut event = records[1].clone();
        event.sequence = i as u64;
        long.push(event);
    }
    let cancelled = std::sync::Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1));
        signal.store(true, std::sync::atomic::Ordering::Release);
    });
    let result = recover(&long, [3; 16], source_hash(SOURCE), &cancelled);
    worker.join().unwrap();
    assert!(matches!(result, Err(Error::Cancelled)));
}

use joyword_recovery_journal::{
    Error, checkpoint,
    journal::{self, AppendLog, Limits},
    replay::{self, Action, Point, Recovery},
};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;
fn bounded_source(path: &str) -> Result<Vec<u8>, Error> {
    let file = std::fs::File::open(path).map_err(|e| Error::Engine(e.to_string()))?;
    let mut bytes = Vec::new();
    file.take((8 << 20) + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::Engine(e.to_string()))?;
    if bytes.len() > 8 << 20 {
        return Err(Error::Limit("source bytes"));
    }
    Ok(bytes)
}
fn identity(bytes: &[u8]) -> Result<([u8; 16], [u8; 32]), Error> {
    let s = journal::scan(bytes, Limits::default())?;
    let first = s.records.first().ok_or(Error::Invalid("no baseline"))?;
    if first.payload.len() < 32 {
        return Err(Error::Invalid("baseline hash"));
    }
    Ok((first.session, first.payload[..32].try_into().unwrap()))
}
fn execute(args: &[String]) -> Result<(), Error> {
    match args.first().map(String::as_str) {
        Some("inspect") if args.len() == 2 => {
            let bytes = journal::read(Path::new(&args[1]), Limits::default())?;
            let s = journal::scan(&bytes, Limits::default())?;
            println!(
                "records={} valid_bytes={} tail={:?}",
                s.records.len(),
                s.valid_bytes,
                s.tail
            );
            Ok(())
        }
        Some("recover") if args.len() == 4 => {
            let bytes = journal::read(Path::new(&args[1]), Limits::default())?;
            let (session, source) = identity(&bytes)?;
            if source != replay::source_hash(&bounded_source(&args[2])?) {
                return Err(Error::Invalid("source fingerprint mismatch"));
            }
            let (_, recovered) = checkpoint::load(Path::new(&args[1]), session, source)?;
            let receipt =
                checkpoint::copy_prefix(Path::new(&args[1]), Path::new(&args[3]), session, source)?;
            println!(
                "recovered_records={} version={} dirty={} texts={:?}",
                receipt.records,
                recovered.version(),
                recovered.dirty(),
                recovered.texts()
            );
            Ok(())
        }
        Some("checkpoint") if args.len() == 3 => {
            let bytes = journal::read(Path::new(&args[1]), Limits::default())?;
            let (session, source) = identity(&bytes)?;
            println!(
                "{:?}",
                checkpoint::rotate(Path::new(&args[1]), Path::new(&args[2]), session, source)?
            );
            Ok(())
        }
        Some("crash-worker") if args.len() == 3 => {
            let source = bounded_source(&args[1])?;
            let sid: [u8; 16] = replay::source_hash(&source)[..16].try_into().unwrap();
            let mut r = Recovery::open(&source, sid)?;
            let mut log = AppendLog::create(Path::new(&args[2]), sid, Limits::default())?;
            log.append(&r.baseline_payload())?;
            for text in ["恢复甲", "👩‍💻e\u{301}"] {
                r.commit(
                    &mut log,
                    r.version(),
                    vec![Action::Insert {
                        at: Point {
                            paragraph: 0,
                            scalar: 0,
                        },
                        text: text.into(),
                    }],
                    &AtomicBool::new(false),
                )?;
            }
            println!("READY version={}", r.version());
            std::io::stdout()
                .flush()
                .map_err(|e| Error::Engine(e.to_string()))?;
            // Test process only. Parent waits for READY then forcibly kills this child.
            loop {
                std::thread::park();
            }
        }
        _ => Err(Error::Invalid(
            "usage: inspect LOG | recover LOG SOURCE NEW_LOG | checkpoint LOG DIRECTORY | crash-worker SOURCE NEW_LOG",
        )),
    }
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = execute(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

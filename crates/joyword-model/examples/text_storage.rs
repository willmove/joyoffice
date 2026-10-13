//! T100 text storage benchmark (ADR-003). Compares, per paragraph:
//!
//! - `arc`: one `Arc<str>` copied on every edit;
//! - `chunked`: `ParagraphText`, 1 KiB chunks in a persistent B-tree;
//! - `ropey`: Ropey 1.6 (benchmark-only dev-dependency).
//!
//! Run with `cargo run --release -p joyword-model --example text_storage`.
//! Memory is the RSS growth of a child process holding 100 000 short
//! paragraphs, so allocator overhead counts. Edits keep the previous version
//! alive, as a background layout snapshot would. Results depend on the
//! machine; record the environment next to any number you cite.

use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use joyword_model::{ParagraphText, ScalarOffset};
use ropey::Rope;

const SHORT_LINE: &str = "甲方应在十个工作日内支付首期款项，乙方 confirm receipt。";
const PARAGRAPHS: usize = 100_000;
const EDITS: usize = 2_000;

trait Storage: Clone {
    const NAME: &'static str;
    fn build(text: &str) -> Self;
    fn insert(&mut self, at: usize, text: &str);
    fn utf16_at(&self, at: usize) -> usize;
}

impl Storage for Arc<str> {
    const NAME: &'static str = "arc";

    fn build(text: &str) -> Self {
        text.into()
    }

    fn insert(&mut self, at: usize, text: &str) {
        let byte = self.char_indices().nth(at).map_or(self.len(), |(b, _)| b);
        let mut next = String::with_capacity(self.len() + text.len());
        next.push_str(&self[..byte]);
        next.push_str(text);
        next.push_str(&self[byte..]);
        *self = next.into();
    }

    fn utf16_at(&self, at: usize) -> usize {
        self.chars().take(at).map(char::len_utf16).sum()
    }
}

impl Storage for ParagraphText {
    const NAME: &'static str = "chunked";

    fn build(text: &str) -> Self {
        ParagraphText::from_text(text).expect("benchmark text has no reserved marker")
    }

    fn insert(&mut self, at: usize, text: &str) {
        self.insert_str(ScalarOffset(at), text)
            .expect("offset in range");
    }

    fn utf16_at(&self, at: usize) -> usize {
        self.scalar_to_utf16(ScalarOffset(at))
            .expect("offset in range")
            .0
    }
}

impl Storage for Rope {
    const NAME: &'static str = "ropey";

    fn build(text: &str) -> Self {
        Rope::from_str(text)
    }

    fn insert(&mut self, at: usize, text: &str) {
        Rope::insert(self, at, text);
    }

    fn utf16_at(&self, at: usize) -> usize {
        self.char_to_utf16_cu(at)
    }
}

fn rss_kib() -> u64 {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps is available on macOS and Linux");
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("ps prints RSS in KiB")
}

/// Child-process mode: hold PARAGRAPHS short paragraphs, print bytes each.
fn memory<S: Storage>() {
    let before = rss_kib();
    let paragraphs: Vec<S> = (0..PARAGRAPHS).map(|_| S::build(SHORT_LINE)).collect();
    let after = rss_kib();
    std::hint::black_box(&paragraphs);
    let per = (after.saturating_sub(before) * 1024) as f64 / PARAGRAPHS as f64;
    println!("{per:.0}");
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

fn percentile(samples: &mut [Duration], fraction: f64) -> Duration {
    samples.sort_unstable();
    let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
    samples[index]
}

fn edits<S: Storage>(chars: usize) -> (Duration, Duration, Duration) {
    let text: String = SHORT_LINE.chars().cycle().take(chars).collect();
    let mut current = S::build(&text);
    let mut rng = Lcg(42);
    let mut samples = Vec::with_capacity(EDITS);
    let mut len = chars;
    for _ in 0..EDITS {
        let at = rng.next(len + 1);
        let snapshot = current.clone();
        let start = Instant::now();
        current.insert(at, "改");
        samples.push(start.elapsed());
        std::hint::black_box(&snapshot);
        len += 1;
    }
    let mut conversions = Vec::with_capacity(EDITS);
    for _ in 0..EDITS {
        let at = rng.next(len + 1);
        let start = Instant::now();
        std::hint::black_box(current.utf16_at(at));
        conversions.push(start.elapsed());
    }
    (
        percentile(&mut samples, 0.5),
        percentile(&mut samples, 0.95),
        percentile(&mut conversions, 0.95),
    )
}

fn run_child(mode: &str) -> String {
    let output = Command::new(std::env::current_exe().expect("current executable"))
        .arg(mode)
        .output()
        .expect("benchmark child process");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn report<S: Storage>() {
    let memory = run_child(S::NAME);
    println!("{:<8} memory/paragraph ≈ {memory} B", S::NAME);
    for chars in [100, 10_000, 1_000_000] {
        let (p50, p95, utf16) = edits::<S>(chars);
        println!(
            "{:<8} {:>9} chars: insert p50 {:>10.2?} p95 {:>10.2?} | utf16 offset p95 {:>10.2?}",
            S::NAME,
            chars,
            p50,
            p95,
            utf16
        );
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("arc") => return memory::<Arc<str>>(),
        Some("chunked") => return memory::<ParagraphText>(),
        Some("ropey") => return memory::<Rope>(),
        _ => {}
    }
    println!(
        "short paragraph: {} chars / {} UTF-8 bytes; {PARAGRAPHS} paragraphs for memory; {EDITS} edits",
        SHORT_LINE.chars().count(),
        SHORT_LINE.len()
    );
    report::<Arc<str>>();
    report::<ParagraphText>();
    report::<Rope>();
}

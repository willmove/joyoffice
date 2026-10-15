use joyword_docx_robustness::{MAX_INPUT, ResultKind, TARGET_SHA, budget, digest, json, run};
use std::io::Read;
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        eprintln!("usage: TARGET CASE_ID SEED INPUT_PATH_OR_DASH");
        std::process::exit(64);
    }
    let seed = match args[2].parse::<u64>() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("invalid seed");
            std::process::exit(64)
        }
    };
    if !matches!(args[0].as_str(), "opc" | "import" | "roundtrip") {
        eprintln!("invalid target");
        std::process::exit(64);
    }
    let input: Box<dyn Read> = if args[3] == "-" {
        Box::new(std::io::stdin())
    } else {
        match std::fs::File::open(&args[3]) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(66)
            }
        }
    };
    let mut bytes = Vec::new();
    if let Err(e) = input.take(MAX_INPUT as u64 + 1).read_to_end(&mut bytes) {
        eprintln!("{e}");
        std::process::exit(66);
    }
    let report = run(&args[0], &bytes, seed);
    let (kind, code) = match report.kind {
        ResultKind::Ok => ("ok", 0),
        ResultKind::Rejected => ("rejected", 2),
        ResultKind::Invariant => ("invariant", 3),
    };
    let b = budget();
    println!(
        "{{\"schema\":1,\"target\":{},\"case_id\":{},\"seed\":{},\"target_sha\":{},\"input_sha256\":{},\"bytes\":{},\"status\":{},\"reason\":{},\"checks\":{},\"budget\":{{\"input_bytes\":{},\"entries\":{},\"part_bytes\":{},\"total_bytes\":{},\"ratio\":{},\"ratio_floor_bytes\":{},\"xml_depth\":{},\"xml_nodes\":{},\"relationships\":{}}}}}",
        json(&args[0]),
        json(&args[1]),
        seed,
        json(TARGET_SHA),
        json(&digest(&bytes)),
        bytes.len(),
        json(kind),
        json(&report.reason),
        report.checks,
        MAX_INPUT,
        b.max_entries,
        b.max_part_bytes,
        b.max_total_bytes,
        b.max_ratio,
        b.ratio_floor_bytes,
        b.max_xml_depth,
        b.max_xml_nodes,
        b.max_relationships
    );
    std::process::exit(code);
}

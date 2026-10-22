// Counterexample for the filesystem gate path filter in scripts/check_gates.py.
//
// The test tree places this file at `crates/demo/src/contests/rules.rs`.
// `contests/` merely contains the substring `tests/`, so a substring filter
// skipped it; a real path-component filter scans it as production code.
fn load_rules(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

fn store_rules(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

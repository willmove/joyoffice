// Counterexample for the filesystem gate in scripts/check_gates.py. Not part of the build.
//!
//! `text.split("#[cfg(test)]", 1)[0]` threw away everything below the first
//! marker, so all of the production code in this file was invisible. Only the
//! body of the test module may be skipped.

use std::process::Command;

#[cfg(test)]
mod tests {
    #[test]
    fn a_test_module_may_touch_the_filesystem() {
        let made = std::fs::create_dir_all("target/gate-counterexample");
        let _ = std::fs::remove_dir_all("target/gate-counterexample");
        assert!(made.is_ok());
        let brace_in_a_string = "}";
        assert_eq!(brace_in_a_string.len(), 1);
    }
}

fn spawning_a_process_after_the_test_module(program: &str) -> bool {
    Command::new(program).arg("--help").spawn().is_ok()
}

fn opening_after_the_test_module(path: &std::path::Path) -> Option<u64> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let opened = options.open(path).ok()?;
    let direct = std::fs::File::open(path).ok()?;
    let entries = std::fs::read_dir(path).ok()?;
    drop(entries);
    let linked = std::fs::hard_link(path, path);
    drop(linked);
    let removed = std::fs::remove_dir_all(path);
    drop(removed);
    Some(direct.metadata().ok()?.len() + opened.metadata().ok()?.len())
}

use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git to identify the production tree");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}

// The harness reports the exact production code it was built against: the
// Git tree id of the committed `crates/` directory. A tree id is content
// addressed, so it stays valid across squash merges, deleted branches and
// shallow clones, and docs-only commits keep the same id.
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let changes = git(
        &root,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "crates",
        ],
    );
    assert!(
        changes.is_empty(),
        "Production crates have uncommitted changes; commit them so results name an exact tree:\n{changes}"
    );
    let tree = git(&root, &["rev-parse", "HEAD:crates"]);
    println!("cargo:rustc-env=JOYWORD_PRODUCTION_TREE={tree}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../crates");
}

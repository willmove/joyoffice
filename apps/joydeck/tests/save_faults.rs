use joydeck::{
    edit::{Command, Session},
    storage,
};
use std::path::PathBuf;
fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("joydeck-fault-{name}-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    root
}
#[test]
fn occupied_destination_and_missing_parent_preserve_document() {
    let root = root("destination");
    let path = root.join("deck.joydeck");
    let mut s = Session::new();
    let fp = storage::save(&path, &s.deck, None).unwrap();
    let before = std::fs::read(&path).unwrap();
    s.execute(s.id, Command::AddPage).unwrap();
    assert!(storage::save(&path, &s.deck, None).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(s.dirty());
    let occupied = root.join("directory.joydeck");
    std::fs::create_dir(&occupied).unwrap();
    assert!(storage::save(&occupied, &s.deck, None).is_err());
    assert!(storage::save(&root.join("missing/deck.joydeck"), &s.deck, None).is_err());
    assert_eq!(storage::fingerprint(&path).unwrap(), fp);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
    std::fs::remove_dir(occupied).unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[cfg(unix)]
#[test]
fn unwritable_parent_preserves_target_and_cleans_staging() {
    use std::os::unix::fs::PermissionsExt;
    let root = root("permission");
    let path = root.join("deck.joydeck");
    let mut s = Session::new();
    let fp = storage::save(&path, &s.deck, None).unwrap();
    let before = std::fs::read(&path).unwrap();
    s.execute(s.id, Command::AddPage).unwrap();
    let mode = std::fs::metadata(&root).unwrap().permissions().mode();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = storage::save(&path, &s.deck, Some(fp));
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(mode)).unwrap();
    assert!(result.is_err(), "requires a non-root test account");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    assert!(s.dirty());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

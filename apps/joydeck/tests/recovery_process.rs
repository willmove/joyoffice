use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn abnormal_process_exit_releases_lease_and_restores_three_pages() {
    let root = std::env::temp_dir().join(format!(
        "joydeck-crash-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut command = Command::new(env!("CARGO_BIN_EXE_joydeck"));
    command.args(["--recovery-crash-probe"]).arg(&root);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    assert_eq!(command.status().unwrap().code(), Some(73));
    let service = joydeck::storage::Recovery::new(root.clone());
    let list = service.list().unwrap();
    assert_eq!(list.len(), 1);
    assert!(!list[0].active);
    assert_eq!(service.restore(&list[0].record).unwrap().slides.len(), 3);
    service.prune(0).unwrap();
    drop(service);
    for e in std::fs::read_dir(&root).unwrap() {
        let p: PathBuf = e.unwrap().path();
        std::fs::remove_file(p).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}

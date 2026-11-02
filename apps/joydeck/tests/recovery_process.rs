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

#[cfg(feature = "validation")]
#[test]
#[ignore = "requires a native desktop and graphics device"]
fn native_startup_and_all_ribbons_fit_the_default_ui_thread_stack() {
    use std::{process::Stdio, time::Duration};

    let root = std::env::temp_dir().join(format!(
        "joydeck-startup-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // Exercise first launch and a restored narrow layout with both panes open.
    for (width, inspector) in [(1280, false), (980, true)] {
        let state = root.join(width.to_string());
        std::fs::create_dir_all(&state).unwrap();
        let preferences = joydeck::preferences::Preferences {
            width,
            height: 800,
            inspector,
            notes: inspector,
            ..Default::default()
        };
        std::fs::write(
            state.join("preferences.json"),
            serde_json::to_vec(&preferences).unwrap(),
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_joydeck"));
        command
            .arg("--startup-smoke")
            .env("JOYDECK_STATE_DIR", &state)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let require_a11y = std::env::var_os("JOYDECK_TEST_A11Y").is_some();
        if require_a11y {
            command.arg("--require-a11y");
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().unwrap();
        let began = std::time::Instant::now();
        while child.try_wait().unwrap().is_none() {
            if began.elapsed() > Duration::from_secs(45) {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "native startup timed out: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let output = child.wait_with_output().unwrap();
        let log = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "width={width}: {log}");
        assert!(
            log.contains(if require_a11y {
                "A11Y_SMOKE design_notes_complete=true"
            } else {
                "STARTUP_SMOKE complete=true cases=48"
            }),
            "width={width}: {log}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

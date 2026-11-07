//! Owned print-helper execution. Call on a background worker, never the UI thread.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const MAX_OUTPUT: u64 = 64 * 1024;
const POLL: Duration = Duration::from_millis(20);

struct OwnedChild(Child);
impl OwnedChild {
    fn reap(&mut self) -> Result<(), String> {
        let cleanup = || "Print helper cleanup failed; queue submission is unknown".to_string();
        if self.0.try_wait().map_err(|_| cleanup())?.is_none() {
            if self.0.kill().is_err() && self.0.try_wait().map_err(|_| cleanup())?.is_none() {
                return Err(cleanup());
            }
            self.0.wait().map_err(|_| cleanup())?;
        }
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Also reap on an early I/O error. This owns only the direct helper.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn captured(file: &mut File) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err("Print helper output exceeded its limit; queue submission is unknown".into());
    }
    Ok(bytes)
}

/// Temp files avoid pipe backpressure and reader threads waiting on inherited
/// pipes. Memory reads are capped; disk usage is checked each polling interval.
/// The deadline bounds normal user-space waiting, not OS kill/wait latency.
pub(crate) fn run(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    if timeout.is_zero() {
        return Err("Print helper deadline must be positive".into());
    }
    let mut stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
    command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
        .stderr(stderr.try_clone().map_err(|e| e.to_string())?);
    let start = Instant::now();
    let mut child = OwnedChild(
        command
            .spawn()
            .map_err(|e| format!("Print helper launch failed: {e}"))?,
    );
    let outcome = (|| {
        loop {
            if stdout.metadata().map_err(|e| e.to_string())?.len() > MAX_OUTPUT
                || stderr.metadata().map_err(|e| e.to_string())?.len() > MAX_OUTPUT
            {
                return Err(
                    "Print helper output exceeded its limit; queue submission is unknown".into(),
                );
            }
            if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
                return Ok(Output {
                    status,
                    stdout: captured(&mut stdout)?,
                    stderr: captured(&mut stderr)?,
                });
            }
            if start.elapsed() >= timeout {
                return Err("Print helper timed out; queue submission is unknown. Check the queue before retrying".into());
            }
            std::thread::sleep(POLL.min(timeout.saturating_sub(start.elapsed())));
        }
    })();
    child.reap()?;
    outcome
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn helper_child() {
        let Ok(mode) = std::env::var("JOYWORD_TEST_PRINT_HELPER") else {
            return;
        };
        match mode.as_str() {
            "success" => {
                println!("synthetic submission reply");
            }
            "failure" => {
                eprintln!("synthetic helper failure");
                std::process::exit(7);
            }
            "excess" => {
                use std::io::Write;
                std::io::stdout()
                    .write_all(&vec![b'x'; 128 * 1024])
                    .unwrap();
                std::thread::sleep(Duration::from_secs(5));
            }
            "hang" => std::thread::sleep(Duration::from_secs(5)),
            _ => panic!("unknown synthetic mode"),
        }
    }

    fn command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "print_process::tests::helper_child",
                "--nocapture",
            ])
            .env("JOYWORD_TEST_PRINT_HELPER", mode);
        command
    }

    #[test]
    fn captures_success_and_failure_without_a_print_queue() {
        let output = run(&mut command("success"), Duration::from_secs(3)).unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("synthetic submission reply"));
        let output = run(&mut command("failure"), Duration::from_secs(3)).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(String::from_utf8_lossy(&output.stderr).contains("synthetic helper failure"));
    }

    #[test]
    fn deadline_and_output_budget_stop_the_owned_child() {
        let before = Instant::now();
        let error = run(&mut command("hang"), Duration::from_millis(100)).unwrap_err();
        assert!(error.contains("timed out") && error.contains("unknown"));
        assert!(before.elapsed() < Duration::from_secs(4));
        let error = run(&mut command("excess"), Duration::from_secs(3)).unwrap_err();
        assert!(error.contains("output exceeded") && error.contains("unknown"));
    }

    #[test]
    fn refuses_zero_deadline_and_reports_launch_failure() {
        assert!(run(&mut command("hang"), Duration::ZERO).is_err());
        let dir = tempfile::tempdir().unwrap();
        match run(
            &mut Command::new(dir.path().join("missing-helper")),
            Duration::from_secs(1),
        ) {
            Err(message) => assert!(message.contains("launch failed")),
            // QEMU user emulation launches a wrapper that exits 127 instead of
            // surfacing exec ENOENT from spawn.
            Ok(output) => {
                assert_eq!(output.status.code(), Some(127));
                assert!(output.stdout.is_empty() && output.stderr.is_empty());
            }
        }
    }
}

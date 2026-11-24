//! Bounded blocking HTTP(S) client for verified language-pack downloads.
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use super::Cancel;

/// Request timeout policy for language-pack index and archive transfers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Timeouts {
    pub connect: Duration,
    pub response: Duration,
    pub overall: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(15),
            response: Duration::from_secs(30),
            overall: Duration::from_secs(600),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum FetchError {
    #[error("cancelled")]
    Cancelled,
    #[error("the response is larger than {max} bytes")]
    TooLarge { max: u64 },
    #[error("{0}")]
    Failed(String),
}

const USER_AGENT: &str = concat!("joyword-packs/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub(super) struct Client {
    agent: ureq::Agent,
}

impl Default for Client {
    fn default() -> Self {
        Self::new(Timeouts::default())
    }
}

impl Client {
    pub(super) fn new(timeouts: Timeouts) -> Self {
        Self::build(timeouts, true)
    }

    pub(super) fn allowing_http(timeouts: Timeouts) -> Self {
        Self::build(timeouts, false)
    }

    fn build(t: Timeouts, https_only: bool) -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(https_only)
            .user_agent(USER_AGENT)
            .timeout_connect(Some(t.connect))
            .timeout_recv_response(Some(t.response))
            .timeout_global(Some(t.overall))
            .max_redirects(3)
            .http_status_as_error(true)
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }

    pub(super) fn get(
        &self,
        url: &str,
        max: u64,
        cancel: &Cancel,
        on_bytes: &mut dyn FnMut(u64),
    ) -> Result<Vec<u8>, FetchError> {
        if cancel.is_cancelled() {
            return Err(FetchError::Cancelled);
        }
        // One outstanding wakeup is sufficient. A slow progress callback cannot
        // grow the queue, and completion makes the final byte count visible.
        let (tx, rx) = mpsc::sync_channel(1);
        let latest = Arc::new(AtomicU64::new(0));
        let worker_bytes = Arc::clone(&latest);
        let (agent, url, worker_cancel) = (self.agent.clone(), url.to_owned(), cancel.clone());
        std::thread::Builder::new()
            .name("joyword-pack-fetch".into())
            .spawn(move || {
                let result = receive_body(&agent, &url, max, &worker_cancel, |count| {
                    worker_bytes.store(count, Ordering::Relaxed);
                    let _ = tx.try_send(Update::Progress);
                });
                let _ = tx.send(Update::Complete(result));
            })
            .map_err(|e| FetchError::Failed(format!("can't start the download: {e}")))?;
        let mut reported = 0;
        loop {
            let update = rx.recv_timeout(Duration::from_millis(20));
            if cancel.is_cancelled() {
                return Err(FetchError::Cancelled);
            }
            let count = latest.load(Ordering::Relaxed);
            if count > reported {
                on_bytes(count);
                reported = count;
                if cancel.is_cancelled() {
                    return Err(FetchError::Cancelled);
                }
            }
            match update {
                Ok(Update::Complete(result)) => return result,
                Ok(Update::Progress) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(FetchError::Failed(
                        "the download stopped unexpectedly".into(),
                    ));
                }
            }
        }
    }
}

enum Update {
    Progress,
    Complete(Result<Vec<u8>, FetchError>),
}

fn describe(e: &ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        ureq::Error::Timeout(_) => "timed out".into(),
        other => other.to_string(),
    }
}

fn receive_body(
    agent: &ureq::Agent,
    url: &str,
    max: u64,
    cancel: &Cancel,
    mut publish: impl FnMut(u64),
) -> Result<Vec<u8>, FetchError> {
    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| FetchError::Failed(describe(&e)))?;
    if resp.body().content_length().is_some_and(|n| n > max) {
        return Err(FetchError::TooLarge { max });
    }
    let mut reader = resp
        .body_mut()
        .with_config()
        .limit(max.saturating_add(1))
        .reader();
    let mut body = Vec::new();
    let mut chunk = [0; 32 * 1024];
    while !cancel.is_cancelled() {
        let count = reader
            .read(&mut chunk)
            .map_err(|e| FetchError::Failed(format!("reading the response failed: {e}")))?;
        if count == 0 {
            return Ok(body);
        }
        let received = body.len() as u64 + count as u64;
        if received > max {
            return Err(FetchError::TooLarge { max });
        }
        body.extend_from_slice(&chunk[..count]);
        publish(received);
    }
    Err(FetchError::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    #[test]
    fn cancelling_after_progress_returns_while_the_body_is_stalled() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pack", listener.local_addr().unwrap());
        let (release, stalled) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = BufReader::new(&mut socket);
            let mut line = String::new();
            loop {
                line.clear();
                let count = request.read_line(&mut line).unwrap();
                if count == 0 || line == "\r\n" {
                    break;
                }
            }
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Length: 65536\r\n\r\n").unwrap();
            socket.write_all(&[1; 32768]).unwrap();
            stalled.recv_timeout(Duration::from_secs(5)).is_ok()
        });
        let cancel = Cancel::new();
        let mut reported = 0;
        let result =
            Client::allowing_http(Timeouts::default()).get(&url, 65536, &cancel, &mut |count| {
                reported = count;
                cancel.cancel();
            });
        release.send(()).unwrap();
        assert!(server.join().unwrap());
        assert_eq!(result, Err(FetchError::Cancelled));
        assert!((1..=32768).contains(&reported));
    }
}

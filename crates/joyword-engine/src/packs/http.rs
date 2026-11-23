//! Bounded blocking HTTP(S) client for verified language-pack downloads.
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (agent, url, flag) = (self.agent.clone(), url.to_owned(), Arc::clone(&stop));
        std::thread::Builder::new()
            .name("joyword-pack-fetch".into())
            .spawn(move || {
                let progress = tx.clone();
                let result = download(&agent, &url, max, &flag, &mut |n| {
                    let _ = progress.send(Msg::Bytes(n));
                });
                let _ = tx.send(Msg::Done(result));
            })
            .map_err(|e| FetchError::Failed(format!("can't start the download: {e}")))?;
        loop {
            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(Msg::Bytes(n)) => on_bytes(n),
                Ok(Msg::Done(r)) => return r,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(FetchError::Failed(
                        "the download stopped unexpectedly".into(),
                    ));
                }
            }
            if cancel.is_cancelled() {
                stop.store(true, Ordering::Relaxed);
                return Err(FetchError::Cancelled);
            }
        }
    }
}

enum Msg {
    Bytes(u64),
    Done(Result<Vec<u8>, FetchError>),
}

fn describe(e: &ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        ureq::Error::Timeout(_) => "timed out".into(),
        other => other.to_string(),
    }
}

fn download(
    agent: &ureq::Agent,
    url: &str,
    max: u64,
    stop: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
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
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if stop.load(Ordering::Relaxed) {
            return Err(FetchError::Cancelled);
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| FetchError::Failed(format!("reading the response failed: {e}")))?;
        if n == 0 {
            return Ok(out);
        }
        if (out.len() as u64).saturating_add(n as u64) > max {
            return Err(FetchError::TooLarge { max });
        }
        out.extend_from_slice(buf.get(..n).unwrap_or_default());
        on_bytes(out.len() as u64);
    }
}

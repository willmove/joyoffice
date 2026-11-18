//! Fetch the published release index and its archives under the user's existing
//! consent. The host HTTP client retains its connection timeouts and mirrors.
use super::{IndexSource, MAX_PACK_INDEX_BYTES, PackEntry, local_source};
use crate::http::{Cancel, Client, Timeouts};
use crate::store::{Error, FontStore, Progress, io};
use crate::verify::{VerifyError, sha256_hex};
use std::io::Read;

struct FileSpec<'a> {
    name: &'a str,
    limit: u64,
    exact_size: bool,
    sha: Option<&'a str>,
}

impl FileSpec<'_> {
    fn verify(&self, bytes: &[u8]) -> Result<(), VerifyError> {
        let actual = bytes.len() as u64;
        if self.exact_size && actual != self.limit {
            return Err(VerifyError::Size {
                expected: self.limit,
                actual,
            });
        }
        if self.sha.is_some_and(|digest| digest != sha256_hex(bytes)) {
            return Err(VerifyError::Sha256);
        }
        Ok(())
    }

    fn receive(
        &self,
        store: &FontStore,
        url: &str,
        local: bool,
        cancel: &Cancel,
        progress: &mut dyn FnMut(u64),
    ) -> Result<Vec<u8>, Error> {
        store.require_network()?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let check = |bytes: &[u8]| self.verify(bytes);
        let remote = |url: &str, progress: &mut dyn FnMut(u64)| {
            store.fetch(
                &[url.to_owned()],
                self.limit,
                self.name,
                cancel,
                progress,
                &check,
            )
        };
        let error = |detail: String| Error::Download {
            what: self.name.to_owned(),
            reasons: vec![detail],
        };
        let invalid_url = || Error::BadUrl(url.chars().take(200).collect());
        let bytes = if local {
            match local_source(url).ok_or_else(invalid_url)? {
                Ok(https) if https.starts_with("https://") => remote(https, progress)?,
                Ok(loopback) => Client::allowing_http(Timeouts::default())
                    .get(loopback, self.limit, cancel, progress)
                    .map_err(|e| error(e.to_string()))?,
                Err(path) => {
                    let mut file = std::fs::File::open(&path)
                        .map_err(io(format!("opening {}", path.display())))?
                        .take(self.limit.saturating_add(1));
                    let mut bytes = Vec::new();
                    let mut chunk = [0; 16 * 1024];
                    loop {
                        if cancel.is_cancelled() {
                            return Err(Error::Cancelled);
                        }
                        let count = file
                            .read(&mut chunk)
                            .map_err(io(format!("reading {}", path.display())))?;
                        if count == 0 {
                            if bytes.is_empty() {
                                progress(0);
                            }
                            break;
                        }
                        bytes.extend_from_slice(&chunk[..count]);
                        if bytes.len() as u64 > self.limit {
                            return Err(error(format!("larger than {} bytes", self.limit)));
                        }
                        progress(bytes.len() as u64);
                    }
                    bytes
                }
            }
        } else if url.starts_with("https://") {
            remote(url, progress)?
        } else {
            return Err(invalid_url());
        };
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        // The generic HTTPS fetcher checks every mirror before accepting it.
        // Local paths and the loopback test service need the same verification.
        if local && !url.starts_with("https://") {
            self.verify(&bytes).map_err(|error| Error::Verify {
                file: self.name.to_owned(),
                error,
            })?;
        }
        Ok(bytes)
    }
}

pub(super) fn index(
    store: &FontStore,
    source: &IndexSource,
    cancel: &Cancel,
) -> Result<Vec<u8>, Error> {
    let (url, digest) = match source {
        IndexSource::Pinned(pin) => (pin.url, Some(pin.sha256)),
        IndexSource::Override(location) => (location.as_str(), None),
    };
    FileSpec {
        name: "the Language pack index",
        limit: MAX_PACK_INDEX_BYTES,
        exact_size: false,
        sha: digest,
    }
    .receive(store, url, source.is_override(), cancel, &mut |_| {})
}

pub(super) fn archive(
    store: &FontStore,
    pack: &PackEntry,
    local: bool,
    cancel: &Cancel,
    progress: &mut dyn FnMut(Progress),
) -> Result<Vec<u8>, Error> {
    FileSpec {
        name: &format!("{}.tar.zst", pack.locale),
        limit: pack.size,
        exact_size: true,
        sha: Some(&pack.sha256),
    }
    .receive(store, &pack.url, local, cancel, &mut |done| {
        progress(Progress {
            file: 0,
            files: 1,
            done,
            total: pack.size,
        });
    })
}

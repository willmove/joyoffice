//! Credential slots. The secret value is never written into config, logs, or
//! [`Debug`](std::fmt::Debug). Operating-system keychains are a stub behind
//! [`CredentialStore`]; tests use [`MemoryCredentialStore`].

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

/// A credential value. Display and debug output are the fixed redaction marker.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret {
    value: String,
}

impl Secret {
    pub const MAX_LEN: usize = 4096;

    pub fn new(value: impl Into<String>) -> Result<Self, CredentialError> {
        let value = value.into();
        if value.is_empty() || value.len() > Self::MAX_LEN || value.chars().any(char::is_control) {
            return Err(CredentialError::Rejected);
        }
        Ok(Self { value })
    }

    /// Borrow the raw value for a provider call. Callers must not log it.
    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialError {
    Rejected,
    Unavailable,
    Poisoned,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CredentialError::Rejected => "credential was rejected",
            CredentialError::Unavailable => "system credential store is not available",
            CredentialError::Poisoned => "credential store lock was poisoned",
        })
    }
}

impl std::error::Error for CredentialError {}

/// Replace exact occurrences of `secrets` with `[redacted]`.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_owned();
    for secret in secrets {
        if secret.is_empty() {
            continue;
        }
        out = out.replace(secret, "[redacted]");
    }
    out
}

pub trait CredentialStore: Send + Sync {
    fn get(&self, slot: &str) -> Result<Option<Secret>, CredentialError>;
    fn put(&self, slot: &str, secret: Secret) -> Result<(), CredentialError>;
    fn delete(&self, slot: &str) -> Result<(), CredentialError>;
}

/// Process-local store for tests and for sessions before an OS backend exists.
#[derive(Debug, Default)]
pub struct MemoryCredentialStore {
    slots: Mutex<BTreeMap<String, Secret>>,
}

impl MemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, BTreeMap<String, Secret>>, CredentialError> {
        self.slots.lock().map_err(|_| CredentialError::Poisoned)
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn get(&self, slot: &str) -> Result<Option<Secret>, CredentialError> {
        Ok(self.lock()?.get(slot).cloned())
    }

    fn put(&self, slot: &str, secret: Secret) -> Result<(), CredentialError> {
        if !valid_slot(slot) {
            return Err(CredentialError::Rejected);
        }
        self.lock()?.insert(slot.to_owned(), secret);
        Ok(())
    }

    fn delete(&self, slot: &str) -> Result<(), CredentialError> {
        self.lock()?.remove(slot);
        Ok(())
    }
}

/// Stub OS backend. It does not read a keychain and always reports unavailable.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableSystemStore;

impl CredentialStore for UnavailableSystemStore {
    fn get(&self, _slot: &str) -> Result<Option<Secret>, CredentialError> {
        Err(CredentialError::Unavailable)
    }

    fn put(&self, _slot: &str, _secret: Secret) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }

    fn delete(&self, _slot: &str) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

pub(crate) fn valid_slot(slot: &str) -> bool {
    !slot.is_empty()
        && slot.len() <= 64
        && slot
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
}

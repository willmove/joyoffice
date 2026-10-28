//! Document protection settings, editing restrictions, and password verification (ECMA-376 §17.15.1.29).

use crate::crypto::{constant_time_eq, sha1, sha256, sha512};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionMode {
    ReadOnly,
    Comments,
    TrackedChanges,
    Forms,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentProtection {
    pub mode: ProtectionMode,
    pub enforced: bool,
    pub algorithm_name: Option<String>,
    pub salt: Option<Vec<u8>>,
    pub hash_value: Option<Vec<u8>>,
    pub spin_count: u32,
}

impl DocumentProtection {
    /// Parse `w:documentProtection` element attributes.
    pub fn parse(
        edit: Option<&str>,
        enforcement: Option<&str>,
        alg_name: Option<&str>,
        hash: Option<&str>,
        salt: Option<&str>,
        spin_count: Option<&str>,
    ) -> Option<Self> {
        let mode = match edit? {
            "readOnly" => ProtectionMode::ReadOnly,
            "comments" => ProtectionMode::Comments,
            "trackedChanges" => ProtectionMode::TrackedChanges,
            "forms" => ProtectionMode::Forms,
            _ => return None,
        };

        let enforced = match enforcement {
            Some("1") | Some("true") | Some("on") => true,
            Some("0") | Some("false") | Some("off") => false,
            _ => true,
        };

        let salt_bytes = salt.and_then(|s| decode_base64(s.trim()));
        let hash_bytes = hash.and_then(|h| decode_base64(h.trim()));
        let spin = spin_count
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(100_000);

        Some(Self {
            mode,
            enforced,
            algorithm_name: alg_name.map(|s| s.to_string()),
            salt: salt_bytes,
            hash_value: hash_bytes,
            spin_count: spin,
        })
    }

    /// Verify a user-provided password against the protected hash.
    /// Never logs or leaks the plaintext password.
    pub fn verify_password(&self, password: &str) -> bool {
        if !self.enforced {
            return true;
        }
        let Some(ref expected_hash) = self.hash_value else {
            return false;
        };
        let salt = self.salt.as_deref().unwrap_or(&[]);

        // ECMA-376 §17.15.1.29:
        // Password is UTF-16LE encoded.
        let mut pwd_utf16_bytes = Vec::new();
        for c in password.encode_utf16() {
            pwd_utf16_bytes.extend_from_slice(&c.to_le_bytes());
        }

        let alg = self
            .algorithm_name
            .as_deref()
            .unwrap_or("SHA-512")
            .to_uppercase();

        let mut current_hash: Vec<u8> = match alg.as_str() {
            "SHA-1" => {
                let mut combined = salt.to_vec();
                combined.extend_from_slice(&pwd_utf16_bytes);
                sha1(&combined).to_vec()
            }
            "SHA-256" => {
                let mut combined = salt.to_vec();
                combined.extend_from_slice(&pwd_utf16_bytes);
                sha256(&combined).to_vec()
            }
            _ => {
                // Default to SHA-512
                let mut combined = salt.to_vec();
                combined.extend_from_slice(&pwd_utf16_bytes);
                sha512(&combined).to_vec()
            }
        };

        // Iterative spin hashing: H(current_hash || counter)
        for i in 0..self.spin_count {
            let mut iter_buf = current_hash;
            iter_buf.extend_from_slice(&i.to_le_bytes());
            current_hash = match alg.as_str() {
                "SHA-1" => sha1(&iter_buf).to_vec(),
                "SHA-256" => sha256(&iter_buf).to_vec(),
                _ => sha512(&iter_buf).to_vec(),
            };
        }

        constant_time_eq(&current_hash, expected_hash)
    }
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in input.as_bytes() {
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

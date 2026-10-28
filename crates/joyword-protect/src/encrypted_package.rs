//! MS-OFFCRYPTO Agile and Standard Encryption parsing and decryption.
//!
//! Standards: [MS-OFFCRYPTO]: Office Document Cryptography Structure.

use joyoffice_cfb::{CfbBudget, CfbError, CfbWriter, CompoundFile};
use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::crypto::{aes_cbc_decrypt, aes_cbc_encrypt, constant_time_eq, sha512};
use crate::protection::decode_base64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionError {
    NotEncrypted,
    CfbError(String),
    MalformedEncryptionInfo(String),
    UnsupportedAlgorithm(String),
    InvalidPassword,
    DecryptionFailed(String),
}

impl From<CfbError> for EncryptionError {
    fn from(err: CfbError) -> Self {
        Self::CfbError(err.to_string())
    }
}

pub struct EncryptedPackageReader;

impl EncryptedPackageReader {
    /// Check whether bytes represent an encrypted Compound File package.
    pub fn is_encrypted_package(bytes: &[u8]) -> bool {
        let budget = CfbBudget {
            max_file_size: 16 * 1024 * 1024,
            ..CfbBudget::default()
        };
        if let Ok(cfb) = CompoundFile::parse(bytes, budget) {
            cfb.read_stream("EncryptionInfo").is_ok() && cfb.read_stream("EncryptedPackage").is_ok()
        } else {
            false
        }
    }

    /// Decrypt an MS-OFFCRYPTO encrypted package using the given password.
    pub fn decrypt(bytes: &[u8], password: &str) -> Result<Vec<u8>, EncryptionError> {
        let budget = CfbBudget::default();
        let cfb = CompoundFile::parse(bytes, budget)?;

        let enc_info_bytes = cfb
            .read_stream("EncryptionInfo")
            .map_err(|_| EncryptionError::NotEncrypted)?;
        let enc_pkg_bytes = cfb
            .read_stream("EncryptedPackage")
            .map_err(|_| EncryptionError::NotEncrypted)?;

        if enc_info_bytes.len() < 8 {
            return Err(EncryptionError::MalformedEncryptionInfo(
                "header too small".into(),
            ));
        }

        let major = u16::from_le_bytes([enc_info_bytes[0], enc_info_bytes[1]]);
        let minor = u16::from_le_bytes([enc_info_bytes[2], enc_info_bytes[3]]);

        if major == 4 && minor == 4 {
            // Agile Encryption XML descriptor starts at offset 8
            let xml_slice = &enc_info_bytes[8..];
            Self::decrypt_agile(xml_slice, &enc_pkg_bytes, password)
        } else {
            Err(EncryptionError::UnsupportedAlgorithm(format!(
                "legacy encryption version {major}.{minor} unsupported"
            )))
        }
    }

    fn decrypt_agile(
        xml_bytes: &[u8],
        encrypted_package: &[u8],
        password: &str,
    ) -> Result<Vec<u8>, EncryptionError> {
        // Parse keyData and encryptedKey elements from Agile XML
        let mut reader = Reader::from_reader(xml_bytes);
        reader.config_mut().trim_text(true);

        let mut key_salt = Vec::new();
        let mut key_bits = 256usize;

        let mut pwd_salt = Vec::new();
        let mut pwd_spin = 100_000u32;
        let mut enc_verifier_input = Vec::new();
        let mut enc_verifier_value = Vec::new();
        let mut enc_key_value = Vec::new();

        loop {
            match reader.read_event() {
                Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                    let name = e.local_name();
                    if name.as_ref() == b"keyData" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"saltValue" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    key_salt = decode_base64(s.trim()).unwrap_or_default();
                                }
                            } else if let (b"keyBits", Ok(s)) =
                                (attr.key.as_ref(), std::str::from_utf8(&attr.value))
                            {
                                key_bits = s.trim().parse().unwrap_or(256);
                            }
                        }
                    } else if name.as_ref() == b"encryptedKey" {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"saltValue" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    pwd_salt = decode_base64(s.trim()).unwrap_or_default();
                                }
                            } else if attr.key.as_ref() == b"spinCount" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    pwd_spin = s.trim().parse().unwrap_or(100_000);
                                }
                            } else if attr.key.as_ref() == b"encryptedVerifierHashInput" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    enc_verifier_input =
                                        decode_base64(s.trim()).unwrap_or_default();
                                }
                            } else if attr.key.as_ref() == b"encryptedVerifierHashValue" {
                                if let Ok(s) = std::str::from_utf8(&attr.value) {
                                    enc_verifier_value =
                                        decode_base64(s.trim()).unwrap_or_default();
                                }
                            } else if let (b"encryptedKeyValue", Ok(s)) =
                                (attr.key.as_ref(), std::str::from_utf8(&attr.value))
                            {
                                enc_key_value = decode_base64(s.trim()).unwrap_or_default();
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(EncryptionError::MalformedEncryptionInfo(err.to_string())),
                _ => {}
            }
        }

        if key_salt.is_empty() || pwd_salt.is_empty() || enc_key_value.is_empty() {
            return Err(EncryptionError::MalformedEncryptionInfo(
                "missing required Agile attributes".into(),
            ));
        }

        // 1. Password hash derivation
        let mut pwd_utf16_bytes = Vec::new();
        for c in password.encode_utf16() {
            pwd_utf16_bytes.extend_from_slice(&c.to_le_bytes());
        }

        let mut hash_input = pwd_salt.clone();
        hash_input.extend_from_slice(&pwd_utf16_bytes);
        let mut current_hash = sha512(&hash_input).to_vec();

        for i in 0..pwd_spin {
            let mut iter_buf = i.to_le_bytes().to_vec();
            iter_buf.extend_from_slice(&current_hash);
            current_hash = sha512(&iter_buf).to_vec();
        }

        // 2. Derive key to decrypt encryptedKeyValue
        // MS-OFFCRYPTO 2.3.4.11: BlockKey for keyValue is 0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6
        let block_key: [u8; 8] = [0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6];
        let mut key_derive_input = current_hash.clone();
        key_derive_input.extend_from_slice(&block_key);
        let derived_aes_key_full = sha512(&key_derive_input);
        let key_bytes_len = key_bits / 8;
        let aes_key = &derived_aes_key_full[..key_bytes_len];

        // Decrypt package key
        let mut iv = [0u8; 16];
        let iv_len = pwd_salt.len().min(16);
        iv[..iv_len].copy_from_slice(&pwd_salt[..iv_len]);

        let package_key = aes_cbc_decrypt(aes_key, &iv, &enc_key_value)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        // 3. Optional verifier check
        if !enc_verifier_input.is_empty() && !enc_verifier_value.is_empty() {
            let verifier_block_key: [u8; 8] = [0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79];
            let mut v_input = current_hash;
            v_input.extend_from_slice(&verifier_block_key);
            let v_key_full = sha512(&v_input);
            let v_key = &v_key_full[..key_bytes_len];

            let verifier = aes_cbc_decrypt(v_key, &iv, &enc_verifier_input)
                .map_err(|_| EncryptionError::InvalidPassword)?;
            let expected_verifier_hash = aes_cbc_decrypt(v_key, &iv, &enc_verifier_value)
                .map_err(|_| EncryptionError::InvalidPassword)?;
            let actual_verifier_hash = sha512(&verifier);

            if !constant_time_eq(&actual_verifier_hash, &expected_verifier_hash) {
                return Err(EncryptionError::InvalidPassword);
            }
        }

        // 4. Decrypt package stream
        let mut pkg_iv = [0u8; 16];
        let pkg_iv_len = key_salt.len().min(16);
        pkg_iv[..pkg_iv_len].copy_from_slice(&key_salt[..pkg_iv_len]);

        // First 8 bytes of EncryptedPackage is the stream length in uint64
        if encrypted_package.len() < 8 {
            return Err(EncryptionError::DecryptionFailed(
                "encrypted package stream too short".into(),
            ));
        }
        let total_size = u64::from_le_bytes([
            encrypted_package[0],
            encrypted_package[1],
            encrypted_package[2],
            encrypted_package[3],
            encrypted_package[4],
            encrypted_package[5],
            encrypted_package[6],
            encrypted_package[7],
        ]) as usize;

        let ciphertext = &encrypted_package[8..];
        let mut decrypted = aes_cbc_decrypt(&package_key, &pkg_iv, ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        if decrypted.len() > total_size {
            decrypted.truncate(total_size);
        }

        Ok(decrypted)
    }

    /// Create an Agile-encrypted CFB container wrapping OPC package bytes.
    pub fn encrypt_agile(package_bytes: &[u8], password: &str) -> Result<Vec<u8>, EncryptionError> {
        let key_salt = [0x55u8; 16];
        let pwd_salt = [0xAAu8; 16];
        let pwd_spin = 1000u32; // reasonable spin count for test generation

        // 1. Generate package key (256-bit AES)
        let package_key = [0x77u8; 32];

        // 2. Derive key from password
        let mut pwd_utf16_bytes = Vec::new();
        for c in password.encode_utf16() {
            pwd_utf16_bytes.extend_from_slice(&c.to_le_bytes());
        }
        let mut hash_input = pwd_salt.to_vec();
        hash_input.extend_from_slice(&pwd_utf16_bytes);
        let mut current_hash = sha512(&hash_input).to_vec();

        for i in 0..pwd_spin {
            let mut iter_buf = i.to_le_bytes().to_vec();
            iter_buf.extend_from_slice(&current_hash);
            current_hash = sha512(&iter_buf).to_vec();
        }

        let block_key: [u8; 8] = [0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6];
        let mut key_derive_input = current_hash.clone();
        key_derive_input.extend_from_slice(&block_key);
        let derived_aes_key = sha512(&key_derive_input);
        let aes_key = &derived_aes_key[..32];

        let iv = pwd_salt;
        let enc_key_value = aes_cbc_encrypt(aes_key, &iv, &package_key)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        // Verifier
        let verifier_input = [0x12u8; 16];
        let verifier_hash = sha512(&verifier_input);
        let verifier_block_key: [u8; 8] = [0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79];
        let mut v_input = current_hash;
        v_input.extend_from_slice(&verifier_block_key);
        let v_key = &sha512(&v_input)[..32];
        let enc_verifier_input = aes_cbc_encrypt(v_key, &iv, &verifier_input)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;
        let enc_verifier_value = aes_cbc_encrypt(v_key, &iv, &verifier_hash)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        // Encrypt package stream
        let pkg_iv = key_salt;
        let encrypted_body = aes_cbc_encrypt(&package_key, &pkg_iv, package_bytes)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))?;

        let mut enc_package_stream = Vec::with_capacity(8 + encrypted_body.len());
        enc_package_stream.extend_from_slice(&(package_bytes.len() as u64).to_le_bytes());
        enc_package_stream.extend_from_slice(&encrypted_body);

        // Build XML
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
            <encryption xmlns=\"http://schemas.microsoft.com/office/2006/encryption\">\n\
              <keyData saltSize=\"16\" blockSize=\"16\" keyBits=\"256\" hashSize=\"64\" cipherAlgorithm=\"AES\" cipherChaining=\"ChainingModeCBC\" hashAlgorithm=\"SHA512\" saltValue=\"{}\"/>\n\
              <dataIntegrity hmacKey=\"\" hmacValue=\"\"/>\n\
              <keyEncryptors>\n\
                <keyEncryptor uri=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\">\n\
                  <p:encryptedKey xmlns:p=\"http://schemas.microsoft.com/office/2006/keyEncryptor/password\" spinCount=\"{}\" saltSize=\"16\" blockSize=\"16\" keyBits=\"256\" hashSize=\"64\" cipherAlgorithm=\"AES\" cipherChaining=\"ChainingModeCBC\" hashAlgorithm=\"SHA512\" saltValue=\"{}\" encryptedVerifierHashInput=\"{}\" encryptedVerifierHashValue=\"{}\" encryptedKeyValue=\"{}\"/>\n\
                </keyEncryptor>\n\
              </keyEncryptors>\n\
            </encryption>",
            encode_base64(&key_salt),
            pwd_spin,
            encode_base64(&pwd_salt),
            encode_base64(&enc_verifier_input),
            encode_base64(&enc_verifier_value),
            encode_base64(&enc_key_value)
        );

        let mut enc_info_stream = vec![4, 0, 4, 0, 0x40, 0, 0, 0]; // major 4, minor 4, flags
        enc_info_stream.extend_from_slice(xml.as_bytes());

        // Write to CFB
        let mut writer = CfbWriter::new();
        writer.add_stream("EncryptionInfo", enc_info_stream);
        writer.add_stream("EncryptedPackage", enc_package_stream);
        let cfb_bytes = writer.build()?;

        Ok(cfb_bytes)
    }
}

pub fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let idx0 = (b0 >> 2) as usize;
        let idx1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
        let idx2 = (((b1 & 0x0f) << 2) | (b2 >> 6)) as usize;
        let idx3 = (b2 & 0x3f) as usize;

        out.push(TABLE[idx0] as char);
        out.push(TABLE[idx1] as char);
        if chunk.len() > 1 {
            out.push(TABLE[idx2] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[idx3] as char);
        } else {
            out.push('=');
        }
    }
    out
}

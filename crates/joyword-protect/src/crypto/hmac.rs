//! HMAC implementation for SHA-256 and SHA-512 (RFC 2104 / RFC 4231).

use super::sha::{sha256, sha512};

pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let block_size = 64;
    let mut k_prime = [0u8; 64];

    if key.len() > block_size {
        let hash = sha256(key);
        k_prime[..32].copy_from_slice(&hash);
    } else {
        k_prime[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];

    for i in 0..64 {
        o_key_pad[i] ^= k_prime[i];
        i_key_pad[i] ^= k_prime[i];
    }

    let mut inner_input = Vec::with_capacity(64 + message.len());
    inner_input.extend_from_slice(&i_key_pad);
    inner_input.extend_from_slice(message);
    let inner_hash = sha256(&inner_input);

    let mut outer_input = Vec::with_capacity(64 + 32);
    outer_input.extend_from_slice(&o_key_pad);
    outer_input.extend_from_slice(&inner_hash);
    sha256(&outer_input)
}

pub fn hmac_sha512(key: &[u8], message: &[u8]) -> [u8; 64] {
    let block_size = 128;
    let mut k_prime = [0u8; 128];

    if key.len() > block_size {
        let hash = sha512(key);
        k_prime[..64].copy_from_slice(&hash);
    } else {
        k_prime[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; 128];
    let mut i_key_pad = [0x36u8; 128];

    for i in 0..128 {
        o_key_pad[i] ^= k_prime[i];
        i_key_pad[i] ^= k_prime[i];
    }

    let mut inner_input = Vec::with_capacity(128 + message.len());
    inner_input.extend_from_slice(&i_key_pad);
    inner_input.extend_from_slice(message);
    let inner_hash = sha512(&inner_input);

    let mut outer_input = Vec::with_capacity(128 + 64);
    outer_input.extend_from_slice(&o_key_pad);
    outer_input.extend_from_slice(&inner_hash);
    sha512(&outer_input)
}

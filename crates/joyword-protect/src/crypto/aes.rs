//! AES-128 and AES-256 (FIPS 197) in CBC mode with PKCS#7 padding.
//!
//! Standards: NIST FIPS 197. Clean-room pure Rust implementation.

const S_BOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const INV_S_BOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

const RCON: [u32; 11] = [
    0x00000000, 0x01000000, 0x02000000, 0x04000000, 0x08000000, 0x10000000, 0x20000000, 0x40000000,
    0x80000000, 0x1B000000, 0x36000000,
];

fn sub_word(w: u32) -> u32 {
    let b0 = S_BOX[((w >> 24) & 0xff) as usize] as u32;
    let b1 = S_BOX[((w >> 16) & 0xff) as usize] as u32;
    let b2 = S_BOX[((w >> 8) & 0xff) as usize] as u32;
    let b3 = S_BOX[(w & 0xff) as usize] as u32;
    (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
}

fn rot_word(w: u32) -> u32 {
    w.rotate_left(8)
}

fn xtime(a: u8) -> u8 {
    if a & 0x80 != 0 {
        (a << 1) ^ 0x1b
    } else {
        a << 1
    }
}

fn multiply(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    p
}

pub struct AesKey {
    round_keys: Vec<u32>,
    rounds: usize,
}

impl AesKey {
    pub fn new(key: &[u8]) -> Option<Self> {
        let (nk, nr) = match key.len() {
            16 => (4, 10),
            32 => (8, 14),
            _ => return None,
        };

        let mut w = vec![0u32; 4 * (nr + 1)];
        for i in 0..nk {
            w[i] = u32::from_be_bytes([key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]]);
        }

        for i in nk..4 * (nr + 1) {
            let mut temp = w[i - 1];
            if i % nk == 0 {
                temp = sub_word(rot_word(temp)) ^ RCON[i / nk];
            } else if nk > 6 && i % nk == 4 {
                temp = sub_word(temp);
            }
            w[i] = w[i - nk] ^ temp;
        }

        Some(Self {
            round_keys: w,
            rounds: nr,
        })
    }

    pub fn encrypt_block(&self, input: &[u8; 16]) -> [u8; 16] {
        let mut state = [0u8; 16];
        state.copy_from_slice(input);

        // Round 0: AddRoundKey
        self.add_round_key(&mut state, 0);

        for round in 1..self.rounds {
            self.sub_bytes(&mut state);
            self.shift_rows(&mut state);
            self.mix_columns(&mut state);
            self.add_round_key(&mut state, round);
        }

        self.sub_bytes(&mut state);
        self.shift_rows(&mut state);
        self.add_round_key(&mut state, self.rounds);

        state
    }

    pub fn decrypt_block(&self, input: &[u8; 16]) -> [u8; 16] {
        let mut state = [0u8; 16];
        state.copy_from_slice(input);

        self.add_round_key(&mut state, self.rounds);

        for round in (1..self.rounds).rev() {
            self.inv_shift_rows(&mut state);
            self.inv_sub_bytes(&mut state);
            self.add_round_key(&mut state, round);
            self.inv_mix_columns(&mut state);
        }

        self.inv_shift_rows(&mut state);
        self.inv_sub_bytes(&mut state);
        self.add_round_key(&mut state, 0);

        state
    }

    fn add_round_key(&self, state: &mut [u8; 16], round: usize) {
        for i in 0..4 {
            let rk = self.round_keys[round * 4 + i].to_be_bytes();
            for j in 0..4 {
                state[j * 4 + i] ^= rk[j];
            }
        }
    }

    fn sub_bytes(&self, state: &mut [u8; 16]) {
        for b in state.iter_mut() {
            *b = S_BOX[*b as usize];
        }
    }

    fn inv_sub_bytes(&self, state: &mut [u8; 16]) {
        for b in state.iter_mut() {
            *b = INV_S_BOX[*b as usize];
        }
    }

    fn shift_rows(&self, state: &mut [u8; 16]) {
        let mut temp = [0u8; 16];
        temp.copy_from_slice(state);
        // Row 0
        state[0] = temp[0];
        state[4] = temp[4];
        state[8] = temp[8];
        state[12] = temp[12];
        // Row 1
        state[1] = temp[5];
        state[5] = temp[9];
        state[9] = temp[13];
        state[13] = temp[1];
        // Row 2
        state[2] = temp[10];
        state[6] = temp[14];
        state[10] = temp[2];
        state[14] = temp[6];
        // Row 3
        state[3] = temp[15];
        state[7] = temp[3];
        state[11] = temp[7];
        state[15] = temp[11];
    }

    fn inv_shift_rows(&self, state: &mut [u8; 16]) {
        let mut temp = [0u8; 16];
        temp.copy_from_slice(state);
        // Row 0
        state[0] = temp[0];
        state[4] = temp[4];
        state[8] = temp[8];
        state[12] = temp[12];
        // Row 1
        state[1] = temp[13];
        state[5] = temp[1];
        state[9] = temp[5];
        state[13] = temp[9];
        // Row 2
        state[2] = temp[10];
        state[6] = temp[14];
        state[10] = temp[2];
        state[14] = temp[6];
        // Row 3
        state[3] = temp[7];
        state[7] = temp[11];
        state[11] = temp[15];
        state[15] = temp[3];
    }

    fn mix_columns(&self, state: &mut [u8; 16]) {
        for col in 0..4 {
            let base = col * 4;
            let s0 = state[base];
            let s1 = state[base + 1];
            let s2 = state[base + 2];
            let s3 = state[base + 3];

            state[base] = multiply(0x02, s0) ^ multiply(0x03, s1) ^ s2 ^ s3;
            state[base + 1] = s0 ^ multiply(0x02, s1) ^ multiply(0x03, s2) ^ s3;
            state[base + 2] = s0 ^ s1 ^ multiply(0x02, s2) ^ multiply(0x03, s3);
            state[base + 3] = multiply(0x03, s0) ^ s1 ^ s2 ^ multiply(0x02, s3);
        }
    }

    fn inv_mix_columns(&self, state: &mut [u8; 16]) {
        for col in 0..4 {
            let base = col * 4;
            let s0 = state[base];
            let s1 = state[base + 1];
            let s2 = state[base + 2];
            let s3 = state[base + 3];

            state[base] =
                multiply(0x0e, s0) ^ multiply(0x0b, s1) ^ multiply(0x0d, s2) ^ multiply(0x09, s3);
            state[base + 1] =
                multiply(0x09, s0) ^ multiply(0x0e, s1) ^ multiply(0x0b, s2) ^ multiply(0x0d, s3);
            state[base + 2] =
                multiply(0x0d, s0) ^ multiply(0x09, s1) ^ multiply(0x0e, s2) ^ multiply(0x0b, s3);
            state[base + 3] =
                multiply(0x0b, s0) ^ multiply(0x0d, s1) ^ multiply(0x09, s2) ^ multiply(0x0e, s3);
        }
    }
}

pub fn aes_cbc_decrypt(
    key: &[u8],
    iv: &[u8; 16],
    ciphertext: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if !ciphertext.len().is_multiple_of(16) {
        return Err("ciphertext length is not a multiple of 16");
    }
    let aes = AesKey::new(key).ok_or("invalid key length for AES")?;
    let mut prev = *iv;
    let mut plaintext = Vec::with_capacity(ciphertext.len());

    let (chunks, _) = ciphertext.as_chunks::<16>();
    for chunk in chunks {
        let dec = aes.decrypt_block(chunk);
        let mut pt_block = [0u8; 16];
        for i in 0..16 {
            pt_block[i] = dec[i] ^ prev[i];
        }
        plaintext.extend_from_slice(&pt_block);
        prev = *chunk;
    }

    // Strip PKCS#7 padding if present and valid
    if let Some(&pad_len) = plaintext.last() {
        let pad = pad_len as usize;
        if pad > 0 && pad <= 16 && plaintext.len() >= pad {
            let is_valid = plaintext[plaintext.len() - pad..]
                .iter()
                .all(|&b| b == pad_len);
            if is_valid {
                plaintext.truncate(plaintext.len() - pad);
            }
        }
    }

    Ok(plaintext)
}

pub fn aes_cbc_encrypt(
    key: &[u8],
    iv: &[u8; 16],
    plaintext: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let aes = AesKey::new(key).ok_or("invalid key length for AES")?;
    let pad_len = 16 - (plaintext.len() % 16);
    let mut padded = plaintext.to_vec();
    padded.extend(std::iter::repeat_n(pad_len as u8, pad_len));

    let mut prev = *iv;
    let mut ciphertext = Vec::with_capacity(padded.len());

    let (chunks, _) = padded.as_chunks::<16>();
    for chunk in chunks {
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ prev[i];
        }
        let enc = aes.encrypt_block(&block);
        ciphertext.extend_from_slice(&enc);
        prev = enc;
    }

    Ok(ciphertext)
}

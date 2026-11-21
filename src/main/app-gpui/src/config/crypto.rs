const SBOX: [u8; 256] = [
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

const INV_SBOX: [u8; 256] = [
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

const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

fn xtime(x: u8) -> u8 {
    (x << 1) ^ if x & 0x80 != 0 { 0x1b } else { 0x00 }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut result = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            result ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    result
}

fn expand_key(key: &[u8]) -> Option<(Vec<[u8; 16]>, usize)> {
    let nk = key.len() / 4;
    let nr = match key.len() {
        16 => 10,
        32 => 14,
        _ => return None,
    };
    let total_words = 4 * (nr + 1);
    let mut words = vec![[0u8; 4]; total_words];
    for (index, chunk) in key.as_chunks::<4>().0.iter().enumerate() {
        words[index] = *chunk;
    }
    for i in nk..total_words {
        let mut temp = words[i - 1];
        if i % nk == 0 {
            temp = [
                SBOX[temp[1] as usize] ^ RCON[i / nk - 1],
                SBOX[temp[2] as usize],
                SBOX[temp[3] as usize],
                SBOX[temp[0] as usize],
            ];
        } else if nk > 6 && i % nk == 4 {
            for byte in &mut temp {
                *byte = SBOX[*byte as usize];
            }
        }
        for j in 0..4 {
            words[i][j] = words[i - nk][j] ^ temp[j];
        }
    }
    let mut round_keys = Vec::with_capacity(nr + 1);
    for r in 0..=nr {
        let mut rk = [0u8; 16];
        for c in 0..4 {
            rk[c * 4..c * 4 + 4].copy_from_slice(&words[r * 4 + c]);
        }
        round_keys.push(rk);
    }
    Some((round_keys, nr))
}

fn add_round_key(state: &mut [u8; 16], rk: &[u8; 16]) {
    for (s, k) in state.iter_mut().zip(rk.iter()) {
        *s ^= *k;
    }
}

fn encrypt_block(mut state: [u8; 16], round_keys: &[[u8; 16]], nr: usize) -> [u8; 16] {
    add_round_key(&mut state, &round_keys[0]);
    for (round, round_key) in round_keys.iter().enumerate().take(nr + 1).skip(1) {
        for byte in &mut state {
            *byte = SBOX[*byte as usize];
        }
        state = [
            state[0], state[5], state[10], state[15], state[4], state[9], state[14], state[3],
            state[8], state[13], state[2], state[7], state[12], state[1], state[6], state[11],
        ];
        if round < nr {
            for c in 0..4 {
                let i = c * 4;
                let (s0, s1, s2, s3) = (state[i], state[i + 1], state[i + 2], state[i + 3]);
                let t = s0 ^ s1 ^ s2 ^ s3;
                state[i] ^= t ^ xtime(s0 ^ s1);
                state[i + 1] ^= t ^ xtime(s1 ^ s2);
                state[i + 2] ^= t ^ xtime(s2 ^ s3);
                state[i + 3] ^= t ^ xtime(s3 ^ s0);
            }
        }
        add_round_key(&mut state, round_key);
    }
    state
}

fn decrypt_block(mut state: [u8; 16], round_keys: &[[u8; 16]], nr: usize) -> [u8; 16] {
    add_round_key(&mut state, &round_keys[nr]);
    for round in (0..nr).rev() {
        state = [
            state[0], state[13], state[10], state[7], state[4], state[1], state[14], state[11],
            state[8], state[5], state[2], state[15], state[12], state[9], state[6], state[3],
        ];
        for byte in &mut state {
            *byte = INV_SBOX[*byte as usize];
        }
        add_round_key(&mut state, &round_keys[round]);
        if round > 0 {
            for c in 0..4 {
                let i = c * 4;
                let (s0, s1, s2, s3) = (state[i], state[i + 1], state[i + 2], state[i + 3]);
                state[i] =
                    gf_mul(s0, 0x0e) ^ gf_mul(s1, 0x0b) ^ gf_mul(s2, 0x0d) ^ gf_mul(s3, 0x09);
                state[i + 1] =
                    gf_mul(s0, 0x09) ^ gf_mul(s1, 0x0e) ^ gf_mul(s2, 0x0b) ^ gf_mul(s3, 0x0d);
                state[i + 2] =
                    gf_mul(s0, 0x0d) ^ gf_mul(s1, 0x09) ^ gf_mul(s2, 0x0e) ^ gf_mul(s3, 0x0b);
                state[i + 3] =
                    gf_mul(s0, 0x0b) ^ gf_mul(s1, 0x0d) ^ gf_mul(s2, 0x09) ^ gf_mul(s3, 0x0e);
            }
        }
    }
    state
}

pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8; 16], plaintext: &[u8]) -> Option<Vec<u8>> {
    let (round_keys, nr) = expand_key(key)?;
    let pad_len = 16 - (plaintext.len() % 16);
    let mut padded = Vec::with_capacity(plaintext.len() + pad_len);
    padded.extend_from_slice(plaintext);
    padded.resize(plaintext.len() + pad_len, pad_len as u8);

    let mut out = Vec::with_capacity(padded.len());
    let mut prev = *iv;
    for chunk in padded.as_chunks::<16>().0 {
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ prev[i];
        }
        prev = encrypt_block(block, &round_keys, nr);
        out.extend_from_slice(&prev);
    }
    Some(out)
}

pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8; 16], ciphertext: &[u8]) -> Option<Vec<u8>> {
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
        return None;
    }
    let (round_keys, nr) = expand_key(key)?;
    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev = *iv;
    for chunk in ciphertext.as_chunks::<16>().0 {
        let block = *chunk;
        let decrypted = decrypt_block(block, &round_keys, nr);
        for i in 0..16 {
            out.push(decrypted[i] ^ prev[i]);
        }
        prev = block;
    }
    let pad = *out.last()? as usize;
    if pad == 0 || pad > 16 || pad > out.len() {
        return None;
    }
    let start = out.len() - pad;
    if out[start..].iter().any(|&b| b as usize != pad) {
        return None;
    }
    out.truncate(start);
    Some(out)
}

#[cfg(any(target_os = "linux", test))]
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (slot, bytes) in w[..16].iter_mut().zip(chunk.as_chunks::<4>().0) {
            *slot = u32::from_be_bytes(*bytes);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc_u32),
                _ => (b ^ c ^ d, 0xca62_c1d6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut digest = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

#[cfg(any(target_os = "linux", test))]
fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let mut block_key = [0u8; 64];
    if key.len() > 64 {
        let hashed = sha1(key);
        block_key[..20].copy_from_slice(&hashed);
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }
    let mut inner = Vec::with_capacity(64 + data.len());
    for byte in block_key {
        inner.push(byte ^ 0x36);
    }
    inner.extend_from_slice(data);
    let inner_hash = sha1(&inner);

    let mut outer = Vec::with_capacity(64 + 20);
    for byte in block_key {
        outer.push(byte ^ 0x5c);
    }
    outer.extend_from_slice(&inner_hash);
    sha1(&outer)
}

#[cfg(any(target_os = "linux", test))]
pub fn pbkdf2_hmac_sha1(password: &[u8], salt: &[u8], iterations: u32, output: &mut [u8]) {
    let mut block_index = 1u32;
    let mut written = 0usize;
    while written < output.len() {
        let mut salt_block = Vec::with_capacity(salt.len() + 4);
        salt_block.extend_from_slice(salt);
        salt_block.extend_from_slice(&block_index.to_be_bytes());
        let mut u = hmac_sha1(password, &salt_block);
        let mut t = u;
        for _ in 1..iterations {
            u = hmac_sha1(password, &u);
            for (dst, src) in t.iter_mut().zip(u.iter()) {
                *dst ^= *src;
            }
        }
        let take = (output.len() - written).min(t.len());
        output[written..written + take].copy_from_slice(&t[..take]);
        written += take;
        block_index = block_index.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_fips_197_aes_128_and_256_vectors() {
        let key128 = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let pt = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let (rk, nr) = expand_key(&key128).unwrap();
        let ct = encrypt_block(pt, &rk, nr);
        assert_eq!(
            ct,
            [
                0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4,
                0xc5, 0x5a,
            ]
        );
        assert_eq!(decrypt_block(ct, &rk, nr), pt);

        let key256 = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let (rk256, nr256) = expand_key(&key256).unwrap();
        let ct256 = encrypt_block(pt, &rk256, nr256);
        assert_eq!(
            ct256,
            [
                0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
                0x60, 0x89,
            ]
        );
        assert_eq!(decrypt_block(ct256, &rk256, nr256), pt);
    }

    #[test]
    fn matches_rfc_6070_pbkdf2_hmac_sha1_vectors() {
        let mut out = [0u8; 20];
        pbkdf2_hmac_sha1(b"password", b"salt", 1, &mut out);
        assert_eq!(
            out,
            [
                0x0c, 0x60, 0xc8, 0x0f, 0x96, 0x1f, 0x0e, 0x71, 0xf3, 0xa9, 0xb5, 0x24, 0xaf, 0x60,
                0x12, 0x06, 0x2f, 0xe0, 0x37, 0xa6,
            ]
        );

        pbkdf2_hmac_sha1(b"password", b"salt", 2, &mut out);
        assert_eq!(
            out,
            [
                0xea, 0x6c, 0x01, 0x4d, 0xc7, 0x2d, 0x6f, 0x8c, 0xcd, 0x1e, 0xd9, 0x2a, 0xce, 0x1d,
                0x41, 0xf0, 0xd8, 0xde, 0x89, 0x57,
            ]
        );
    }

    #[test]
    fn round_trips_aes_cbc_pkcs7_padding() {
        let key = [0x42u8; 32];
        let iv = [0x24u8; 16];
        for len in [0usize, 1, 15, 16, 17, 48] {
            let plain: Vec<u8> = (0..len).map(|i| (i * 13 % 251) as u8).collect();
            let encrypted = aes_cbc_encrypt(&key, &iv, &plain).unwrap();
            let decrypted = aes_cbc_decrypt(&key, &iv, &encrypted).unwrap();
            assert_eq!(decrypted, plain);
        }
    }
}

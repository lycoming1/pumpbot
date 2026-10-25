//! Base58 senza allocazioni nell'hot path (five8).

use crate::types::{Pubkey, Sig};

pub fn decode32(s: &str) -> Option<Pubkey> {
    let mut out = [0u8; 32];
    five8::decode_32(s, &mut out).ok()?;
    Some(out)
}

pub fn decode64(s: &str) -> Option<Sig> {
    let mut out = [0u8; 64];
    five8::decode_64(s, &mut out).ok()?;
    Some(out)
}

pub fn encode32(k: &Pubkey) -> String {
    let mut buf = [0u8; five8::BASE58_ENCODED_32_MAX_LEN];
    let n = five8::encode_32(k, &mut buf) as usize;
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

pub fn encode64(k: &Sig) -> String {
    let mut buf = [0u8; five8::BASE58_ENCODED_64_MAX_LEN];
    let n = five8::encode_64(k, &mut buf) as usize;
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// Chiave per la deduplica delle firme: primi 8 byte decodificati.
pub fn sig_key(sig: &Sig) -> u64 {
    u64::from_le_bytes(sig[..8].try_into().unwrap())
}

/// Forma corta per i log.
pub fn short(k: &Pubkey) -> String {
    let s = encode32(k);
    s.chars().take(8).collect()
}

//! Deterministic ids for structures that GEDCOM stores inline (places, names, inline notes...).
//!
//! Round trips must give the same ids, and inline structures have no `_GTID` line, so their id is
//! derived from their owner's id and position. The result is shaped like a UUID v7.

use uuid::Uuid;

fn fnv(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^= h >> 29;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^ (h >> 32)
}

/// Deterministic UUID from arbitrary bytes.
pub(crate) fn det_id(bytes: &[u8]) -> Uuid {
    let a = fnv(0xcbf2_9ce4_8422_2325, bytes);
    let b = fnv(0x8422_2325_cbf2_9ce4, bytes);
    let mut raw = [0u8; 16];
    raw[..8].copy_from_slice(&a.to_be_bytes());
    raw[8..].copy_from_slice(&b.to_be_bytes());
    raw[6] = (raw[6] & 0x0f) | 0x70;
    raw[8] = (raw[8] & 0x3f) | 0x80;
    Uuid::from_bytes(raw)
}

/// Id of the `idx`-th child of kind `kind` owned by `owner`.
pub(crate) fn derive_id(owner: Uuid, kind: &str, idx: usize) -> Uuid {
    let mut v = owner.as_bytes().to_vec();
    v.extend_from_slice(kind.as_bytes());
    v.extend_from_slice(&(idx as u64).to_be_bytes());
    det_id(&v)
}

/// Place id for a chain text such as `Berlin, Germany`.
pub(crate) fn place_id(chain: &str) -> Uuid {
    let mut v = b"place:".to_vec();
    v.extend_from_slice(chain.as_bytes());
    det_id(&v)
}

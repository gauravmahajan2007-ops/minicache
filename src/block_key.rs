//! `block_key` — a deterministic, stable hash of a token span.
//!
//! FNV-1a 64-bit over the little-endian bytes of each token, in order. This is
//! the analog of TensorRT-LLM's `makeKey`: a cross-language contract used by a
//! KV-cache-aware router to route a request to the worker holding its prefix.
//!
//! The exact constants are part of the contract (SPEC_PREFIX.md §4) and are
//! locked by known-vector tests.

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x100000001b3;

/// Hash a token span into a stable `u64` key.
///
/// Deterministic and stable: the same span always yields the same key,
/// byte-for-byte, across processes and languages.
pub fn block_key(tokens: &[u32]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &t in tokens {
        for b in t.to_le_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

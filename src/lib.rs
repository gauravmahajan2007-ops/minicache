//! MiniCache — a minimal in-memory TTL key-value cache with a simulated clock.
//!
//! Deliberately small: no networking, persistence, distribution, replication,
//! external databases, background threads, or heavy dependencies. Time is
//! supplied by [`SimulatedClock`], so tests are deterministic and instant.
//!
//! The authoritative behavioral contract lives in `SPEC.md`; this module is one
//! concrete (Rust) realization of it.
//!
//! The `BlockManager` (see `SPEC_RETENTION.md`) extends this foundation toward
//! NVIDIA KV-cache retention/eviction semantics.

pub mod block_key;
pub mod block_manager;
pub mod prefix_index;
pub mod retention;

pub use block_key::block_key;
pub use block_manager::{BlockId, BlockManager, BlockState};
pub use prefix_index::PrefixIndex;
pub use retention::{Retention, RetentionError, DEFAULT_PRIORITY, MAX_PRIORITY};

use std::collections::HashMap;
use std::hash::Hash;

/// Errors returned by cache and clock operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Error {
    /// A TTL was not a finite, strictly positive number.
    InvalidTtl(f64),
    /// A clock advance was not a finite, non-negative number.
    InvalidAdvance(f64),
}

/// A deterministic, monotonic time source.
///
/// Starts at `0.0` (or a caller-supplied start) and only moves forward via
/// [`advance`](Self::advance). No real time is read anywhere.
#[derive(Debug, Clone)]
pub struct SimulatedClock {
    now: f64,
}

impl SimulatedClock {
    pub fn new() -> Self {
        Self { now: 0.0 }
    }

    pub fn at(start: f64) -> Self {
        Self { now: start }
    }

    pub fn now(&self) -> f64 {
        self.now
    }

    /// Advance time by `delta` (finite, non-negative).
    ///
    /// Returns `Err(Error::InvalidAdvance)` for a negative or non-finite delta:
    /// time cannot move backward.
    pub fn advance(&mut self, delta: f64) -> Result<f64, Error> {
        if !delta.is_finite() || delta < 0.0 {
            return Err(Error::InvalidAdvance(delta));
        }
        self.now += delta;
        Ok(self.now)
    }
}

impl Default for SimulatedClock {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_ttl(ttl: f64) -> Result<f64, Error> {
    if !ttl.is_finite() || ttl <= 0.0 {
        Err(Error::InvalidTtl(ttl))
    } else {
        Ok(ttl)
    }
}

/// An in-memory TTL key-value cache. See `SPEC.md` for the authoritative
/// semantics.
///
/// * `put(key, value, ttl)` — store, expiring at `now + ttl`; overwrite replaces
///   both value and expiry (TTL reset); invalid TTL returns `Err` and leaves
///   state unchanged.
/// * `get(key)` — `Some(&value)` while `now < expiry`, else `None`; expired
///   entries are evicted lazily.
/// * `delete(key)` — remove if present; missing-key delete is a no-op.
/// * `size()` — number of live entries (does not eagerly evict).
#[derive(Debug, Clone)]
pub struct MiniCache<K, V> {
    clock: SimulatedClock,
    // key -> (value, absolute expiry time)
    store: HashMap<K, (V, f64)>,
}

impl<K, V> MiniCache<K, V>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self::with_clock(SimulatedClock::new())
    }

    pub fn with_clock(clock: SimulatedClock) -> Self {
        Self {
            clock,
            store: HashMap::new(),
        }
    }

    pub fn clock(&self) -> &SimulatedClock {
        &self.clock
    }

    pub fn clock_mut(&mut self) -> &mut SimulatedClock {
        &mut self.clock
    }

    pub fn put(&mut self, key: K, value: V, ttl: f64) -> Result<(), Error> {
        let ttl = validate_ttl(ttl)?;
        let expiry = self.clock.now() + ttl;
        self.store.insert(key, (value, expiry));
        Ok(())
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let now = self.clock.now();
        // Copy the expiry out so we don't hold an immutable borrow across the
        // mutable `remove` below.
        let expiry = self.store.get(key).map(|(_, e)| *e);
        match expiry {
            None => None,
            Some(e) if now >= e => {
                self.store.remove(key);
                None
            }
            Some(_) => self.store.get(key).map(|(v, _)| v),
        }
    }

    pub fn delete(&mut self, key: &K) {
        self.store.remove(key);
    }

    pub fn size(&self) -> usize {
        let now = self.clock.now();
        self.store
            .values()
            .filter(|(_, expiry)| now < *expiry)
            .count()
    }
}

impl<K: Eq + Hash, V> Default for MiniCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

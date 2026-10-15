//! Test-only support module: an independent reference model (oracle) and a
//! deliberately faulty fixture for the repair-workflow demonstration.
//!
//! Nothing here is compiled into the library; it exists only for the
//! integration tests under `tests/`.
//!
//! Different test binaries use different subsets of this module, so dead-code
//! warnings are expected and silenced here.
#![allow(dead_code)]

pub mod block_manager_ref;
pub mod prefix_ref;

use std::collections::HashMap;
use std::hash::Hash;

/// Independent reference clock, written from `SPEC.md`, not the implementation.
pub struct ReferenceClock {
    pub t: f64,
}

impl ReferenceClock {
    pub fn new() -> Self {
        Self { t: 0.0 }
    }

    pub fn now(&self) -> f64 {
        self.t
    }

    pub fn advance(&mut self, delta: f64) -> Result<f64, ()> {
        if !delta.is_finite() || delta < 0.0 {
            return Err(());
        }
        self.t += delta;
        Ok(self.t)
    }
}

/// Independent reference cache (oracle), written directly from the spec with no
/// reference to the implementation. Uses `Option<&V>` for "live value" vs
/// "absent/expired", mirroring the implementation's contract.
pub struct ReferenceCache<K, V> {
    pub clock: ReferenceClock,
    entries: HashMap<K, (V, f64)>,
}

impl<K: Eq + Hash, V> ReferenceCache<K, V> {
    pub fn new() -> Self {
        Self {
            clock: ReferenceClock::new(),
            entries: HashMap::new(),
        }
    }

    pub fn put(&mut self, key: K, value: V, ttl: f64) -> Result<(), ()> {
        if !ttl.is_finite() || ttl <= 0.0 {
            return Err(());
        }
        let expires = self.clock.now() + ttl;
        self.entries.insert(key, (value, expires));
        Ok(())
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let expires = match self.entries.get(key) {
            Some((_, e)) => *e,
            None => return None,
        };
        if self.clock.now() >= expires {
            self.entries.remove(key);
            return None;
        }
        self.entries.get(key).map(|(v, _)| v)
    }

    pub fn delete(&mut self, key: &K) {
        self.entries.remove(key);
    }

    pub fn size(&self) -> usize {
        let now = self.clock.now();
        self.entries.values().filter(|(_, e)| now < *e).count()
    }
}

/// Deliberately faulty cache for the repair-workflow demonstration.
///
/// Identical to `MiniCache` except the expiry comparison uses `>` instead of
/// `>=`, so an entry remains readable *at exactly* its expiry time — violating
/// `SPEC.md` §3.2 ("a key cannot be read at or after its expiry time").
///
/// This fixture must never be imported by production code; it exists solely to
/// prove the test suite catches this class of boundary defect.
pub struct FaultyCache<K, V> {
    clock: minicache::SimulatedClock,
    store: HashMap<K, (V, f64)>,
}

impl<K: Eq + Hash, V> FaultyCache<K, V> {
    pub fn new() -> Self {
        Self {
            clock: minicache::SimulatedClock::new(),
            store: HashMap::new(),
        }
    }

    pub fn put(&mut self, key: K, value: V, ttl: f64) -> Result<(), ()> {
        if !ttl.is_finite() || ttl <= 0.0 {
            return Err(());
        }
        let exp = self.clock.now() + ttl;
        self.store.insert(key, (value, exp));
        Ok(())
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let exp = match self.store.get(key) {
            Some((_, e)) => *e,
            None => return None,
        };
        if self.clock.now() > exp {
            // ^^^ BUG: should be `>=`
            self.store.remove(key);
            return None;
        }
        self.store.get(key).map(|(v, _)| v)
    }

    pub fn clock_mut(&mut self) -> &mut minicache::SimulatedClock {
        &mut self.clock
    }
}

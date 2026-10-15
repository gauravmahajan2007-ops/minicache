//! Ordinary unit tests for MiniCache — explicit coverage of the six required
//! properties plus invalid-TTL handling and clock behavior.
//!
//! Deterministic: every test drives a `SimulatedClock` and never real sleep.

use minicache::{Error, MiniCache, SimulatedClock};

fn cache() -> MiniCache<String, i64> {
    MiniCache::new()
}

fn key(s: &str) -> String {
    s.to_string()
}

#[test]
fn key_unreadable_at_expiry() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap();
    c.clock_mut().advance(10.0).unwrap(); // exactly at expiry
    assert_eq!(c.get(&key("k")), None);
}

#[test]
fn key_unreadable_well_after_expiry() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap();
    c.clock_mut().advance(10.0).unwrap();
    c.clock_mut().advance(1000.0).unwrap();
    assert_eq!(c.get(&key("k")), None);
}

#[test]
fn key_readable_before_expiry() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap();
    c.clock_mut().advance(5.0).unwrap();
    assert_eq!(c.get(&key("k")), Some(&1));
    c.clock_mut().advance(4.999).unwrap(); // 9.999 < 10
    assert_eq!(c.get(&key("k")), Some(&1));
}

#[test]
fn delete_makes_unavailable() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap();
    c.delete(&key("k"));
    assert_eq!(c.get(&key("k")), None);
    assert_eq!(c.size(), 0);
}

#[test]
fn delete_missing_safe_and_repeatable() {
    let mut c = cache();
    c.delete(&key("ghost"));
    c.delete(&key("ghost"));
    c.delete(&key("ghost"));
    assert_eq!(c.get(&key("ghost")), None);
    // after expiry, delete is still a safe no-op
    c.put(key("k"), 1, 1.0).unwrap();
    c.clock_mut().advance(1.0).unwrap();
    c.delete(&key("k"));
    c.delete(&key("k"));
    assert_eq!(c.get(&key("k")), None);
}

#[test]
fn overwrite_resets_ttl() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap(); // expiry 10
    c.clock_mut().advance(5.0).unwrap();
    c.put(key("k"), 2, 10.0).unwrap(); // expiry reset to 15
    c.clock_mut().advance(4.999).unwrap(); // t = 9.999, expiry 15
    assert_eq!(c.get(&key("k")), Some(&2));
    c.clock_mut().advance(0.001).unwrap(); // t = 10.0 (old expiry would be gone)
    assert_eq!(c.get(&key("k")), Some(&2)); // still live: proves reset, not max/inherit
}

#[test]
fn overwrite_replaces_value() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap();
    c.put(key("k"), 2, 10.0).unwrap();
    assert_eq!(c.get(&key("k")), Some(&2));
}

#[test]
fn lazy_eviction_on_get() {
    let mut c = cache();
    c.put(key("k"), 1, 10.0).unwrap();
    c.clock_mut().advance(10.0).unwrap();
    assert_eq!(c.get(&key("k")), None);
    assert_eq!(c.size(), 0); // entry actually removed
}

#[test]
fn size_counts_only_live() {
    let mut c = cache();
    c.put(key("a"), 1, 10.0).unwrap();
    c.put(key("b"), 2, 100.0).unwrap();
    assert_eq!(c.size(), 2);
    c.clock_mut().advance(10.0).unwrap(); // "a" expired, "b" live
    assert_eq!(c.size(), 1);
}

#[test]
fn invalid_ttl_rejected_and_noop() {
    let mut c = cache();
    c.put(key("a"), 1, 10.0).unwrap(); // baseline
    for bad in [0.0, -1.0, -0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(c.put(key("k"), 1, bad).is_err(), "ttl {bad} should be rejected");
    }
    assert_eq!(c.get(&key("k")), None); // nothing stored
    assert_eq!(c.size(), 1); // baseline untouched
    // precise variant for a finite invalid value (avoid NaN != NaN)
    assert!(matches!(c.put(key("k"), 1, -1.0), Err(Error::InvalidTtl(x)) if x == -1.0));
}

#[test]
fn clock_monotonic_and_validation() {
    let mut clk = SimulatedClock::new();
    assert_eq!(clk.now(), 0.0);
    clk.advance(1.5).unwrap();
    assert_eq!(clk.now(), 1.5);
    assert!(clk.advance(-1.0).is_err());
    assert!(clk.advance(f64::NAN).is_err());
    assert!(clk.advance(f64::INFINITY).is_err());
    assert_eq!(clk.now(), 1.5); // unchanged after rejected advances
    assert!(matches!(clk.advance(-1.0), Err(Error::InvalidAdvance(x)) if x == -1.0));
}

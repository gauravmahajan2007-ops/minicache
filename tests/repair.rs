//! Repair-workflow demonstration and regression guard.
//!
//! The permanent regression test (`expiry_boundary_regression`) runs against the
//! REAL implementation and must always pass. The deliberately faulty fixture
//! (`common::FaultyCache`) proves the same boundary check is capable of catching
//! an off-by-one expiry bug.

use minicache::MiniCache;
use std::path::PathBuf;

mod common;

fn boundary_check_impl() -> Option<i64> {
    let mut c = MiniCache::<String, i64>::new();
    c.put("k".to_string(), 1, 10.0).unwrap();
    c.clock_mut().advance(10.0).unwrap();
    c.get(&"k".to_string()).copied()
}

fn boundary_check_faulty() -> Option<i64> {
    let mut c = common::FaultyCache::<String, i64>::new();
    c.put("k".to_string(), 1, 10.0).unwrap();
    c.clock_mut().advance(10.0).unwrap();
    c.get(&"k".to_string()).copied()
}

#[test]
fn expiry_boundary_regression() {
    // permanent guard: a key is unreadable at exactly its expiry time
    assert_eq!(boundary_check_impl(), None);
}

#[test]
fn faulty_fixture_exhibits_the_defect() {
    // document that the faulty fixture IS detectable: it returns the value at
    // expiry (the buggy behavior), differing from the correct None
    assert_eq!(boundary_check_faulty(), Some(1));
}

#[test]
fn saved_counterexample_reproduces_and_now_passes() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("counterexamples/boundary-off-by-one.json");
    let raw = std::fs::read_to_string(&path).expect("counterexample committed");
    assert!(raw.contains("expiry comparison used '>' instead of '>='"));
    assert!(raw.contains("\"op\": \"get\""));
    // after the fix, the real implementation satisfies the counterexample
    assert_eq!(boundary_check_impl(), None);
}

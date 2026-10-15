//! Repair-workflow demonstration and regression guard for `PrefixIndex`.
//!
//! The permanent regression test (`longest_prefix_chain_regression`) runs against
//! the REAL implementation and must always pass. The deliberately faulty fixture
//! (`common::prefix_ref::FaultyPrefixIndex`) proves the boundary check catches the
//! "drops shallower blocks" defect.

use minicache::PrefixIndex;
use std::path::PathBuf;

mod common;

fn longest_prefix_impl() -> (usize, Vec<u32>) {
    let mut p = PrefixIndex::new();
    p.insert(&[1], 10);
    p.insert(&[1, 2], 20);
    p.longest_prefix(&[1, 2, 3])
}

fn longest_prefix_faulty() -> (usize, Vec<u32>) {
    let mut p = common::prefix_ref::FaultyPrefixIndex::new();
    p.insert(&[1], 10);
    p.insert(&[1, 2], 20);
    p.longest_prefix(&[1, 2, 3])
}

#[test]
fn longest_prefix_chain_regression() {
    // permanent guard: full chain returned in root-to-leaf order
    assert_eq!(longest_prefix_impl(), (2, vec![10, 20]));
}

#[test]
fn faulty_fixture_exhibits_the_defect() {
    // the faulty fixture drops the shallower block (the bug)
    assert_eq!(longest_prefix_faulty(), (2, vec![20]));
}

#[test]
fn saved_counterexample_reproduces_and_now_passes() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("counterexamples/prefix-chain-drop.json");
    let raw = std::fs::read_to_string(&path).expect("counterexample committed");
    assert!(raw.contains("longest_prefix dropped shallower blocks"));
    assert_eq!(longest_prefix_impl(), (2, vec![10, 20]));
}

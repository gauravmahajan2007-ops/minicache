//! Repair-workflow demonstration and regression guard for the BlockManager.
//!
//! The permanent regression test (`duration_revert_boundary_regression`) runs
//! against the REAL implementation and must always pass. The deliberately faulty
//! fixture (`common::block_manager_ref::FaultyBlockManager`) proves the boundary
//! check catches the off-by-one duration-revert defect.

use minicache::{BlockManager, Retention, SimulatedClock, DEFAULT_PRIORITY};
use std::path::PathBuf;

mod common;

fn boundary_check_impl() -> Option<u8> {
    let mut bm = BlockManager::with_capacity(2);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    bm.clock_mut().advance(10.0).unwrap(); // exactly at expiry
    bm.effective_priority(id)
}

fn boundary_check_faulty() -> Option<u8> {
    let mut bm = common::block_manager_ref::FaultyBlockManager::new(2, SimulatedClock::new());
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    bm.clock_mut().advance(10.0).unwrap();
    bm.effective_priority(id)
}

#[test]
fn duration_revert_boundary_regression() {
    // permanent guard: priority reverts to default at exactly expiry
    assert_eq!(boundary_check_impl(), Some(DEFAULT_PRIORITY));
}

#[test]
fn faulty_fixture_exhibits_the_defect() {
    // the faulty fixture returns the stale priority at expiry (the bug)
    assert_eq!(boundary_check_faulty(), Some(90));
}

#[test]
fn saved_counterexample_reproduces_and_now_passes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("counterexamples/duration-revert-off-by-one.json");
    let raw = std::fs::read_to_string(&path).expect("counterexample committed");
    assert!(raw.contains("duration revert used '>' instead of '>='"));
    assert_eq!(boundary_check_impl(), Some(DEFAULT_PRIORITY));
}

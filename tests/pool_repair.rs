//! Repair-workflow demonstration and regression guard for `CpuPool`.
//!
//! The permanent regression test (`clear_block_regression`) runs against the REAL
//! implementation. The deliberately faulty fixture (`common::pool_ref::FaultyCpuPool`)
//! proves the suite catches the "clear is a no-op" defect.

use minicache::{CpuPool, KvPool};
use std::path::PathBuf;

mod common;

fn clear_then_read_cpu() -> Vec<u8> {
    let mut p = CpuPool::new(4, 4);
    p.write_block(0, &[1, 2, 3, 4]).unwrap();
    p.clear_block(0).unwrap();
    let mut buf = [0u8; 4];
    p.read_block(0, &mut buf).unwrap();
    buf.to_vec()
}

fn clear_then_read_faulty() -> Vec<u8> {
    let mut p = common::pool_ref::FaultyCpuPool::new(4, 4);
    p.write_block(0, &[1, 2, 3, 4]).unwrap();
    p.clear_block(0).unwrap();
    let mut buf = [0u8; 4];
    p.read_block(0, &mut buf).unwrap();
    buf.to_vec()
}

#[test]
fn clear_block_regression() {
    // permanent guard: clear zeroes the block
    assert_eq!(clear_then_read_cpu(), vec![0, 0, 0, 0]);
}

#[test]
fn faulty_fixture_exhibits_the_defect() {
    // the faulty fixture leaks stale data (clear is a no-op)
    assert_eq!(clear_then_read_faulty(), vec![1, 2, 3, 4]);
}

#[test]
fn saved_counterexample_reproduces_and_now_passes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("counterexamples/clear-noop.json");
    let raw = std::fs::read_to_string(&path).expect("counterexample committed");
    assert!(raw.contains("clear_block is a no-op"));
    assert_eq!(clear_then_read_cpu(), vec![0, 0, 0, 0]);
}

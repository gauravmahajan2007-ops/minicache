//! Switch-off verification for the BlockManager invariants without a dedicated
//! faulty fixture: prove each guard is load-bearing by demonstrating that a
//! model with that ONE guard removed (`SwitchFaulty`) diverges from the real
//! implementation in exactly the way the guard prevents.
//!
//! This is the Rust analog of the linux-formal "one fix OFF must fail" switch
//! style: I3 (LRU tie-break), I4 (never evict active), I6 (reuse resets
//! retention), I7 (invalidate idempotent). I2 already has its own faulty
//! fixture in `tests/retention_repair.rs`.

use minicache::{BlockManager, Retention, DEFAULT_PRIORITY};

mod common;
use common::block_manager_ref::{Defect, SwitchFaulty};

#[test]
fn switch_i3_lru_tiebreak() {
    // Three blocks, same priority, released 0 then 1 then 2 (same release order
    // for both). LRU is the first released (0); MRU is the last (2).
    let mut imp = BlockManager::with_capacity(3);
    imp.allocate().unwrap();
    imp.allocate().unwrap();
    imp.allocate().unwrap();
    imp.release(0, Retention::new(50, None).unwrap()).unwrap();
    imp.release(1, Retention::new(50, None).unwrap()).unwrap();
    imp.release(2, Retention::new(50, None).unwrap()).unwrap();
    assert_eq!(imp.eviction_candidate(), Some(0)); // least recently used

    let mut f = SwitchFaulty::new(3, Defect::LruTiebreak);
    f.allocate().unwrap();
    f.allocate().unwrap();
    f.allocate().unwrap();
    f.release(0, Retention::new(50, None).unwrap()).unwrap();
    f.release(1, Retention::new(50, None).unwrap()).unwrap();
    f.release(2, Retention::new(50, None).unwrap()).unwrap();
    assert_eq!(f.eviction_candidate(), Some(2)); // BUG: most recently used
    assert_ne!(f.eviction_candidate(), Some(0));
}

#[test]
fn switch_i4_never_evict_active() {
    // All blocks active, nothing free or reusable → real returns None (OOM).
    let mut imp = BlockManager::with_capacity(2);
    imp.allocate().unwrap();
    imp.allocate().unwrap();
    assert_eq!(imp.allocate(), None);

    // Defect: evicts an Active block instead.
    let mut f = SwitchFaulty::new(2, Defect::EvictsActive);
    f.allocate().unwrap();
    f.allocate().unwrap();
    assert!(f.allocate().is_some()); // BUG: returns an active block
}

#[test]
fn switch_i6_reuse_resets_retention() {
    // Real: release with a duration, advance past it (reverts), reuse, release
    // again with a fresh duration → priority is fresh (90), not stale (default).
    let mut imp = BlockManager::with_capacity(2);
    let id = imp.allocate().unwrap();
    imp.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    imp.clock_mut().advance(20.0).unwrap();
    assert_eq!(imp.effective_priority(id), Some(DEFAULT_PRIORITY)); // reverted
    imp.acquire(id);
    imp.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    assert_eq!(imp.effective_priority(id), Some(90)); // fresh retention

    // Defect: reuse keeps the stale expiry, so priority stays reverted.
    let mut f = SwitchFaulty::new(2, Defect::ReuseKeepsRetention);
    let fid = f.allocate().unwrap();
    f.release(fid, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    f.clock_mut().advance(20.0).unwrap();
    f.acquire(fid);
    f.release(fid, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    assert_eq!(f.effective_priority(fid), Some(DEFAULT_PRIORITY)); // BUG: stale
}

#[test]
fn switch_i7_invalidate_idempotent() {
    // Real: invalidate an active block, then again on the now-free block → no-op.
    let mut imp = BlockManager::with_capacity(4);
    let id = imp.allocate().unwrap();
    imp.invalidate(id);
    assert!(imp.is_free(id));
    let n = imp.num_free();
    imp.invalidate(id); // second invalidate is a no-op
    assert_eq!(imp.num_free(), n); // free count unchanged

    // Defect: invalidate is not idempotent — each call inflates the free count.
    let mut f = SwitchFaulty::new(4, Defect::Invalidate);
    let fid = f.allocate().unwrap();
    f.invalidate(fid);
    let n = f.num_free();
    f.invalidate(fid); // BUG: counts the already-free block again
    assert_eq!(f.num_free(), n + 1);
}

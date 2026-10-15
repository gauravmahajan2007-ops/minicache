//! Unit tests for `BlockManager` — explicit coverage of invariants I1–I9,
//! eviction ordering, duration revert, refcount, invalidate, and input
//! validation. All time is simulated (no real sleep, no hardware).

use minicache::{BlockManager, Retention, RetentionError, DEFAULT_PRIORITY, MAX_PRIORITY};

#[test]
fn default_and_max_priority() {
    assert_eq!(DEFAULT_PRIORITY, 35);
    assert_eq!(MAX_PRIORITY, 100);
}

#[test]
fn allocate_takes_free_block() {
    let mut bm = BlockManager::with_capacity(4);
    assert_eq!(bm.allocate(), Some(0));
    assert_eq!(bm.allocate(), Some(1));
    assert!(bm.is_active(0));
    assert!(bm.is_active(1));
    assert_eq!(bm.num_active(), 2);
    assert_eq!(bm.num_free(), 2);
}

#[test]
fn release_transitions_active_to_reusable_not_free() {
    let mut bm = BlockManager::with_capacity(4);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(80, None).unwrap()).unwrap();
    assert!(bm.is_reusable(id));
    assert!(!bm.is_active(id));
    assert_eq!(bm.num_reusable(), 1);
    assert_eq!(bm.num_free(), 3); // freed block is kept for reuse, not returned
}

#[test]
fn release_non_active_is_error() {
    let mut bm = BlockManager::with_capacity(4);
    assert_eq!(
        bm.release(0, Retention::default()),
        Err(RetentionError::NotActive(0))
    );
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::default()).unwrap();
    assert_eq!(
        bm.release(id, Retention::default()),
        Err(RetentionError::NotActive(id))
    );
}

#[test]
fn duration_reverts_priority_at_exact_expiry() {
    let mut bm = BlockManager::with_capacity(4);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    assert_eq!(bm.effective_priority(id), Some(90));
    bm.clock_mut().advance(5.0).unwrap();
    assert_eq!(bm.effective_priority(id), Some(90));
    bm.clock_mut().advance(5.0).unwrap(); // now == 10
    assert_eq!(bm.effective_priority(id), Some(DEFAULT_PRIORITY));
    assert!(bm.is_reusable(id)); // still reusable, only deprioritized
}

#[test]
fn duration_none_never_reverts() {
    let mut bm = BlockManager::with_capacity(4);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(90, None).unwrap()).unwrap();
    bm.clock_mut().advance(1000.0).unwrap();
    assert_eq!(bm.effective_priority(id), Some(90));
}

#[test]
fn eviction_prefers_lowest_priority() {
    let mut bm = BlockManager::with_capacity(2);
    let a = bm.allocate().unwrap();
    let b = bm.allocate().unwrap();
    bm.release(a, Retention::new(90, None).unwrap()).unwrap();
    bm.release(b, Retention::new(10, None).unwrap()).unwrap();
    assert_eq!(bm.eviction_candidate(), Some(b)); // lower priority
    let c = bm.allocate().unwrap();
    assert_eq!(c, b); // b evicted, reused
    assert!(bm.is_reusable(a)); // high-priority block untouched
}

#[test]
fn eviction_lru_tiebreak_within_same_priority() {
    let mut bm = BlockManager::with_capacity(3);
    let a = bm.allocate().unwrap();
    let b = bm.allocate().unwrap();
    let c = bm.allocate().unwrap();
    // same priority, released a then b then c
    bm.release(a, Retention::new(50, None).unwrap()).unwrap();
    bm.release(b, Retention::new(50, None).unwrap()).unwrap();
    bm.release(c, Retention::new(50, None).unwrap()).unwrap();
    // a was released first (least recently used) -> evicted first
    assert_eq!(bm.eviction_candidate(), Some(a));
}

#[test]
fn never_evict_active() {
    let mut bm = BlockManager::with_capacity(2);
    bm.allocate().unwrap();
    bm.allocate().unwrap();
    // all blocks active, no free, no reusable -> OOM
    assert_eq!(bm.allocate(), None);
    assert_eq!(bm.num_active(), 2);
}

#[test]
fn acquire_promotes_reusable_and_refcounts_active() {
    let mut bm = BlockManager::with_capacity(4);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(50, None).unwrap()).unwrap();
    assert!(bm.is_reusable(id));
    assert!(bm.acquire(id)); // reusable -> active (reuse hit)
    assert!(bm.is_active(id));
    assert_eq!(bm.refcount(id), Some(1));
    assert!(!bm.acquire(id)); // active -> refcount++
    assert_eq!(bm.refcount(id), Some(2));
}

#[test]
fn reuse_resets_retention() {
    let mut bm = BlockManager::with_capacity(4);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    bm.clock_mut().advance(20.0).unwrap(); // reverted to default
    assert_eq!(bm.effective_priority(id), Some(DEFAULT_PRIORITY));
    bm.acquire(id); // reuse
    bm.release(id, Retention::new(90, Some(10.0)).unwrap())
        .unwrap();
    assert_eq!(bm.effective_priority(id), Some(90)); // fresh retention applied
}

#[test]
fn invalidate_is_idempotent_and_frees() {
    let mut bm = BlockManager::with_capacity(4);
    let id = bm.allocate().unwrap();
    bm.release(id, Retention::default()).unwrap();
    assert!(bm.is_reusable(id));
    bm.invalidate(id);
    assert!(bm.is_free(id));
    bm.invalidate(id); // no-op on already-free
    assert!(bm.is_free(id));
    assert_eq!(bm.num_free(), 4);
}

#[test]
fn invalid_priority_rejected() {
    assert_eq!(
        Retention::new(101, None),
        Err(RetentionError::InvalidPriority(101))
    );
    assert_eq!(Retention::new(100, None).unwrap().priority, 100); // boundary ok
    assert_eq!(Retention::new(0, None).unwrap().priority, 0); // 0 is valid
}

#[test]
fn invalid_duration_rejected() {
    assert!(Retention::new(50, Some(0.0)).is_err());
    assert!(Retention::new(50, Some(-1.0)).is_err());
    assert!(Retention::new(50, Some(f64::NAN)).is_err());
    assert!(Retention::new(50, Some(f64::INFINITY)).is_err());
    assert!(Retention::new(50, Some(f64::NEG_INFINITY)).is_err());
    // finite positive is accepted, and the exact value is preserved
    assert_eq!(
        Retention::new(50, Some(10.0)).unwrap().duration_ms,
        Some(10.0)
    );
    // precise variant for a finite invalid value (avoid NaN != NaN in assert_eq!)
    assert!(matches!(
        Retention::new(50, Some(-1.0)),
        Err(RetentionError::InvalidDuration(x)) if x == -1.0
    ));
}

//! Unit tests for `PrefixIndex` and `block_key` — explicit coverage of
//! invariants I1–I6: longest-prefix correctness, exact vs prefix, pruning,
//! overwrite, empty handling, and block_key determinism + known vectors.

use minicache::{block_key, PrefixIndex};

fn idx() -> PrefixIndex {
    PrefixIndex::new()
}

#[test]
fn insert_and_exact_get() {
    let mut p = idx();
    p.insert(&[1, 2, 3], 10);
    assert_eq!(p.get(&[1, 2, 3]), Some(10));
    assert_eq!(p.get(&[1, 2]), None);
    assert_eq!(p.get(&[1, 2, 3, 4]), None);
    assert_eq!(p.len(), 1);
}

#[test]
fn longest_prefix_returns_full_chain_in_order() {
    let mut p = idx();
    p.insert(&[1], 10);
    p.insert(&[1, 2], 20);
    p.insert(&[1, 2, 3], 30);
    // query [1,2,3,4] matches spans [1], [1,2], [1,2,3] -> all three blocks
    assert_eq!(p.longest_prefix(&[1, 2, 3, 4]), (3, vec![10, 20, 30]));
    // query [1,2] matches [1], [1,2]
    assert_eq!(p.longest_prefix(&[1, 2]), (2, vec![10, 20]));
    // query [1] matches [1]
    assert_eq!(p.longest_prefix(&[1]), (1, vec![10]));
}

#[test]
fn longest_prefix_stops_at_divergence() {
    let mut p = idx();
    p.insert(&[1, 2], 10); // path 1->2
    p.insert(&[1, 3], 20); // path 1->3 (shares prefix [1])
                           // query [1,2,9]: matches [1,2] only (9 diverges), NOT [1,3]
    assert_eq!(p.longest_prefix(&[1, 2, 9]), (2, vec![10]));
    // query [1,3,9]: matches [1,3]
    assert_eq!(p.longest_prefix(&[1, 3, 9]), (2, vec![20]));
    // query [1,9]: matches nothing (no [1] block, no [1,9])
    assert_eq!(p.longest_prefix(&[1, 9]), (0, vec![]));
}

#[test]
fn longest_prefix_no_match() {
    let mut p = idx();
    p.insert(&[7, 8], 1);
    assert_eq!(p.longest_prefix(&[1, 2, 3]), (0, vec![]));
    assert_eq!(p.longest_prefix(&[]), (0, vec![]));
}

#[test]
fn remove_prunes_and_updates_lookup() {
    let mut p = idx();
    p.insert(&[1, 2, 3], 10);
    assert_eq!(p.node_count(), 4); // root + 3
    assert!(p.remove(&[1, 2, 3]));
    assert_eq!(p.get(&[1, 2, 3]), None);
    assert_eq!(p.longest_prefix(&[1, 2, 3]), (0, vec![]));
    assert_eq!(p.len(), 0);
    assert_eq!(p.node_count(), 1); // fully pruned back to root
}

#[test]
fn remove_missing_is_false() {
    let mut p = idx();
    p.insert(&[1, 2], 10);
    assert!(!p.remove(&[9, 9]));
    assert!(!p.remove(&[1])); // [1] is a node but has no block
    assert_eq!(p.len(), 1);
}

#[test]
fn remove_does_not_prune_shared_or_block_nodes() {
    let mut p = idx();
    p.insert(&[1], 10); // block at depth 1
    p.insert(&[1, 2], 20); // block at depth 2
    assert!(p.remove(&[1, 2])); // remove deeper block
                                // [1] block must survive
    assert_eq!(p.get(&[1]), Some(10));
    assert_eq!(p.longest_prefix(&[1, 2]), (1, vec![10]));
    assert_eq!(p.len(), 1);
}

#[test]
fn overwrite_replaces_block_without_changing_len() {
    let mut p = idx();
    p.insert(&[1, 2], 10);
    p.insert(&[1, 2], 99); // overwrite
    assert_eq!(p.get(&[1, 2]), Some(99));
    assert_eq!(p.len(), 1);
}

#[test]
fn empty_span_handling() {
    let mut p = idx();
    p.insert(&[], 10); // no-op
    assert_eq!(p.len(), 0);
    assert_eq!(p.get(&[]), None);
    assert!(!p.remove(&[]));
    assert_eq!(p.longest_prefix(&[]), (0, vec![]));
}

#[test]
fn block_key_is_deterministic_and_sensitive() {
    assert_eq!(block_key(&[1, 2, 3]), block_key(&[1, 2, 3]));
    assert_ne!(block_key(&[1, 2, 3]), block_key(&[1, 2, 4]));
    assert_ne!(block_key(&[1, 2]), block_key(&[1, 2, 3]));
    assert_ne!(block_key(&[1, 2, 3]), block_key(&[2, 1, 3])); // order matters
}

#[test]
fn block_key_known_vectors() {
    // FNV-1a 64 over little-endian u32 bytes; constants are part of the contract.
    assert_eq!(block_key(&[]), 0xcbf29ce484222325);
    assert_eq!(block_key(&[0]), 0x4d25767f9dce13f5);
    assert_eq!(block_key(&[1]), 0xad2aca7747985764);
}

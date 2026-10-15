//! Property-based tests for `BlockManager`: replay generated operation
//! sequences against BOTH the implementation and the independent reference
//! model, requiring exact agreement on block state, effective priority,
//! refcount, eviction candidate, and allocation result after every step.
//!
//! A small pool (4 blocks) forces eviction under simulated memory pressure.

use minicache::{BlockManager, Retention};
use proptest::prelude::*;

mod common;

const N: usize = 4;

#[derive(Debug, Clone)]
enum Op {
    Allocate,
    Release(u32, u8, Option<f64>),
    Acquire(u32),
    Invalidate(u32),
    Advance(f64),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let id = 0u32..N as u32;
    let priority = 0u8..101; // 0..=100
    let duration = prop_oneof![Just(None::<f64>), (0.1f64..50.0).prop_map(Some)];
    let delta = 0.0f64..20.0;
    prop_oneof![
        Just(Op::Allocate),
        (id.clone(), priority, duration).prop_map(|(i, p, d)| Op::Release(i, p, d)),
        id.clone().prop_map(Op::Acquire),
        id.clone().prop_map(Op::Invalidate),
        delta.clone().prop_map(Op::Advance),
    ]
}

fn assert_agree(imp: &BlockManager, oracle: &common::block_manager_ref::ReferenceBlockManager) {
    assert_eq!(imp.num_free(), oracle.num_free(), "num_free");
    assert_eq!(imp.num_active(), oracle.num_active(), "num_active");
    assert_eq!(imp.num_reusable(), oracle.num_reusable(), "num_reusable");
    assert_eq!(
        imp.eviction_candidate(),
        oracle.eviction_candidate(),
        "eviction_candidate"
    );
    for id in 0..N as u32 {
        assert_eq!(imp.is_free(id), oracle.is_free(id), "is_free {id}");
        assert_eq!(imp.is_active(id), oracle.is_active(id), "is_active {id}");
        assert_eq!(
            imp.is_reusable(id),
            oracle.is_reusable(id),
            "is_reusable {id}"
        );
        assert_eq!(imp.refcount(id), oracle.refcount(id), "refcount {id}");
        assert_eq!(
            imp.effective_priority(id),
            oracle.effective_priority(id),
            "effective_priority {id}"
        );
        assert_eq!(imp.expires_at(id), oracle.expires_at(id), "expires_at {id}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn implementation_agrees_with_reference(
        ops in proptest::collection::vec(op_strategy(), 1..200)
    ) {
        let mut imp = BlockManager::with_capacity(N);
        let mut oracle = common::block_manager_ref::ReferenceBlockManager::with_capacity(N);

        for op in ops {
            match op {
                Op::Allocate => {
                    assert_eq!(imp.allocate(), oracle.allocate(), "allocate");
                }
                Op::Release(id, p, d) => {
                    let r = Retention::new(p, d).unwrap();
                    let a = imp.release(id, r);
                    let b = oracle.release(id, r);
                    assert_eq!(a.is_ok(), b.is_ok(), "release {id}");
                }
                Op::Acquire(id) => {
                    assert_eq!(imp.acquire(id), oracle.acquire(id), "acquire {id}");
                }
                Op::Invalidate(id) => {
                    imp.invalidate(id);
                    oracle.invalidate(id);
                }
                Op::Advance(d) => {
                    imp.clock_mut().advance(d).unwrap();
                    oracle.clock_mut().advance(d).unwrap();
                }
            }
            assert_agree(&imp, &oracle);
        }
    }
}

//! Property-based tests: replay generated operation sequences against BOTH the
//! implementation and the independent reference model, requiring exact agreement
//! after every step (availability, value, and live count).
//!
//! Generated with `proptest`; both sides use simulated clocks, so these are
//! deterministic-per-seed and fast.

use minicache::MiniCache;
use proptest::prelude::*;

mod common;

#[derive(Debug, Clone)]
enum Op {
    Put(i64, i64, f64),
    Get(i64),
    Delete(i64),
    Advance(f64),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let key = 0i64..8;
    let value = -50i64..50;
    let ttl = 0.1f64..25.0;
    let delta = 0.0f64..15.0;
    prop_oneof![
        (key.clone(), value.clone(), ttl.clone()).prop_map(|(k, v, t)| Op::Put(k, v, t)),
        key.clone().prop_map(Op::Get),
        key.clone().prop_map(Op::Delete),
        delta.clone().prop_map(Op::Advance),
    ]
}

fn invalid_ttl() -> impl Strategy<Value = f64> {
    prop_oneof![
        (-100.0f64..0.0), // negatives
        Just(0.0f64),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn implementation_agrees_with_reference(
        ops in proptest::collection::vec(op_strategy(), 1..150)
    ) {
        let mut imp = MiniCache::<i64, i64>::new();
        let mut oracle = common::ReferenceCache::<i64, i64>::new();

        for op in ops {
            match op {
                Op::Put(k, v, t) => {
                    let a = imp.put(k, v, t);
                    let b = oracle.put(k, v, t);
                    assert_eq!(a.is_ok(), b.is_ok(), "put agreement for key {k}");
                }
                Op::Get(k) => {
                    assert_eq!(
                        imp.get(&k).map(|v| *v),
                        oracle.get(&k).map(|v| *v),
                        "get agreement for key {k}"
                    );
                }
                Op::Delete(k) => {
                    imp.delete(&k);
                    oracle.delete(&k);
                }
                Op::Advance(d) => {
                    imp.clock_mut().advance(d).ok();
                    oracle.clock.advance(d).ok();
                }
            }
            // full-state agreement after every step
            assert_eq!(imp.size(), oracle.size(), "size mismatch");
            for k in 0..8i64 {
                assert_eq!(
                    imp.get(&k).map(|v| *v),
                    oracle.get(&k).map(|v| *v),
                    "state mismatch for key {k}"
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn invalid_ttl_agreement(key in 0i64..8, value in -50i64..50, bad in invalid_ttl()) {
        let mut imp = MiniCache::<i64, i64>::new();
        let mut oracle = common::ReferenceCache::<i64, i64>::new();
        assert!(imp.put(key, value, bad).is_err());
        assert!(oracle.put(key, value, bad).is_err());
        assert_eq!(imp.size(), 0);
        assert_eq!(oracle.size(), 0);
    }
}

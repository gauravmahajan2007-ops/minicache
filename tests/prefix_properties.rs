//! Property-based tests for `PrefixIndex`: replay generated
//! insert/get/remove/lookup sequences against BOTH the trie implementation and
//! the independent reference model, requiring exact agreement on longest-prefix
//! results, exact get, and len after every step.

use minicache::PrefixIndex;
use proptest::prelude::*;

mod common;

fn tokens_strategy() -> impl Strategy<Value = Vec<u32>> {
    proptest::collection::vec(0u32..4, 0..6)
}

#[derive(Debug, Clone)]
enum Op {
    Insert(Vec<u32>, u32),
    Get(Vec<u32>),
    Remove(Vec<u32>),
    Lookup(Vec<u32>),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (tokens_strategy(), 0u32..1000).prop_map(|(t, b)| Op::Insert(t, b)),
        tokens_strategy().prop_map(Op::Get),
        tokens_strategy().prop_map(Op::Remove),
        tokens_strategy().prop_map(Op::Lookup),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn impl_agrees_with_reference(
        ops in proptest::collection::vec(op_strategy(), 1..150)
    ) {
        let mut imp = PrefixIndex::new();
        let mut oracle = common::prefix_ref::ReferencePrefixIndex::new();

        for op in ops {
            match op {
                Op::Insert(t, b) => {
                    imp.insert(&t, b);
                    oracle.insert(&t, b);
                }
                Op::Get(t) => {
                    assert_eq!(imp.get(&t), oracle.get(&t), "get {t:?}");
                }
                Op::Remove(t) => {
                    assert_eq!(imp.remove(&t), oracle.remove(&t), "remove {t:?}");
                }
                Op::Lookup(t) => {
                    assert_eq!(
                        imp.longest_prefix(&t),
                        oracle.longest_prefix(&t),
                        "longest_prefix {t:?}"
                    );
                }
            }
            assert_eq!(imp.len(), oracle.len(), "len mismatch");
        }
    }
}

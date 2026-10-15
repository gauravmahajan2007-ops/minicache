//! Property-based tests for `CpuPool`: random pool dimensions and blocks,
//! asserting round-trip identity, clear-to-zero, and out-of-range rejection.

use minicache::{CpuPool, KvPool, PoolError};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn round_trip_clear_and_bounds(
        num_blocks in 1usize..8,
        block_size in 1usize..24,
        block in 0..10u32,
    ) {
        let mut pool = CpuPool::new(num_blocks, block_size);
        // deterministic data derived from block + block_size
        let data: Vec<u8> = (0..block_size)
            .map(|i| block.wrapping_add(i as u32) as u8)
            .collect();

        if (block as usize) >= num_blocks {
            prop_assert!(matches!(
                pool.write_block(block, &data),
                Err(PoolError::InvalidBlock(_))
            ));
            return Ok(());
        }

        // round-trip
        pool.write_block(block, &data).unwrap();
        let mut buf = vec![0u8; block_size];
        pool.read_block(block, &mut buf).unwrap();
        prop_assert_eq!(buf, data);

        // clear -> zeros
        pool.clear_block(block).unwrap();
        let mut z = vec![1u8; block_size];
        pool.read_block(block, &mut z).unwrap();
        prop_assert!(z.iter().all(|&b| b == 0), "clear must zero the block");
    }
}

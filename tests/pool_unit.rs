//! Unit tests for `CpuPool` — invariants I1–I5: round-trip, clear, isolation,
//! bounds, and length validation.

use minicache::{CpuPool, KvPool, PoolError};

#[test]
fn write_then_read_round_trips() {
    let mut p = CpuPool::new(4, 8);
    p.write_block(0, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    let mut buf = [0u8; 8];
    p.read_block(0, &mut buf).unwrap();
    assert_eq!(&buf, &[1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn clear_zeroes_block() {
    let mut p = CpuPool::new(4, 4);
    p.write_block(0, &[1, 2, 3, 4]).unwrap();
    p.clear_block(0).unwrap();
    let mut buf = [9u8; 4];
    p.read_block(0, &mut buf).unwrap();
    assert_eq!(&buf, &[0, 0, 0, 0]);
}

#[test]
fn isolation_between_blocks() {
    let mut p = CpuPool::new(4, 4);
    p.write_block(0, &[1, 2, 3, 4]).unwrap();
    p.write_block(2, &[9, 9, 9, 9]).unwrap();
    let mut b1 = [0u8; 4];
    let mut b3 = [0u8; 4];
    p.read_block(1, &mut b1).unwrap();
    p.read_block(3, &mut b3).unwrap();
    assert_eq!(&b1, &[0, 0, 0, 0]); // block 1 untouched
    assert_eq!(&b3, &[0, 0, 0, 0]); // block 3 untouched
}

#[test]
fn bounds_check_rejects_out_of_range() {
    let mut p = CpuPool::new(4, 4);
    assert_eq!(
        p.write_block(4, &[1, 2, 3, 4]),
        Err(PoolError::InvalidBlock(4))
    );
    assert_eq!(
        p.read_block(4, &mut [0u8; 4]),
        Err(PoolError::InvalidBlock(4))
    );
    assert_eq!(p.clear_block(100), Err(PoolError::InvalidBlock(100)));
    // boundary: block 3 is valid (last block)
    assert!(p.write_block(3, &[1, 2, 3, 4]).is_ok());
}

#[test]
fn length_check_rejects_wrong_buffer_size() {
    let mut p = CpuPool::new(4, 4);
    assert_eq!(
        p.write_block(0, &[1, 2, 3]),
        Err(PoolError::BadLength {
            expected: 4,
            got: 3
        })
    );
    assert_eq!(
        p.read_block(0, &mut [0u8; 5]),
        Err(PoolError::BadLength {
            expected: 4,
            got: 5
        })
    );
}

#[test]
fn capacity_and_dimensions() {
    let p = CpuPool::new(8, 16);
    assert_eq!(p.num_blocks(), 8);
    assert_eq!(p.block_size(), 16);
    assert_eq!(p.capacity_bytes(), 128);
}

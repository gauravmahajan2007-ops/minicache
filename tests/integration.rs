//! Integration test: `BlockManager` (policy) composes with `CpuPool` (storage) —
//! KV bytes persist across release/acquire (the point of reuse) and are cleared
//! on invalidate.

use minicache::{BlockManager, CpuPool, KvPool, Retention};

#[test]
fn block_manager_and_cpu_pool_compose() {
    let mut bm = BlockManager::with_capacity(4);
    let mut pool = CpuPool::new(4, 8);
    let kv = [1u8, 2, 3, 4, 5, 6, 7, 8];

    // allocate a block and write its KV bytes
    let id = bm.allocate().unwrap();
    pool.write_block(id, &kv).unwrap();

    // release -> reusable; bytes must survive (reuse depends on it)
    bm.release(id, Retention::new(80, None).unwrap()).unwrap();
    let mut buf = [0u8; 8];
    pool.read_block(id, &mut buf).unwrap();
    assert_eq!(&buf, &kv, "bytes must survive release");

    // acquire (reuse hit) -> active again; bytes still present
    assert!(bm.acquire(id));
    let mut buf2 = [0u8; 8];
    pool.read_block(id, &mut buf2).unwrap();
    assert_eq!(&buf2, &kv, "bytes must survive acquire/reuse");

    // invalidate -> free; clear the bytes (no stale KV leak)
    bm.invalidate(id);
    pool.clear_block(id).unwrap();
    let mut buf3 = [0u8; 8];
    pool.read_block(id, &mut buf3).unwrap();
    assert_eq!(&buf3, &[0u8; 8], "invalidate + clear must zero the block");
}

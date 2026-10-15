//! `KvPool` — the device abstraction for KV block storage.
//!
//! A fixed set of equal-sized blocks of raw bytes. `BlockManager` decides which
//! block is free/active/reusable; a `KvPool` stores and retrieves the bytes at
//! that block. `CpuPool` is host memory; `CudaPool` (see `cuda_pool.rs`) is GPU
//! device memory behind the `cuda` feature.

use crate::BlockId;

/// Errors returned by pool operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// `block >= num_blocks`.
    InvalidBlock(BlockId),
    /// A caller buffer's length did not equal `block_size`.
    BadLength { expected: usize, got: usize },
    /// Device allocation failed (CUDA error code).
    AllocFailed(i32),
}

/// A pool of fixed-size byte blocks backing KV storage.
pub trait KvPool {
    fn num_blocks(&self) -> usize;
    fn block_size(&self) -> usize;

    /// Total capacity in bytes.
    fn capacity_bytes(&self) -> usize {
        self.num_blocks() * self.block_size()
    }

    /// Copy a block's bytes into `buf` (must be `block_size` long).
    fn read_block(&self, block: BlockId, buf: &mut [u8]) -> Result<(), PoolError>;

    /// Copy `data` (must be `block_size` long) into a block.
    fn write_block(&mut self, block: BlockId, data: &[u8]) -> Result<(), PoolError>;

    /// Zero a block's bytes (on free, so stale KV is not leaked).
    fn clear_block(&mut self, block: BlockId) -> Result<(), PoolError>;
}

/// A host-memory pool: a single contiguous `Vec<u8>` divided into blocks.
pub struct CpuPool {
    block_size: usize,
    num_blocks: usize,
    data: Vec<u8>,
}

impl CpuPool {
    pub fn new(num_blocks: usize, block_size: usize) -> Self {
        Self {
            block_size,
            num_blocks,
            data: vec![0u8; num_blocks * block_size],
        }
    }

    fn offset(&self, block: BlockId) -> Result<usize, PoolError> {
        if block as usize >= self.num_blocks {
            return Err(PoolError::InvalidBlock(block));
        }
        Ok(block as usize * self.block_size)
    }
}

impl KvPool for CpuPool {
    fn num_blocks(&self) -> usize {
        self.num_blocks
    }

    fn block_size(&self) -> usize {
        self.block_size
    }

    fn read_block(&self, block: BlockId, buf: &mut [u8]) -> Result<(), PoolError> {
        let off = self.offset(block)?;
        if buf.len() != self.block_size {
            return Err(PoolError::BadLength {
                expected: self.block_size,
                got: buf.len(),
            });
        }
        buf.copy_from_slice(&self.data[off..off + self.block_size]);
        Ok(())
    }

    fn write_block(&mut self, block: BlockId, data: &[u8]) -> Result<(), PoolError> {
        let off = self.offset(block)?;
        if data.len() != self.block_size {
            return Err(PoolError::BadLength {
                expected: self.block_size,
                got: data.len(),
            });
        }
        self.data[off..off + self.block_size].copy_from_slice(data);
        Ok(())
    }

    fn clear_block(&mut self, block: BlockId) -> Result<(), PoolError> {
        let off = self.offset(block)?;
        self.data[off..off + self.block_size].fill(0);
        Ok(())
    }
}

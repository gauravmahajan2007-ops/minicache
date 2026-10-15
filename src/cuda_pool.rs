//! `CudaPool` — GPU-device-memory `KvPool`, feature-gated behind `cuda`.
//!
//! Uses the CUDA runtime (`cudaMalloc`/`cudaMemcpy`/`cudaMemset`/`cudaFree`).
//!
//! **Not linked or run on the development host** (no CUDA toolkit / GPU): this
//! module only compiles with `--features cuda`, and is type-checked here via
//! `cargo check --features cuda`, not executed. It is the device-seam
//! implementation for GPU deployments, provided as a starting point.

use crate::pool::{KvPool, PoolError};
use crate::BlockId;

#[cfg(feature = "cuda")]
extern "C" {
    fn cudaMalloc(ptr: *mut *mut u8, size: usize) -> i32;
    fn cudaMemcpy(dst: *mut u8, src: *const u8, count: usize, kind: u32) -> i32;
    fn cudaMemset(ptr: *mut u8, value: i32, count: usize) -> i32;
    fn cudaFree(ptr: *mut u8) -> i32;
}

#[cfg(feature = "cuda")]
const CUDA_SUCCESS: i32 = 0;
#[cfg(feature = "cuda")]
const CUDA_MEMCPY_HOST_TO_DEVICE: u32 = 1;
#[cfg(feature = "cuda")]
const CUDA_MEMCPY_DEVICE_TO_HOST: u32 = 2;

/// A GPU-device-memory block pool.
#[cfg(feature = "cuda")]
pub struct CudaPool {
    block_size: usize,
    num_blocks: usize,
    dev: *mut u8,
}

#[cfg(feature = "cuda")]
unsafe impl Send for CudaPool {}

#[cfg(feature = "cuda")]
impl CudaPool {
    /// Allocate `num_blocks * block_size` bytes of device memory.
    pub fn new(num_blocks: usize, block_size: usize) -> Result<Self, PoolError> {
        let mut dev: *mut u8 = std::ptr::null_mut();
        let rc = unsafe { cudaMalloc(&mut dev, num_blocks * block_size) };
        if rc != CUDA_SUCCESS || dev.is_null() {
            return Err(PoolError::AllocFailed(rc));
        }
        Ok(Self {
            block_size,
            num_blocks,
            dev,
        })
    }

    fn offset(&self, block: BlockId) -> Result<usize, PoolError> {
        if block as usize >= self.num_blocks {
            return Err(PoolError::InvalidBlock(block));
        }
        Ok(block as usize * self.block_size)
    }
}

#[cfg(feature = "cuda")]
impl KvPool for CudaPool {
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
        let rc = unsafe {
            cudaMemcpy(
                buf.as_mut_ptr(),
                self.dev.add(off),
                self.block_size,
                CUDA_MEMCPY_DEVICE_TO_HOST,
            )
        };
        if rc != CUDA_SUCCESS {
            return Err(PoolError::AllocFailed(rc));
        }
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
        let rc = unsafe {
            cudaMemcpy(
                self.dev.add(off),
                data.as_ptr(),
                self.block_size,
                CUDA_MEMCPY_HOST_TO_DEVICE,
            )
        };
        if rc != CUDA_SUCCESS {
            return Err(PoolError::AllocFailed(rc));
        }
        Ok(())
    }

    fn clear_block(&mut self, block: BlockId) -> Result<(), PoolError> {
        let off = self.offset(block)?;
        let rc = unsafe { cudaMemset(self.dev.add(off), 0, self.block_size) };
        if rc != CUDA_SUCCESS {
            return Err(PoolError::AllocFailed(rc));
        }
        Ok(())
    }
}

#[cfg(feature = "cuda")]
impl Drop for CudaPool {
    fn drop(&mut self) {
        if !self.dev.is_null() {
            unsafe { cudaFree(self.dev) };
        }
    }
}

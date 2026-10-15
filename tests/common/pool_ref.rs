//! Test-only faulty fixture for the KvPool repair workflow: `clear_block` is a
//! no-op (returns Ok but does not zero the block), leaking stale KV data on free.

/// A host-memory pool whose `clear_block` deliberately does nothing.
pub struct FaultyCpuPool {
    block_size: usize,
    num_blocks: usize,
    data: Vec<u8>,
}

impl FaultyCpuPool {
    pub fn new(num_blocks: usize, block_size: usize) -> Self {
        Self {
            block_size,
            num_blocks,
            data: vec![0u8; num_blocks * block_size],
        }
    }

    fn offset(&self, block: u32) -> Result<usize, ()> {
        if block as usize >= self.num_blocks {
            return Err(());
        }
        Ok(block as usize * self.block_size)
    }

    pub fn write_block(&mut self, block: u32, data: &[u8]) -> Result<(), ()> {
        let off = self.offset(block)?;
        self.data[off..off + self.block_size].copy_from_slice(data);
        Ok(())
    }

    pub fn read_block(&self, block: u32, buf: &mut [u8]) -> Result<(), ()> {
        let off = self.offset(block)?;
        buf.copy_from_slice(&self.data[off..off + self.block_size]);
        Ok(())
    }

    pub fn clear_block(&mut self, _block: u32) -> Result<(), ()> {
        // BUG: does not zero the block
        Ok(())
    }
}

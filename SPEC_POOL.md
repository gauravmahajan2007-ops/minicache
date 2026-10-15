# KvPool — Behavioral Specification (Device Byte-Pool Seam)

Version 1.0 — Systems Lab Pass 3. The storage layer beneath `BlockManager`: where
a block's KV bytes actually live, abstracted over the device.

## 1. Purpose

`KvPool` is the **device abstraction**: a fixed set of equal-sized blocks of raw
bytes (the KV tensor storage). `BlockManager` (Pass 1) decides *which* block is
free/active/reusable; `KvPool` stores and retrieves the bytes *at* that block.
Two implementations: `CpuPool` (host memory, fully tested) and `CudaPool` (GPU
device memory, feature-gated).

## 2. Domain

- A pool has `num_blocks` blocks, each `block_size` bytes, laid out contiguously:
  `offset(block) = block * block_size`, valid for `block in 0..num_blocks`.
- `read_block` copies a block's bytes *out* into a caller buffer.
- `write_block` copies caller bytes *into* a block.
- `clear_block` zeroes a block (on free, so stale KV is not leaked).

## 3. Operations

- `num_blocks() -> usize`, `block_size() -> usize`.
- `read_block(block, buf) -> Result<(), PoolError>` — `buf.len()` must equal
  `block_size`.
- `write_block(block, data) -> Result<(), PoolError>` — `data.len()` must equal
  `block_size`.
- `clear_block(block) -> Result<(), PoolError>` — zeroes the block.

## 4. Errors

- `InvalidBlock(BlockId)` — `block >= num_blocks`.
- `BadLength { expected, got }` — buffer length != `block_size`.
- `AllocFailed(i32)` — device allocation failed (CUDA only).

## 5. Invariants

- **I1 (round-trip):** `write_block(b, d)` then `read_block(b, buf)` yields
  `buf == d` (and mutates no other block).
- **I2 (clear):** after `clear_block(b)`, `read_block(b)` yields all zeros.
- **I3 (isolation):** writing block `b` never changes the bytes of any other
  block.
- **I4 (bounds):** `block >= num_blocks` is rejected with `InvalidBlock`, never
  accepted (no out-of-range access).
- **I5 (length):** a buffer whose length != `block_size` is rejected with
  `BadLength`, never accepted.

## 6. Reference model / verification

The byte pool is simple enough that the round-trip identity *is* the oracle:
property tests write random data to random blocks and assert read-back equality,
isolation, and clear behavior directly (no separate reference struct is needed —
one would be isomorphic to `CpuPool` itself).

## 7. CUDA seam (unverified on this host)

`CudaPool` is the same trait over `cudaMalloc`/`cudaMemcpy`/`cudaMemset`/`cudaFree`
device memory. It is **feature-gated** (`--features cuda`) and **requires the CUDA
runtime + a GPU to link and run**. On this host it is provided as the device-seam
implementation but has **not** been linked or executed (no CUDA toolkit/GPU);
`cargo check --features cuda` is used to confirm it at least type-checks.
